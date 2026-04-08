// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! SnapDog library — re-exports all modules for integration tests.

#![forbid(unsafe_code)]
#![warn(clippy::redundant_closure)]
#![warn(clippy::implicit_clone)]
#![warn(clippy::uninlined_format_args)]
// TODO: enable once public API is documented
#![warn(missing_docs)]

pub mod api;
pub mod audio;
pub mod config;
pub mod knx;
pub mod mqtt;
pub mod player;
pub mod process;
pub mod receiver;
pub mod snapcast;
pub mod state;
pub mod subsonic;
