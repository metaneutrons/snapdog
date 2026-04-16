// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Client-side parametric EQ — biquad filters applied in the cpal callback.

#[allow(dead_code)]
pub const TYPE_EQ_CONFIG: u16 = 10;

use biquad::{Biquad, Coefficients, DirectForm2Transposed, Hertz, Q_BUTTERWORTH_F32, ToHertz};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FilterType {
    LowShelf,
    HighShelf,
    Peaking,
    LowPass,
    HighPass,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqBand {
    pub freq: f32,
    pub gain: f32,
    pub q: f32,
    #[serde(rename = "type")]
    pub filter_type: FilterType,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EqConfig {
    pub enabled: bool,
    pub bands: Vec<EqBand>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
}

/// Per-client EQ processor. Owns biquad filter instances for each band × channel.
pub struct ZoneEq {
    bands: Vec<Vec<DirectForm2Transposed<f32>>>,
    enabled: bool,
    sample_rate: f32,
    channels: usize,
}

impl ZoneEq {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            bands: vec![],
            enabled: false,
            sample_rate: sample_rate as f32,
            channels: channels as usize,
        }
    }

    pub fn set_config(&mut self, config: &EqConfig) {
        self.enabled = config.enabled;
        self.bands = config
            .bands
            .iter()
            .filter_map(|b| self.make_band(b))
            .collect();
    }

    /// Process interleaved f32 samples in-place.
    pub fn process(&mut self, samples: &mut [f32]) {
        if !self.enabled || self.bands.is_empty() {
            return;
        }
        let ch = self.channels;
        let frames = samples.len() / ch;
        for frame in 0..frames {
            for band in &mut self.bands {
                for (c, filter) in band.iter_mut().enumerate() {
                    let idx = frame * ch + c;
                    samples[idx] = filter.run(samples[idx]);
                }
            }
        }
    }

    fn make_band(&self, band: &EqBand) -> Option<Vec<DirectForm2Transposed<f32>>> {
        let fs: Hertz<f32> = self.sample_rate.hz();
        let f0: Hertz<f32> = band.freq.hz();
        let q = if band.q > 0.0 {
            band.q
        } else {
            Q_BUTTERWORTH_F32
        };
        let filter_type = match band.filter_type {
            FilterType::LowShelf => biquad::Type::LowShelf(band.gain),
            FilterType::HighShelf => biquad::Type::HighShelf(band.gain),
            FilterType::Peaking => biquad::Type::PeakingEQ(band.gain),
            FilterType::LowPass => biquad::Type::LowPass,
            FilterType::HighPass => biquad::Type::HighPass,
        };
        let coeffs = Coefficients::<f32>::from_params(filter_type, fs, f0, q).ok()?;
        Some(
            (0..self.channels)
                .map(|_| DirectForm2Transposed::<f32>::new(coeffs))
                .collect(),
        )
    }
}
