// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! AirPlay 1 (RAOP) receiver via libshairplay FFI.
//!
//! Safe Rust wrappers around libshairplay's C API.
//! Receives PCM audio + metadata + cover art from AirPlay clients.

mod ffi;

use std::ffi::CString;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::mpsc;

use crate::audio::PcmSender;
use crate::config::AirplayConfig;

/// AirPlay events sent to the ZonePlayer.
#[derive(Debug)]
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
    SessionEnded,
}

pub type AirplayEventSender = mpsc::Sender<AirplayEvent>;
pub type AirplayEventReceiver = mpsc::Receiver<AirplayEvent>;

/// AirPlay receiver. Runs RAOP server + mDNS advertisement.
pub struct AirplayReceiver {
    raop: *mut ffi::raop_t,
    dnssd: *mut ffi::dnssd_t,
    _callback_data: Arc<CallbackData>,
}

unsafe impl Send for AirplayReceiver {}

struct CallbackData {
    pcm_tx: PcmSender,
    event_tx: AirplayEventSender,
}

impl AirplayReceiver {
    /// Start the AirPlay receiver. PCM audio goes to `pcm_tx`, events to `event_tx`.
    #[tracing::instrument(skip(pcm_tx, event_tx))]
    pub fn start(
        config: &AirplayConfig,
        pcm_tx: PcmSender,
        event_tx: AirplayEventSender,
    ) -> Result<Self> {
        let callback_data = Arc::new(CallbackData { pcm_tx, event_tx });
        let cls = Arc::as_ptr(&callback_data) as *mut std::ffi::c_void;

        let pemkey = include_str!("../../../vendor/shairplay/airport.key");
        let pemkey_c = CString::new(pemkey).context("Invalid PEM key")?;

        let callbacks = ffi::raop_callbacks_t {
            cls,
            audio_init: Some(cb_audio_init),
            audio_process: Some(cb_audio_process),
            audio_destroy: Some(cb_audio_destroy),
            audio_flush: Some(cb_audio_flush),
            audio_set_volume: Some(cb_audio_set_volume),
            audio_set_metadata: Some(cb_audio_set_metadata),
            audio_set_coverart: Some(cb_audio_set_coverart),
            audio_remote_control_id: None,
            audio_set_progress: Some(cb_audio_set_progress),
        };

        let mut error: i32 = 0;
        let raop = unsafe {
            ffi::raop_init(
                1,
                &callbacks as *const _ as *mut _,
                pemkey_c.as_ptr(),
                &mut error,
            )
        };
        if raop.is_null() {
            anyhow::bail!("raop_init failed with error {error}");
        }

        unsafe { ffi::raop_set_log_level(raop, 6) };

        let hwaddr: [u8; 6] = [0x02, 0x42, 0xAA, 0xBB, 0xCC, 0xDD];
        let mut port: u16 = 0; // OS assigns free port
        let password = config
            .password
            .as_deref()
            .and_then(|p| CString::new(p).ok());
        let password_ptr = password.as_ref().map_or(std::ptr::null(), |p| p.as_ptr());

        let ret = unsafe {
            ffi::raop_start(
                raop,
                &mut port,
                hwaddr.as_ptr() as *const i8,
                hwaddr.len() as i32,
                password_ptr,
            )
        };
        if ret < 0 {
            unsafe { ffi::raop_destroy(raop) };
            anyhow::bail!("raop_start failed with error {ret}");
        }

        let mut dnssd_error: i32 = 0;
        let dnssd = unsafe { ffi::dnssd_init(&mut dnssd_error) };
        if dnssd.is_null() {
            unsafe {
                ffi::raop_stop(raop);
                ffi::raop_destroy(raop);
            }
            anyhow::bail!("dnssd_init failed with error {dnssd_error}");
        }

        let name_c = CString::new(config.name.as_str()).context("Invalid AirPlay name")?;
        let has_password = i32::from(config.password.is_some());
        unsafe {
            ffi::dnssd_register_raop(
                dnssd,
                name_c.as_ptr(),
                port,
                hwaddr.as_ptr() as *const i8,
                hwaddr.len() as i32,
                has_password,
            );
        }

        tracing::info!(name = %config.name, port, "AirPlay receiver started");
        Ok(Self {
            raop,
            dnssd,
            _callback_data: callback_data,
        })
    }

    pub fn is_running(&self) -> bool {
        unsafe { ffi::raop_is_running(self.raop) != 0 }
    }
}

impl Drop for AirplayReceiver {
    fn drop(&mut self) {
        tracing::info!("Stopping AirPlay receiver");
        unsafe {
            ffi::dnssd_unregister_raop(self.dnssd);
            ffi::dnssd_destroy(self.dnssd);
            ffi::raop_stop(self.raop);
            ffi::raop_destroy(self.raop);
        }
    }
}

// ── C Callbacks ───────────────────────────────────────────────

unsafe extern "C" fn cb_audio_init(
    _cls: *mut std::ffi::c_void,
    bits: i32,
    channels: i32,
    samplerate: i32,
) -> *mut std::ffi::c_void {
    tracing::info!(bits, channels, samplerate, "AirPlay audio session started");
    std::ptr::null_mut()
}

unsafe extern "C" fn cb_audio_process(
    cls: *mut std::ffi::c_void,
    _session: *mut std::ffi::c_void,
    buffer: *const std::ffi::c_void,
    buflen: i32,
) {
    if cls.is_null() || buffer.is_null() || buflen <= 0 {
        return;
    }
    let data = unsafe { &*(cls as *const CallbackData) };
    let pcm = unsafe { std::slice::from_raw_parts(buffer as *const u8, buflen as usize) };
    let _ = data.pcm_tx.try_send(pcm.to_vec());
}

unsafe extern "C" fn cb_audio_destroy(cls: *mut std::ffi::c_void, _session: *mut std::ffi::c_void) {
    tracing::info!("AirPlay audio session ended");
    if !cls.is_null() {
        let data = unsafe { &*(cls as *const CallbackData) };
        let _ = data.event_tx.try_send(AirplayEvent::SessionEnded);
    }
}

unsafe extern "C" fn cb_audio_flush(_cls: *mut std::ffi::c_void, _session: *mut std::ffi::c_void) {
    tracing::debug!("AirPlay audio flush");
}

unsafe extern "C" fn cb_audio_set_volume(
    cls: *mut std::ffi::c_void,
    _session: *mut std::ffi::c_void,
    volume: f32,
) {
    // AirPlay volume: -144.0 (mute) to 0.0 (max)
    let percent = if volume <= -144.0 {
        0
    } else {
        ((volume + 30.0) / 30.0 * 100.0).clamp(0.0, 100.0) as i32
    };
    tracing::info!(raw = volume, percent, "AirPlay volume");
    if !cls.is_null() {
        let data = unsafe { &*(cls as *const CallbackData) };
        let _ = data.event_tx.try_send(AirplayEvent::Volume { percent });
    }
}

unsafe extern "C" fn cb_audio_set_metadata(
    cls: *mut std::ffi::c_void,
    _session: *mut std::ffi::c_void,
    buffer: *const std::ffi::c_void,
    buflen: i32,
) {
    if cls.is_null() || buffer.is_null() || buflen <= 0 {
        return;
    }
    let data = unsafe { &*(cls as *const CallbackData) };
    let bytes = unsafe { std::slice::from_raw_parts(buffer as *const u8, buflen as usize) };

    // DMAP metadata — parse key fields
    let (title, artist, album) = parse_dmap(bytes);
    tracing::info!(title = %title, artist = %artist, album = %album, "AirPlay metadata");
    let _ = data.event_tx.try_send(AirplayEvent::Metadata {
        title,
        artist,
        album,
    });
}

unsafe extern "C" fn cb_audio_set_coverart(
    cls: *mut std::ffi::c_void,
    _session: *mut std::ffi::c_void,
    buffer: *const std::ffi::c_void,
    buflen: i32,
) {
    if cls.is_null() || buffer.is_null() || buflen <= 0 {
        return;
    }
    let data = unsafe { &*(cls as *const CallbackData) };
    let bytes = unsafe { std::slice::from_raw_parts(buffer as *const u8, buflen as usize) };
    tracing::info!(size = bytes.len(), "AirPlay cover art received");
    let _ = data.event_tx.try_send(AirplayEvent::CoverArt {
        bytes: bytes.to_vec(),
    });
}

unsafe extern "C" fn cb_audio_set_progress(
    cls: *mut std::ffi::c_void,
    _session: *mut std::ffi::c_void,
    start: std::ffi::c_uint,
    curr: std::ffi::c_uint,
    end: std::ffi::c_uint,
) {
    if cls.is_null() {
        return;
    }
    let data = unsafe { &*(cls as *const CallbackData) };
    // RTP timestamps at 44100 Hz
    let position_ms = ((curr - start) as u64 * 1000) / 44100;
    let duration_ms = ((end - start) as u64 * 1000) / 44100;
    let _ = data.event_tx.try_send(AirplayEvent::Progress {
        position_ms,
        duration_ms,
    });
}

/// Parse DMAP metadata buffer. Returns (title, artist, album).
fn parse_dmap(data: &[u8]) -> (String, String, String) {
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
            b"minm" => title = value.to_string(),  // dmap.itemname (title)
            b"asar" => artist = value.to_string(), // daap.songartist
            b"asal" => album = value.to_string(),  // daap.songalbum
            b"mlit" => {
                // Container — recurse into it
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
            _ => {} // Skip unknown tags
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
        // Construct a simple DMAP: minm + asar
        let mut data = Vec::new();
        // minm = "Test Song"
        data.extend_from_slice(b"minm");
        data.extend_from_slice(&9u32.to_be_bytes());
        data.extend_from_slice(b"Test Song");
        // asar = "Artist"
        data.extend_from_slice(b"asar");
        data.extend_from_slice(&6u32.to_be_bytes());
        data.extend_from_slice(b"Artist");

        let (title, artist, album) = parse_dmap(&data);
        assert_eq!(title, "Test Song");
        assert_eq!(artist, "Artist");
        assert_eq!(album, "");
    }
}
