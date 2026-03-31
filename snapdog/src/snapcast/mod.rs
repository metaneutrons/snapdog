// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Snapcast integration: server lifecycle, control API, audio source feeding.
//!
//! - Generates snapserver.conf from app config
//! - Manages snapserver as child process
//! - JSON-RPC control via `snapcast-control` crate
//! - Feeds PCM audio to TCP sources (loopback-only)
