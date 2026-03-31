// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Configuration loading and validation.
//!
//! Single TOML file → all derived config (KNX addresses, sink paths, snapserver.conf).
//! Convention over configuration: sensible defaults, auto-generated where possible.
