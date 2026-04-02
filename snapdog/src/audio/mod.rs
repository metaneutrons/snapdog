// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Audio decoding and PCM pipeline.
//!
//! Fetches HTTP audio streams, decodes via symphonia to raw PCM (S16LE),
//! and sends PCM chunks to a consumer (Snapcast TCP source).

pub mod icy;
pub mod resample;

use std::io::{Read, Seek};

use anyhow::{Context, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use tokio::sync::mpsc;

use crate::config::AudioConfig;

/// PCM output: interleaved S16LE bytes.
pub type PcmSender = mpsc::Sender<Vec<u8>>;
pub type PcmReceiver = mpsc::Receiver<Vec<u8>>;

/// Create a PCM channel pair.
pub fn pcm_channel(buffer: usize) -> (PcmSender, PcmReceiver) {
    mpsc::channel(buffer)
}

/// Decode an HTTP audio stream to PCM and send chunks to the provided sender.
/// Runs until the stream ends or the sender is dropped.
/// Returns an optional ICY metadata receiver for live title updates.
#[tracing::instrument(skip(tx, audio_config, icy_meta_tx))]
pub async fn decode_http_stream(
    url: String,
    tx: PcmSender,
    audio_config: AudioConfig,
    icy_meta_tx: Option<tokio::sync::mpsc::Sender<icy::IcyMetadata>>,
) -> Result<()> {
    // Resolve playlist URLs (.m3u/.m3u8/.pls) to the actual stream URL
    let url = resolve_playlist_url(&url).await.unwrap_or(url);

    // Use ICY-aware client to request metadata
    let client = icy::icy_client();
    let response = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("Failed to fetch {url}"))?
        .error_for_status()
        .with_context(|| format!("HTTP error for {url}"))?;

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    // Set up ICY metadata stripping if server supports it
    let metaint = icy::parse_metaint(&response);
    let mut icy_processor =
        metaint.and_then(|mi| icy_meta_tx.map(|tx| icy::IcyProcessor::new(mi, tx)));

    if metaint.is_some() {
        tracing::info!(content_type = %content_type, metaint = ?metaint, "Stream connected (ICY enabled)");
    } else {
        tracing::info!(content_type = %content_type, "Stream connected");
    }

    // MP4/M4A containers need seeking — buffer entire response first
    let needs_seek = content_type.contains("mp4") || content_type.contains("m4a");
    if needs_seek {
        let bytes = response
            .bytes()
            .await
            .context("Failed to read MP4 stream body")?;
        tracing::debug!(
            size = bytes.len(),
            "Buffered MP4 stream for seekable decode"
        );
        let cursor = std::io::Cursor::new(bytes.to_vec());
        tokio::task::spawn_blocking(move || {
            decode_to_pcm(cursor, &content_type, tx, &audio_config)
        })
        .await
        .context("Decoder task panicked")??;
        return Ok(());
    }

    // Collect the async byte stream into a sync reader via a pipe
    let (mut pipe_tx, pipe_rx) = tokio::io::duplex(64 * 1024);

    // Task: read HTTP chunks, strip ICY metadata, write audio to pipe
    let url_clone = url.clone();
    let http_task = tokio::spawn(async move {
        use futures_util::StreamExt;
        use tokio::io::AsyncWriteExt;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    let audio = if let Some(ref mut proc) = icy_processor {
                        proc.process(bytes)
                    } else {
                        bytes.to_vec()
                    };
                    if !audio.is_empty() && pipe_tx.write_all(&audio).await.is_err() {
                        break; // Decoder closed
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, url = %url_clone, "Stream read error");
                    break;
                }
            }
        }
    });

    // Decode in blocking thread (symphonia is sync + CPU-bound)
    let decode_task = tokio::task::spawn_blocking(move || {
        let reader = SyncReader(tokio::runtime::Handle::current(), pipe_rx);
        decode_to_pcm(reader, &content_type, tx, &audio_config)
    });

    // Wait for either task to finish
    tokio::select! {
        _ = http_task => tracing::debug!("HTTP stream ended"),
        result = decode_task => {
            result.context("Decoder task panicked")??;
        }
    }

    Ok(())
}

/// Synchronous symphonia decoding loop.
fn decode_to_pcm(
    reader: impl MediaSource + 'static,
    content_type: &str,
    tx: PcmSender,
    audio_config: &AudioConfig,
) -> Result<()> {
    let mut hint = Hint::new();
    match content_type {
        t if t.contains("mp4") || t.contains("m4a") => hint.with_extension("m4a"),
        t if t.contains("aac") => hint.with_extension("aac"),
        t if t.contains("mpeg") || t.contains("mp3") => hint.with_extension("mp3"),
        t if t.contains("flac") => hint.with_extension("flac"),
        t if t.contains("ogg") => hint.with_extension("ogg"),
        _ => &mut hint,
    };

    let mss = MediaSourceStream::new(Box::new(reader), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("Failed to probe audio format")?;

    let mut format = probed.format;
    let track = format.default_track().context("No audio track found")?;
    let track_id = track.id;

    tracing::info!(
        codec = ?track.codec_params.codec,
        sample_rate = track.codec_params.sample_rate,
        channels = ?track.codec_params.channels,
        "Decoding audio"
    );

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("Failed to create decoder")?;

    let _target_rate = audio_config.sample_rate;
    let _target_channels = audio_config.channels;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                tracing::debug!("Stream ended (EOF)");
                break;
            }
            Err(e) => {
                tracing::warn!(error = %e, "Packet read error, skipping");
                continue;
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(error = %e, "Decode error, skipping packet");
                continue;
            }
        };

        // Convert to interleaved S16LE
        let spec = *decoded.spec();
        let num_frames = decoded.frames();
        let mut sample_buf = SampleBuffer::<i16>::new(num_frames as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);

        let samples = sample_buf.samples();
        let mut bytes = Vec::with_capacity(samples.len() * 2);
        for s in samples {
            bytes.extend_from_slice(&s.to_le_bytes());
        }

        if tx.blocking_send(bytes).is_err() {
            tracing::debug!("PCM consumer dropped, stopping decode");
            break;
        }
    }

    // Note: resampling is handled by the ZonePlayer, not the decoder

    Ok(())
}

/// Bridge from async tokio DuplexStream to sync Read for symphonia.
struct SyncReader(tokio::runtime::Handle, tokio::io::DuplexStream);

impl Read for SyncReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        use tokio::io::AsyncReadExt;
        self.0.block_on(self.1.read(buf))
    }
}

impl Seek for SyncReader {
    fn seek(&mut self, _pos: std::io::SeekFrom) -> std::io::Result<u64> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "not seekable",
        ))
    }
}

impl MediaSource for SyncReader {
    fn is_seekable(&self) -> bool {
        false
    }

    fn byte_len(&self) -> Option<u64> {
        None
    }
}

/// Resolve playlist URLs (.m3u/.m3u8/.pls) to the actual stream URL.
/// Detects playlists by Content-Type first, then falls back to file extension.
async fn resolve_playlist_url(url: &str) -> Option<String> {
    // Quick check: skip obvious non-playlists by extension
    let lower = url.to_lowercase();
    let _ext_hint = lower.ends_with(".m3u") || lower.ends_with(".m3u8") || lower.ends_with(".pls");

    // If extension doesn't hint at playlist, do a HEAD request to check Content-Type
    let response = reqwest::get(url).await.ok()?;
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    let is_m3u = content_type.contains("mpegurl")
        || content_type.contains("x-mpegurl")
        || content_type.contains("vnd.apple.mpegurl")
        || lower.ends_with(".m3u")
        || lower.ends_with(".m3u8");

    let is_pls = content_type.contains("x-scpls")
        || content_type.contains("scpls")
        || lower.ends_with(".pls");

    if !is_m3u && !is_pls {
        return None;
    }

    tracing::debug!(url, content_type, "Resolving playlist");
    let body = response.text().await.ok()?;

    if is_pls {
        // PLS format: INI-style, look for File1=URL
        for line in body.lines() {
            let line = line.trim();
            if let Some(stream_url) = line
                .strip_prefix("File1=")
                .or_else(|| line.strip_prefix("file1="))
            {
                let resolved = stream_url.trim().to_string();
                tracing::info!(playlist = url, %resolved, "Resolved PLS playlist");
                return Some(resolved);
            }
        }
    } else {
        // M3U/M3U8: first non-empty, non-comment line
        for line in body.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                tracing::info!(playlist = url, resolved = %line, "Resolved M3U playlist");
                return Some(line.to_string());
            }
        }
    }

    tracing::warn!(url, "Playlist contained no stream URLs");
    None
}
