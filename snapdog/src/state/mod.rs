// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Application state management.
//!
//! In-memory state for zones, clients, playback.
//! Persisted to JSON file on state changes (atomic write + rename).
