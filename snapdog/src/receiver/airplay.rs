// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! AirPlay 1 + 2 receiver implementing [`ReceiverProvider`].
//!
//! Bridges the [`shairplay`] crate's callback-based API into SnapDog's
//! channel-based receiver model. Audio is delivered as F32 interleaved PCM.

// TODO: Move implementation from airplay/mod.rs here
