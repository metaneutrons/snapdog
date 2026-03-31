// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Persistent audio resampler for streaming PCM data.
//!
//! Wraps rubato's SincFixedIn resampler with an internal buffer to handle
//! arbitrary input chunk sizes. The resampler maintains filter state across
//! calls for artifact-free output.

use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

/// Streaming resampler that accepts arbitrary-sized interleaved S16LE chunks.
pub struct PcmResampler {
    resampler: SincFixedIn<f64>,
    channels: usize,
    buffer: Vec<Vec<f64>>, // Per-channel accumulation buffer
    chunk_size: usize,     // Frames needed per process() call
}

impl PcmResampler {
    /// Create a new resampler. Returns `None` if source_rate == target_rate.
    pub fn new(source_rate: u32, target_rate: u32, channels: u16) -> Option<Self> {
        if source_rate == target_rate {
            return None;
        }

        let ch = channels as usize;
        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };

        let chunk_size = 1024;
        let resampler = SincFixedIn::<f64>::new(
            target_rate as f64 / source_rate as f64,
            2.0, // max relative ratio (headroom)
            params,
            chunk_size,
            ch,
        )
        .expect("Failed to create resampler");

        let buffer = vec![Vec::new(); ch];

        tracing::info!(
            source_rate,
            target_rate,
            channels = ch,
            chunk_size,
            "Resampler created"
        );

        Some(Self {
            resampler,
            channels: ch,
            buffer,
            chunk_size,
        })
    }

    /// Feed interleaved S16LE bytes, get back resampled interleaved S16LE bytes.
    /// May return empty Vec if not enough data accumulated yet.
    pub fn process(&mut self, pcm: &[u8]) -> Vec<u8> {
        // Deinterleave S16LE → per-channel f64
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

        // Process complete chunks
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
                Err(e) => {
                    tracing::warn!(error = %e, "Resample error, dropping chunk");
                }
            }
        }

        output
    }
}

/// Passthrough or resample: wraps Option<PcmResampler> for ergonomic use.
pub enum Resampling {
    Passthrough,
    Active(PcmResampler),
}

impl Resampling {
    /// Create from source and target rates. Passthrough if rates match.
    pub fn new(source_rate: u32, target_rate: u32, channels: u16) -> Self {
        match PcmResampler::new(source_rate, target_rate, channels) {
            Some(r) => Self::Active(r),
            None => Self::Passthrough,
        }
    }

    /// Process a PCM chunk. Returns the data unchanged if passthrough.
    pub fn process(&mut self, pcm: &[u8]) -> Vec<u8> {
        match self {
            Self::Passthrough => pcm.to_vec(),
            Self::Active(r) => {
                let out = r.process(pcm);
                if out.is_empty() {
                    // Not enough data accumulated yet — return empty
                    // The caller should handle this (skip TCP write)
                    Vec::new()
                } else {
                    out
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passthrough_when_rates_match() {
        let mut r = Resampling::new(48000, 48000, 2);
        let pcm = vec![1u8; 1024];
        assert_eq!(r.process(&pcm), pcm);
    }

    #[test]
    fn creates_resampler_when_rates_differ() {
        let r = Resampling::new(44100, 48000, 2);
        assert!(matches!(r, Resampling::Active(_)));
    }

    #[test]
    fn resamples_after_enough_data() {
        let mut r = Resampling::new(44100, 48000, 2);
        // Feed enough data for at least one chunk (1024 frames * 2 channels * 2 bytes)
        let mut total_output = Vec::new();
        for _ in 0..8 {
            let mut pcm = Vec::new();
            for i in 0..256 {
                let sample = ((i as f64 * 440.0 * 2.0 * std::f64::consts::PI / 44100.0).sin()
                    * 16000.0) as i16;
                pcm.extend_from_slice(&sample.to_le_bytes()); // L
                pcm.extend_from_slice(&sample.to_le_bytes()); // R
            }
            let out = r.process(&pcm);
            total_output.extend_from_slice(&out);
        }
        // After 8 * 256 = 2048 frames, we should have output
        assert!(
            !total_output.is_empty(),
            "Should have produced output after 2048 frames"
        );
    }

    #[test]
    fn none_when_rates_match() {
        assert!(PcmResampler::new(48000, 48000, 2).is_none());
    }
}
