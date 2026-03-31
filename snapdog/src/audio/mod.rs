// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Audio decoding and PCM pipeline.
//!
//! Decodes AAC, MP3, FLAC, ALAC via symphonia into raw PCM,
//! then routes PCM to the appropriate Snapcast TCP source.
