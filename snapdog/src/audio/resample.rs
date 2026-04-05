// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Persistent audio resamplers for streaming PCM data.
//!
//! Two resampler variants:
//! - [`PcmResampler`] / [`Resampling`] — S16LE bytes in/out (for active sources: radio, Subsonic)
//! - [`F32Resampler`] / [`F32Resampling`] — F32 samples in/out (for receiver providers: AirPlay, Spotify)
//!
//! Both wrap rubato's SincFixedIn resampler with internal buffers to handle
//! arbitrary input chunk sizes. Filter state is maintained across calls
//! for artifact-free output.

use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

/// Rubato parameters shared by both resamplers.
fn sinc_params() -> SincInterpolationParameters {
    SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    }
}

const CHUNK_SIZE: usize = 1024;

// ── S16LE resampler (active sources) ──────────────────────────

/// Streaming resampler that accepts arbitrary-sized interleaved S16LE chunks.
pub struct PcmResampler {
    resampler: SincFixedIn<f64>,
    channels: usize,
    buffer: Vec<Vec<f64>>,
    chunk_size: usize,
}

impl PcmResampler {
    /// Create a new resampler. Returns `None` if source_rate == target_rate.
    pub fn new(source_rate: u32, target_rate: u32, channels: u16) -> Option<Self> {
        if source_rate == target_rate {
            return None;
        }

        let ch = channels as usize;
        let resampler = SincFixedIn::<f64>::new(
            target_rate as f64 / source_rate as f64,
            2.0,
            sinc_params(),
            CHUNK_SIZE,
            ch,
        )
        .map_err(|e| tracing::error!(error = %e, "Failed to create S16LE resampler"))
        .ok()?;

        tracing::info!(
            source_rate,
            target_rate,
            channels = ch,
            "S16LE resampler created"
        );

        Some(Self {
            resampler,
            channels: ch,
            buffer: vec![Vec::new(); ch],
            chunk_size: CHUNK_SIZE,
        })
    }

    /// Feed interleaved S16LE bytes, get back resampled interleaved S16LE bytes.
    pub fn process(&mut self, pcm: &[u8]) -> Vec<u8> {
        let samples: Vec<i16> = pcm
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect();

        let frames = samples.len() / self.channels;
        for frame in 0..frames {
            for ch in 0..self.channels {
                self.buffer[ch].push(samples[frame * self.channels + ch] as f64 / 32768.0);
            }
        }

        let mut output = Vec::new();
        while self.buffer[0].len() >= self.chunk_size {
            let chunk: Vec<Vec<f64>> = self
                .buffer
                .iter_mut()
                .map(|ch_buf| ch_buf.drain(..self.chunk_size).collect())
                .collect();

            let refs: Vec<&[f64]> = chunk.iter().map(|v| v.as_slice()).collect();
            match self.resampler.process(&refs, None) {
                Ok(resampled) => {
                    let out_frames = resampled[0].len();
                    for frame in 0..out_frames {
                        for ch in &resampled {
                            let sample = (ch[frame] * 32767.0).clamp(-32768.0, 32767.0) as i16;
                            output.extend_from_slice(&sample.to_le_bytes());
                        }
                    }
                }
                Err(e) => tracing::warn!(error = %e, "S16LE resample error, dropping chunk"),
            }
        }

        output
    }
}

/// Passthrough or resample S16LE.
pub enum Resampling {
    Passthrough,
    Active(PcmResampler),
}

impl Resampling {
    pub fn new(source_rate: u32, target_rate: u32, channels: u16) -> Self {
        match PcmResampler::new(source_rate, target_rate, channels) {
            Some(r) => Self::Active(r),
            None => Self::Passthrough,
        }
    }

    /// Returns resampled S16LE data, or `None` for passthrough / buffering.
    pub fn process(&mut self, pcm: &[u8]) -> Option<Vec<u8>> {
        match self {
            Self::Passthrough => None,
            Self::Active(r) => {
                let out = r.process(pcm);
                if out.is_empty() { None } else { Some(out) }
            }
        }
    }
}

// ── F32 resampler (receiver providers) ────────────────────────

/// Streaming resampler that operates on F32 interleaved samples.
///
/// Resamples in f32→f64→f32 precision without S16LE round-trips.
/// Used for receiver providers (AirPlay, Spotify Connect) that deliver F32 PCM.
pub struct F32Resampler {
    resampler: SincFixedIn<f64>,
    channels: usize,
    buffer: Vec<Vec<f64>>,
    chunk_size: usize,
}

impl F32Resampler {
    /// Create a new F32 resampler. Returns `None` if source_rate == target_rate.
    pub fn new(source_rate: u32, target_rate: u32, channels: u16) -> Option<Self> {
        if source_rate == target_rate {
            return None;
        }

        let ch = channels as usize;
        let resampler = SincFixedIn::<f64>::new(
            target_rate as f64 / source_rate as f64,
            2.0,
            sinc_params(),
            CHUNK_SIZE,
            ch,
        )
        .map_err(|e| tracing::error!(error = %e, "Failed to create F32 resampler"))
        .ok()?;

        tracing::info!(
            source_rate,
            target_rate,
            channels = ch,
            "F32 resampler created"
        );

        Some(Self {
            resampler,
            channels: ch,
            buffer: vec![Vec::new(); ch],
            chunk_size: CHUNK_SIZE,
        })
    }

    /// Feed F32 interleaved samples, get back resampled F32 interleaved samples.
    pub fn process(&mut self, samples: &[f32]) -> Vec<f32> {
        let frames = samples.len() / self.channels;
        for frame in 0..frames {
            for ch in 0..self.channels {
                self.buffer[ch].push(samples[frame * self.channels + ch] as f64);
            }
        }

        let mut output = Vec::new();
        while self.buffer[0].len() >= self.chunk_size {
            let chunk: Vec<Vec<f64>> = self
                .buffer
                .iter_mut()
                .map(|ch_buf| ch_buf.drain(..self.chunk_size).collect())
                .collect();

            let refs: Vec<&[f64]> = chunk.iter().map(|v| v.as_slice()).collect();
            match self.resampler.process(&refs, None) {
                Ok(resampled) => {
                    let out_frames = resampled[0].len();
                    for frame in 0..out_frames {
                        for ch in &resampled {
                            output.push(ch[frame] as f32);
                        }
                    }
                }
                Err(e) => tracing::warn!(error = %e, "F32 resample error, dropping chunk"),
            }
        }

        output
    }
}

/// Passthrough or resample F32.
pub enum F32Resampling {
    Passthrough,
    Active(F32Resampler),
}

impl F32Resampling {
    pub fn new(source_rate: u32, target_rate: u32, channels: u16) -> Self {
        match F32Resampler::new(source_rate, target_rate, channels) {
            Some(r) => Self::Active(r),
            None => Self::Passthrough,
        }
    }

    /// Returns resampled F32 data, or `None` for passthrough / buffering.
    pub fn process(&mut self, samples: &[f32]) -> Option<Vec<f32>> {
        match self {
            Self::Passthrough => None,
            Self::Active(r) => {
                let out = r.process(samples);
                if out.is_empty() { None } else { Some(out) }
            }
        }
    }
}

// ── Conversion ────────────────────────────────────────────────

/// Convert F32 interleaved samples to S16LE bytes.
#[inline]
pub fn f32_to_s16le(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        let clamped = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
        out.extend_from_slice(&clamped.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_when_rates_match() {
        let mut r = Resampling::new(48000, 48000, 2);
        assert!(r.process(&[1u8; 1024]).is_none());
    }

    #[test]
    fn creates_resampler_when_rates_differ() {
        assert!(matches!(
            Resampling::new(44100, 48000, 2),
            Resampling::Active(_)
        ));
    }

    #[test]
    fn none_when_rates_match() {
        assert!(PcmResampler::new(48000, 48000, 2).is_none());
    }

    #[test]
    fn resamples_s16le_after_enough_data() {
        let mut r = Resampling::new(44100, 48000, 2);
        let mut total = Vec::new();
        for _ in 0..8 {
            let pcm: Vec<u8> = (0..256)
                .flat_map(|i| {
                    let s = ((i as f64 * 440.0 * 2.0 * std::f64::consts::PI / 44100.0).sin()
                        * 16000.0) as i16;
                    let b = s.to_le_bytes();
                    [b[0], b[1], b[0], b[1]] // stereo
                })
                .collect();
            if let Some(out) = r.process(&pcm) {
                total.extend_from_slice(&out);
            }
        }
        assert!(!total.is_empty());
    }

    #[test]
    fn f32_passthrough_when_rates_match() {
        assert!(matches!(
            F32Resampling::new(48000, 48000, 2),
            F32Resampling::Passthrough
        ));
    }

    #[test]
    fn f32_resamples_after_enough_data() {
        let mut r = F32Resampling::new(44100, 48000, 2);
        let mut total = Vec::new();
        for _ in 0..8 {
            let samples: Vec<f32> = (0..512)
                .map(|i| (i as f32 * 440.0 * 2.0 * std::f32::consts::PI / 44100.0).sin() * 0.5)
                .collect();
            if let Some(out) = r.process(&samples) {
                total.extend_from_slice(&out);
            }
        }
        assert!(!total.is_empty());
    }

    #[test]
    fn f32_to_s16le_converts_correctly() {
        let bytes = f32_to_s16le(&[0.0, 1.0, -1.0, 0.5]);
        assert_eq!(bytes.len(), 8);
        assert_eq!(i16::from_le_bytes([bytes[0], bytes[1]]), 0);
        assert_eq!(i16::from_le_bytes([bytes[2], bytes[3]]), 32767);
        assert_eq!(i16::from_le_bytes([bytes[4], bytes[5]]), -32767);
        assert_eq!(i16::from_le_bytes([bytes[6], bytes[7]]), 16383);
    }
}
