// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! AirPlay 1 + 2 (RAOP) receiver via the pure-Rust `shairplay` crate.
//!
//! Implements [`shairplay::AudioHandler`] / [`shairplay::AudioSession`] to bridge
//! decoded PCM audio and metadata into the SnapDog ZonePlayer channels.

use std::sync::Arc;

use anyhow::Result;
use shairplay::{
    AudioFormat, AudioHandler, AudioSession, BindConfig, PairingStore, RaopServer, RemoteControl,
};
use tokio::sync::mpsc;

use crate::audio::PcmSender;
use crate::config::AirplayConfig;

// ── Public types (unchanged from FFI version) ─────────────────

/// AirPlay events sent to the ZonePlayer.
pub enum AirplayEvent {
    Metadata {
        title: String,
        artist: String,
        album: String,
    },
    CoverArt {
        bytes: Vec<u8>,
    },
    Progress {
        position_ms: u64,
        duration_ms: u64,
    },
    Volume {
        percent: i32,
    },
    RemoteAvailable {
        remote: Arc<dyn RemoteControl>,
    },
    SessionEnded,
}

pub type AirplayEventSender = mpsc::Sender<AirplayEvent>;
pub type AirplayEventReceiver = mpsc::Receiver<AirplayEvent>;

// ── AirplayReceiver ───────────────────────────────────────────

/// AirPlay receiver wrapping [`shairplay::RaopServer`].
pub struct AirplayReceiver {
    server: RaopServer,
}

impl AirplayReceiver {
    /// Start the AirPlay receiver. PCM audio goes to `pcm_tx`, events to `event_tx`.
    pub async fn start(
        config: &AirplayConfig,
        zone_index: usize,
        pcm_tx: PcmSender,
        event_tx: AirplayEventSender,
    ) -> Result<Self> {
        let mut hwaddr = detect_hwaddr();
        hwaddr[5] = hwaddr[5].wrapping_add(zone_index as u8);

        let handler = Arc::new(BridgeHandler { pcm_tx, event_tx });

        let mut builder = RaopServer::builder()
            .name(&config.name)
            .hwaddr(hwaddr.to_vec())
            .port(7000 + zone_index as u16)
            .max_clients(1);

        if let Some(ref pw) = config.password {
            builder = builder.password(pw);
        }

        if let Some(ref addrs) = config.bind {
            builder = builder.bind(BindConfig::new().addrs(addrs.clone()));
        }

        if let Some(ref path) = config.pairing_store {
            builder = builder.pairing_store(Arc::new(FilePairingStore::new(path.clone())));
        }

        let mut server = builder.build(handler)?;
        server.start().await?;

        let port = server.service_info().port;
        tracing::info!(name = %config.name, port, "AirPlay 2 receiver started");

        Ok(Self { server })
    }

    pub fn is_running(&self) -> bool {
        self.server.is_running()
    }

    pub async fn stop(&mut self) {
        self.server.stop().await;
    }
}

// ── AudioHandler / AudioSession bridge ────────────────────────

struct BridgeHandler {
    pcm_tx: PcmSender,
    event_tx: AirplayEventSender,
}

impl AudioHandler for BridgeHandler {
    fn audio_init(&self, format: AudioFormat) -> Box<dyn AudioSession> {
        tracing::info!(
            bits = format.bits,
            channels = format.channels,
            samplerate = format.sample_rate,
            "AirPlay audio session started"
        );
        Box::new(BridgeSession {
            pcm_tx: self.pcm_tx.clone(),
            event_tx: self.event_tx.clone(),
        })
    }
}

struct BridgeSession {
    pcm_tx: PcmSender,
    event_tx: AirplayEventSender,
}

impl AudioSession for BridgeSession {
    fn audio_process(&mut self, samples: &[f32]) {
        // Convert F32 interleaved PCM to S16LE for the pipeline
        let s16_bytes: Vec<u8> = samples
            .iter()
            .flat_map(|&s| {
                let clamped = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
                clamped.to_le_bytes()
            })
            .collect();
        let _ = self.pcm_tx.try_send(s16_bytes);
    }

    fn audio_flush(&mut self) {
        tracing::debug!("AirPlay audio flush");
    }

    fn audio_set_volume(&mut self, volume: f32) {
        let percent = if volume <= -144.0 {
            0
        } else {
            ((volume + 30.0) / 30.0 * 100.0).clamp(0.0, 100.0) as i32
        };
        tracing::info!(raw = volume, percent, "AirPlay volume");
        let _ = self.event_tx.try_send(AirplayEvent::Volume { percent });
    }

    fn audio_set_metadata(&mut self, metadata: &[u8]) {
        let (title, artist, album) = parse_dmap(metadata);
        tracing::info!(title = %title, artist = %artist, album = %album, "AirPlay metadata");
        let _ = self.event_tx.try_send(AirplayEvent::Metadata {
            title,
            artist,
            album,
        });
    }

    fn audio_set_coverart(&mut self, coverart: &[u8]) {
        tracing::info!(size = coverart.len(), "AirPlay cover art received");
        let _ = self.event_tx.try_send(AirplayEvent::CoverArt {
            bytes: coverart.to_vec(),
        });
    }

    fn audio_set_progress(&mut self, start: u32, current: u32, end: u32) {
        let position_ms = ((current - start) as u64 * 1000) / 44100;
        let duration_ms = ((end - start) as u64 * 1000) / 44100;
        let _ = self.event_tx.try_send(AirplayEvent::Progress {
            position_ms,
            duration_ms,
        });
    }

    fn remote_control_available(&mut self, remote: Arc<dyn RemoteControl>) {
        tracing::info!("Remote control available");
        let _ = self
            .event_tx
            .try_send(AirplayEvent::RemoteAvailable { remote });
    }
}

impl Drop for BridgeSession {
    fn drop(&mut self) {
        tracing::info!("AirPlay audio session ended");
        let _ = self.event_tx.try_send(AirplayEvent::SessionEnded);
    }
}

// ── FilePairingStore (AP2 key persistence) ────────────────────

struct FilePairingStore {
    path: std::path::PathBuf,
    keys: std::sync::Mutex<std::collections::HashMap<String, [u8; 32]>>,
}

impl FilePairingStore {
    fn new(path: std::path::PathBuf) -> Self {
        let keys = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self {
            path,
            keys: std::sync::Mutex::new(keys),
        }
    }

    fn save(&self, keys: &std::collections::HashMap<String, [u8; 32]>) {
        if let Ok(json) = serde_json::to_string_pretty(keys) {
            let _ = std::fs::write(&self.path, json);
        }
    }
}

impl PairingStore for FilePairingStore {
    fn get(&self, device_id: &str) -> Option<[u8; 32]> {
        self.keys.lock().ok()?.get(device_id).copied()
    }
    fn put(&self, device_id: &str, public_key: [u8; 32]) {
        if let Ok(mut keys) = self.keys.lock() {
            keys.insert(device_id.to_string(), public_key);
            self.save(&keys);
        }
    }
    fn remove(&self, device_id: &str) {
        if let Ok(mut keys) = self.keys.lock() {
            keys.remove(device_id);
            self.save(&keys);
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────

/// Detect the MAC address of the primary network interface.
pub(crate) fn detect_hwaddr() -> [u8; 6] {
    mac_address::get_mac_address()
        .ok()
        .flatten()
        .map(|mac| mac.bytes())
        .unwrap_or([0x02, 0x42, 0xAA, 0xBB, 0xCC, 0x00])
}

/// Parse DMAP metadata buffer. Returns (title, artist, album).
pub fn parse_dmap(data: &[u8]) -> (String, String, String) {
    let mut title = String::new();
    let mut artist = String::new();
    let mut album = String::new();

    let mut i = 0;
    while i + 8 <= data.len() {
        let tag = &data[i..i + 4];
        let len = u32::from_be_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]]) as usize;
        i += 8;
        if i + len > data.len() {
            break;
        }
        let value = std::str::from_utf8(&data[i..i + len]).unwrap_or("");
        match tag {
            b"minm" => title = value.to_string(),
            b"asar" => artist = value.to_string(),
            b"asal" => album = value.to_string(),
            b"mlit" => {
                let (t, ar, al) = parse_dmap(&data[i..i + len]);
                if !t.is_empty() {
                    title = t;
                }
                if !ar.is_empty() {
                    artist = ar;
                }
                if !al.is_empty() {
                    album = al;
                }
            }
            _ => {}
        }
        i += len;
    }
    (title, artist, album)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dmap_metadata() {
        let mut data = Vec::new();
        data.extend_from_slice(b"minm");
        data.extend_from_slice(&9u32.to_be_bytes());
        data.extend_from_slice(b"Test Song");
        data.extend_from_slice(b"asar");
        data.extend_from_slice(&6u32.to_be_bytes());
        data.extend_from_slice(b"Artist");

        let (title, artist, album) = parse_dmap(&data);
        assert_eq!(title, "Test Song");
        assert_eq!(artist, "Artist");
        assert_eq!(album, "");
    }
}
