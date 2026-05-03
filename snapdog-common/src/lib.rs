// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! Shared types and constants for SnapDog server and client.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use serde::{Deserialize, Serialize};

/// Client name used by SnapDog clients to identify themselves to the server.
pub const CLIENT_NAME: &str = "SnapDog";

/// Snapcast custom message type ID for EQ configuration.
pub const MSG_TYPE_EQ_CONFIG: u16 = 10;

/// Snapcast custom message type ID for speaker correction EQ.
pub const MSG_TYPE_SPEAKER_EQ: u16 = 11;

/// Maximum number of EQ bands per zone/client.
pub const MAX_EQ_BANDS: usize = 10;

// ── EQ types ──────────────────────────────────────────────────

/// Filter type for an EQ band.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FilterType {
    /// Boosts or cuts frequencies below the cutoff.
    LowShelf,
    /// Boosts or cuts frequencies above the cutoff.
    HighShelf,
    /// Boosts or cuts a narrow band around the center frequency.
    Peaking,
    /// Passes frequencies below the cutoff, attenuates above.
    LowPass,
    /// Passes frequencies above the cutoff, attenuates below.
    HighPass,
}

/// Single EQ band configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqBand {
    /// Center frequency in Hz.
    pub freq: f32,
    /// Gain in dB (positive = boost, negative = cut). Ignored for low/high pass.
    pub gain: f32,
    /// Q factor controlling bandwidth. Higher values = narrower band.
    pub q: f32,
    /// Filter type (low shelf, high shelf, peaking, low pass, high pass).
    #[serde(rename = "type")]
    pub filter_type: FilterType,
}

/// Full EQ configuration for a zone.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqConfig {
    /// Whether the EQ is active. When `false`, audio passes through unmodified.
    pub enabled: bool,
    /// Ordered list of biquad filter bands applied in series.
    pub bands: Vec<EqBand>,
    /// Name of the preset this config was loaded from, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
}

impl Default for EqConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bands: vec![],
            preset: Some("flat".into()),
        }
    }
}

// ── Volume ────────────────────────────────────────────────────

/// Perceptual (quadratic) volume curve: maps linear 0–100 to 0.0–1.0.
/// Input: linear percentage (0–100). Output: gain factor (0.0–1.0).
pub fn perceptual_volume(linear: u8) -> f32 {
    let normalized = linear as f32 / 100.0;
    normalized * normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_curve_boundaries() {
        assert_eq!(perceptual_volume(0), 0.0);
        assert_eq!(perceptual_volume(100), 1.0);
    }

    #[test]
    fn volume_curve_midpoint() {
        let mid = perceptual_volume(50);
        assert!((mid - 0.25).abs() < 0.001);
    }

    #[test]
    fn eq_config_default() {
        let config = EqConfig::default();
        assert!(!config.enabled);
        assert!(config.bands.is_empty());
        assert_eq!(config.preset, Some("flat".into()));
    }
}
