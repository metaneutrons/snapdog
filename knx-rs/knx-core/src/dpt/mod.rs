// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! KNX Datapoint Type (DPT) framework.
//!
//! Provides the [`Dpt`] identifier, encoding/decoding traits, and concrete
//! implementations for the most common datapoint types.
//!
//! # Architecture
//!
//! Each DPT main group has a dedicated encode/decode function pair dispatched
//! by [`decode`] and [`encode`]. The [`Dpt`] struct identifies a specific
//! datapoint type by main group, sub group, and index.

mod convert;

use alloc::vec::Vec;
use core::fmt;

/// A KNX Datapoint Type identifier (main group / sub group / index).
///
/// Matches the C++ `Dpt` class. The main group determines the wire encoding
/// size and format; the sub group selects the semantic interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dpt {
    /// Main group number (e.g. 1 for boolean, 9 for 16-bit float).
    pub main: u16,
    /// Sub group number (e.g. 1 for DPT 1.001 Switch).
    pub sub: u16,
    /// Index (used by DPT 10.001 `TimeOfDay`, usually 0).
    pub index: u16,
}

impl Dpt {
    /// Create a new DPT identifier.
    pub const fn new(main: u16, sub: u16) -> Self {
        Self {
            main,
            sub,
            index: 0,
        }
    }

    /// Create a new DPT identifier with index.
    pub const fn with_index(main: u16, sub: u16, index: u16) -> Self {
        Self { main, sub, index }
    }

    /// Wire data length in bytes for this DPT's main group.
    ///
    /// Matches the C++ `Dpt::dataLength()` implementation.
    pub const fn data_length(self) -> u8 {
        match self.main {
            7 | 8 | 9 | 22 | 207 | 217 | 234 | 237 | 244 | 246 => 2,
            10 | 11 | 30 | 206 | 225 | 232 | 240 | 250 | 254 => 3,
            12 | 13 | 14 | 15 | 27 | 241 | 251 => 4,
            252 => 5,
            219 | 222 | 229 | 235 | 242 | 245 | 249 => 6,
            19 | 29 | 230 | 255 | 275 => 8,
            16 => 14,
            285 => 16,
            _ => 1,
        }
    }
}

impl fmt::Display for Dpt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.index == 0 {
            write!(f, "{}.{:03}", self.main, self.sub)
        } else {
            write!(f, "{}.{:03}.{}", self.main, self.sub, self.index)
        }
    }
}

/// Error returned when DPT encoding or decoding fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DptError {
    /// The payload is too short for the requested DPT.
    PayloadTooShort,
    /// The DPT main group is not supported.
    UnsupportedDpt(Dpt),
    /// The value is out of range for the requested DPT.
    OutOfRange,
}

impl fmt::Display for DptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PayloadTooShort => f.write_str("payload too short for DPT"),
            Self::UnsupportedDpt(dpt) => write!(f, "unsupported DPT: {dpt}"),
            Self::OutOfRange => f.write_str("value out of range for DPT"),
        }
    }
}

impl core::error::Error for DptError {}

/// Decode a KNX bus payload into an `f64` value using the given DPT.
///
/// # Errors
///
/// Returns [`DptError`] if the payload is too short or the DPT is unsupported.
pub fn decode(dpt: Dpt, payload: &[u8]) -> Result<f64, DptError> {
    convert::decode(dpt, payload)
}

/// Encode an `f64` value into a KNX bus payload using the given DPT.
///
/// # Errors
///
/// Returns [`DptError`] if the value is out of range or the DPT is unsupported.
pub fn encode(dpt: Dpt, value: f64) -> Result<Vec<u8>, DptError> {
    convert::encode(dpt, value)
}

// ── Well-known DPT constants ──────────────────────────────────

/// DPT 1.001 — Switch (bool).
pub const DPT_SWITCH: Dpt = Dpt::new(1, 1);
/// DPT 1.002 — Bool.
pub const DPT_BOOL: Dpt = Dpt::new(1, 2);
/// DPT 5.001 — Scaling (0–100%).
pub const DPT_SCALING: Dpt = Dpt::new(5, 1);
/// DPT 5.003 — Angle (0–360°).
pub const DPT_ANGLE: Dpt = Dpt::new(5, 3);
/// DPT 5.010 — Unsigned count (0–255).
pub const DPT_VALUE_1_UCOUNT: Dpt = Dpt::new(5, 10);
/// DPT 9.001 — Temperature (°C), 16-bit float.
pub const DPT_VALUE_TEMP: Dpt = Dpt::new(9, 1);
/// DPT 9.004 — Lux, 16-bit float.
pub const DPT_VALUE_LUX: Dpt = Dpt::new(9, 4);
/// DPT 14.056 — Power (W), 32-bit float.
pub const DPT_VALUE_POWER: Dpt = Dpt::new(14, 56);
/// DPT 16.000 — ASCII string (14 bytes).
pub const DPT_STRING_ASCII: Dpt = Dpt::new(16, 0);
/// DPT 16.001 — ISO 8859-1 string (14 bytes).
pub const DPT_STRING_8859_1: Dpt = Dpt::new(16, 1);
