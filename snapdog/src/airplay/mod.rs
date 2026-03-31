// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! AirPlay 1 (RAOP) receiver via libshairplay FFI.
//!
//! Safe Rust wrappers around libshairplay's C API.
//! Receives PCM audio + metadata from AirPlay clients.

mod ffi;

use std::ffi::CString;
use std::sync::Arc;

use anyhow::{Context, Result};

use crate::audio::PcmSender;
use crate::config::AirplayConfig;

/// AirPlay receiver. Runs RAOP server + mDNS advertisement.
pub struct AirplayReceiver {
    raop: *mut ffi::raop_t,
    dnssd: *mut ffi::dnssd_t,
    _callback_data: Arc<CallbackData>,
}

// SAFETY: The C library handles its own threading. We only call start/stop from one thread.
unsafe impl Send for AirplayReceiver {}

struct CallbackData {
    pcm_tx: PcmSender,
}

impl AirplayReceiver {
    /// Start the AirPlay receiver. PCM audio is sent to `pcm_tx`.
    #[tracing::instrument(skip(pcm_tx))]
    pub fn start(config: &AirplayConfig, pcm_tx: PcmSender) -> Result<Self> {
        let callback_data = Arc::new(CallbackData { pcm_tx });
        let cls = Arc::as_ptr(&callback_data) as *mut std::ffi::c_void;

        // Read the airport key
        let pemkey = include_str!("../../../vendor/shairplay/airport.key");
        let pemkey_c = CString::new(pemkey).context("Invalid PEM key")?;

        // Set up callbacks
        let callbacks = ffi::raop_callbacks_t {
            cls,
            audio_init: Some(cb_audio_init),
            audio_process: Some(cb_audio_process),
            audio_destroy: Some(cb_audio_destroy),
            audio_flush: Some(cb_audio_flush),
            audio_set_volume: Some(cb_audio_set_volume),
            audio_set_metadata: None,
            audio_set_coverart: None,
            audio_remote_control_id: None,
            audio_set_progress: None,
        };

        // Init RAOP
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

        unsafe { ffi::raop_set_log_level(raop, 6) }; // INFO

        // Start RAOP server
        let hwaddr: [u8; 6] = [0x02, 0x42, 0xAA, 0xBB, 0xCC, 0xDD];
        let mut port: u16 = 5000;
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
        if ret != 0 {
            unsafe { ffi::raop_destroy(raop) };
            anyhow::bail!("raop_start failed with error {ret}");
        }

        // Init DNS-SD and register
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
        let has_password = if config.password.is_some() { 1 } else { 0 };
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

    /// Check if the RAOP server is running.
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
    std::ptr::null_mut() // session pointer — unused for now
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
    // Non-blocking send — drop samples if channel is full
    let _ = data.pcm_tx.try_send(pcm.to_vec());
}

unsafe extern "C" fn cb_audio_destroy(
    _cls: *mut std::ffi::c_void,
    _session: *mut std::ffi::c_void,
) {
    tracing::info!("AirPlay audio session ended");
}

unsafe extern "C" fn cb_audio_flush(_cls: *mut std::ffi::c_void, _session: *mut std::ffi::c_void) {
    tracing::debug!("AirPlay audio flush");
}

unsafe extern "C" fn cb_audio_set_volume(
    _cls: *mut std::ffi::c_void,
    _session: *mut std::ffi::c_void,
    volume: f32,
) {
    // AirPlay volume is -144.0 (mute) to 0.0 (max)
    let percent = if volume <= -144.0 {
        0
    } else {
        ((volume + 30.0) / 30.0 * 100.0) as i32
    };
    tracing::info!(raw = volume, percent, "AirPlay volume changed");
}
