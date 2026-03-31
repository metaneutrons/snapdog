// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! ZonePlayer — per-zone audio pipeline with command channel.
//!
//! Each zone runs an independent ZonePlayer task that owns its audio pipeline,
//! manages source switching, and forwards PCM to its Snapcast TCP source.

mod commands;
mod runner;

pub use commands::ZoneCommand;
pub use runner::{ZoneCommandSender, spawn_zone_players};
