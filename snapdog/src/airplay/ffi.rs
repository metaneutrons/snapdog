// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Raw FFI bindings to libshairplay (vendored C library).

#![allow(non_camel_case_types, dead_code)]

use std::ffi::{c_char, c_float, c_int, c_uint, c_void};

// ── RAOP ──────────────────────────────────────────────────────

pub enum raop_s {}
pub type raop_t = raop_s;

#[repr(C)]
pub struct raop_callbacks_t {
    pub cls: *mut c_void,
    pub audio_init: Option<unsafe extern "C" fn(*mut c_void, c_int, c_int, c_int) -> *mut c_void>,
    pub audio_process: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, c_int)>,
    pub audio_destroy: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    pub audio_flush: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    pub audio_set_volume: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, c_float)>,
    pub audio_set_metadata:
        Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, c_int)>,
    pub audio_set_coverart:
        Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, c_int)>,
    pub audio_remote_control_id:
        Option<unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char)>,
    pub audio_set_progress:
        Option<unsafe extern "C" fn(*mut c_void, *mut c_void, c_uint, c_uint, c_uint)>,
}

unsafe extern "C" {
    pub fn raop_init(
        max_clients: c_int,
        callbacks: *mut raop_callbacks_t,
        pemkey: *const c_char,
        error: *mut c_int,
    ) -> *mut raop_t;

    pub fn raop_set_log_level(raop: *mut raop_t, level: c_int);

    pub fn raop_start(
        raop: *mut raop_t,
        port: *mut u16,
        hwaddr: *const c_char,
        hwaddrlen: c_int,
        password: *const c_char,
    ) -> c_int;

    pub fn raop_is_running(raop: *mut raop_t) -> c_int;
    pub fn raop_stop(raop: *mut raop_t);
    pub fn raop_destroy(raop: *mut raop_t);
}

// ── DNS-SD ────────────────────────────────────────────────────

pub enum dnssd_s {}
pub type dnssd_t = dnssd_s;

unsafe extern "C" {
    pub fn dnssd_init(error: *mut c_int) -> *mut dnssd_t;

    pub fn dnssd_register_raop(
        dnssd: *mut dnssd_t,
        name: *const c_char,
        port: u16,
        hwaddr: *const c_char,
        hwaddrlen: c_int,
        password: c_int,
    ) -> c_int;

    pub fn dnssd_unregister_raop(dnssd: *mut dnssd_t);
    pub fn dnssd_destroy(dnssd: *mut dnssd_t);
}
