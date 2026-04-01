// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! ZonePlayer — per-zone audio pipeline with command channel.

mod commands;
pub mod context;
mod helpers;
mod runner;

pub use commands::ZoneCommand;
pub use context::{SnapcastCmd, SnapcastCmdSender, ZoneCommandSender, ZonePlayerContext};
pub use runner::spawn_zone_players;
