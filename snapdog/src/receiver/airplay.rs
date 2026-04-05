// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! AirPlay 1 + 2 receiver implementing [`ReceiverProvider`].
//!
//! Bridges the [`shairplay`] crate's callback-based API into SnapDog's
//! channel-based receiver model. Audio is delivered as F32 interleaved PCM.

use std::sync::Arc;

use anyhow::Result;
use shairplay::{BindConfig, PairingStore, RaopServer};

use super::{AudioFormat, AudioSender, ReceiverEvent, ReceiverEventTx, ReceiverProvider};
use crate::config::AirplayConfig;

// ── AirPlayReceiver ───────────────────────────────────────────

/// AirPlay receiver wrapping [`shairplay::RaopServer`].
pub struct AirPlayReceiver {
    config: AirplayConfig,
    zone_index: usize,
    server: Option<RaopServer>,
}

impl AirPlayReceiver {
    /// Create a new (stopped) AirPlay receiver for the given zone.
    pub fn new(config: AirplayConfig, zone_index: usize) -> Self {
        Self {
            config,
            zone_index,
            server: None,
        }
    }
}

impl ReceiverProvider for AirPlayReceiver {
    fn name(&self) -> &'static str {
        "AirPlay"
    }

    async fn start(&mut self, audio_tx: AudioSender, event_tx: ReceiverEventTx) -> Result<()> {
        let mut hwaddr = detect_hwaddr();
        hwaddr[5] = hwaddr[5].wrapping_add(self.zone_index as u8);

        let handler = Arc::new(BridgeHandler { audio_tx, event_tx });

        let mut builder = RaopServer::builder()
            .name(&self.config.name)
            .hwaddr(hwaddr.to_vec())
            .port(7000 + self.zone_index as u16)
            .max_clients(1);

        if let Some(ref pw) = self.config.password {
            builder = builder.password(pw);
        }

        if let Some(ref addrs) = self.config.bind {
            builder = builder.bind(BindConfig::new().addrs(addrs.clone()));
        }

        if let Some(ref path) = self.config.pairing_store {
            builder = builder.pairing_store(Arc::new(FilePairingStore::new(path.clone())));
        }

        let mut server = builder.build(handler)?;
        server.start().await?;

        let port = server.service_info().port;
        tracing::info!(name = %self.config.name, port, zone = self.zone_index, "AirPlay 2 receiver started");

        self.server = Some(server);
        Ok(())
    }

    async fn stop(&mut self) {
        if let Some(ref mut server) = self.server {
            server.stop().await;
        }
        self.server = None;
    }

    fn is_running(&self) -> bool {
        self.server.as_ref().is_some_and(|s| s.is_running())
    }
}

// ── AudioHandler / AudioSession bridge ────────────────────────

struct BridgeHandler {
    audio_tx: AudioSender,
    event_tx: ReceiverEventTx,
}

impl shairplay::AudioHandler for BridgeHandler {
    fn audio_init(&self, format: shairplay::AudioFormat) -> Box<dyn shairplay::AudioSession> {
        tracing::info!(
            channels = format.channels,
            sample_rate = format.sample_rate,
            "AirPlay audio session started"
        );
        let _ = self.event_tx.try_send(ReceiverEvent::SessionStarted {
            format: AudioFormat {
                sample_rate: format.sample_rate,
                channels: format.channels as u16,
            },
        });
        Box::new(BridgeSession {
            audio_tx: self.audio_tx.clone(),
            event_tx: self.event_tx.clone(),
        })
    }
}

struct BridgeSession {
    audio_tx: AudioSender,
    event_tx: ReceiverEventTx,
}

impl shairplay::AudioSession for BridgeSession {
    fn audio_process(&mut self, samples: &[f32]) {
        let _ = self.audio_tx.try_send(samples.to_vec());
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
        tracing::debug!(percent, "AirPlay volume");
        let _ = self.event_tx.try_send(ReceiverEvent::Volume { percent });
    }

    fn audio_set_metadata(&mut self, metadata: &[u8]) {
        let (title, artist, album) = parse_dmap(metadata);
        tracing::debug!(title = %title, artist = %artist, "AirPlay metadata");
        let _ = self.event_tx.try_send(ReceiverEvent::Metadata {
            title,
            artist,
            album,
        });
    }

    fn audio_set_coverart(&mut self, coverart: &[u8]) {
        tracing::debug!(size = coverart.len(), "AirPlay cover art");
        let _ = self.event_tx.try_send(ReceiverEvent::CoverArt {
            bytes: coverart.to_vec(),
        });
    }

    fn audio_set_progress(&mut self, start: u32, current: u32, end: u32) {
        let position_ms = ((current - start) as u64 * 1000) / 44100;
        let duration_ms = ((end - start) as u64 * 1000) / 44100;
        let _ = self.event_tx.try_send(ReceiverEvent::Progress {
            position_ms,
            duration_ms,
        });
    }

    fn remote_control_available(&mut self, remote: Arc<dyn shairplay::RemoteControl>) {
        tracing::debug!("AirPlay remote control available");
        let _ = self.event_tx.try_send(ReceiverEvent::RemoteAvailable {
            remote: Arc::new(ShairplayRemoteBridge(remote)),
        });
    }
}

impl Drop for BridgeSession {
    fn drop(&mut self) {
        tracing::info!("AirPlay audio session ended");
        let _ = self.event_tx.try_send(ReceiverEvent::SessionEnded);
    }
}

// ── RemoteControl bridge ──────────────────────────────────────

/// Bridges shairplay's [`RemoteControl`](shairplay::RemoteControl) to SnapDog's
/// protocol-agnostic [`RemoteControl`](super::RemoteControl).
struct ShairplayRemoteBridge(Arc<dyn shairplay::RemoteControl>);

impl super::RemoteControl for ShairplayRemoteBridge {
    fn send_command(&self, cmd: super::RemoteCommand) -> Result<()> {
        let sp_cmd = match cmd {
            super::RemoteCommand::Play => shairplay::RemoteCommand::Play,
            super::RemoteCommand::Pause => shairplay::RemoteCommand::Pause,
            super::RemoteCommand::NextTrack => shairplay::RemoteCommand::NextTrack,
            super::RemoteCommand::PreviousTrack => shairplay::RemoteCommand::PreviousTrack,
            super::RemoteCommand::Stop => shairplay::RemoteCommand::Stop,
            super::RemoteCommand::SetVolume(v) => shairplay::RemoteCommand::SetVolume(v),
            super::RemoteCommand::ToggleShuffle => shairplay::RemoteCommand::ToggleShuffle,
            super::RemoteCommand::ToggleRepeat => shairplay::RemoteCommand::ToggleRepeat,
        };
        self.0.send_command(sp_cmd).map_err(Into::into)
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
