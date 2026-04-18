// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! `knx-core` — platform-independent KNX protocol types.
//!
//! This crate provides the foundational types for the KNX protocol stack:
//!
//! - [`address`] — Individual and group address types
//! - [`types`] — Frame-level protocol enums (priority, format, medium)
//! - [`message`] — CEMI message codes and APDU/TPDU service types
//! - [`device`] — Device management types (restart, erase, security, return codes)
//! - [`cemi`] — Common External Message Interface frame parsing and serialization
//! - [`tpdu`] — Transport Protocol Data Unit
//! - [`apdu`] — Application Protocol Data Unit
//!
//! # `no_std` Support
//!
//! This crate is `no_std`-compatible by default. Enable the `std` feature
//! for `std`-dependent functionality.

#![no_std]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

extern crate alloc;

pub mod address;
pub mod apdu;
pub mod cemi;
pub mod device;
pub mod dpt;
pub mod knxip;
pub mod message;
pub mod tpdu;
pub mod types;

#[cfg(test)]
mod cemi_tests;
