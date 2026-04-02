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
///
/// Strategy:
/// - If the URL has a playlist extension (.m3u/.m3u8/.pls), fetch it directly.
/// - Otherwise, do a HEAD request first to check Content-Type without downloading the body.
/// - Handles relative URLs in playlists by resolving against the playlist's base URL.
/// - For HLS master playlists (.m3u8 with #EXT-X-STREAM-INF), extracts the highest bitrate variant.
/// - For HLS media playlists (segment lists), logs a warning — these need a proper HLS client.
async fn resolve_playlist_url(url: &str) -> Option<String> {
    let lower = url.to_lowercase();
    let ext_is_playlist =
        lower.ends_with(".m3u") || lower.ends_with(".m3u8") || lower.ends_with(".pls");

    let client = reqwest::Client::builder()
        .user_agent("SnapDog/1.0")
        .build()
        .ok()?;

    // For non-playlist extensions, do a HEAD request first to avoid downloading audio data
    if !ext_is_playlist {
        let head = client.head(url).send().await.ok()?;
        let ct = head
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_lowercase();
        let ct_is_playlist = ct.contains("mpegurl")
            || ct.contains("x-mpegurl")
            || ct.contains("vnd.apple.mpegurl")
            || ct.contains("x-scpls")
            || ct.contains("scpls");
        if !ct_is_playlist {
            return None; // Not a playlist — caller should use the original URL directly
        }
    }

    // Fetch the playlist body
    let response = client.get(url).send().await.ok()?.error_for_status().ok()?;
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    let is_pls = content_type.contains("x-scpls")
        || content_type.contains("scpls")
        || lower.ends_with(".pls");

    tracing::debug!(url, content_type, "Resolving playlist");
    let body = response.text().await.ok()?;

    // Parse the playlist base URL for resolving relative paths
    let base_url = url::Url::parse(url).ok()?;

    if is_pls {
        // PLS format: INI-style, look for File1=URL
        for line in body.lines() {
            let line = line.trim();
            if let Some(stream_url) = line
                .strip_prefix("File1=")
                .or_else(|| line.strip_prefix("file1="))
            {
                let resolved = resolve_relative(&base_url, stream_url.trim());
                tracing::info!(playlist = url, %resolved, "Resolved PLS playlist");
                return Some(resolved);
            }
        }
    } else if body.contains("#EXT-X-STREAM-INF") {
        // HLS master playlist — extract highest bitrate variant
        let resolved = resolve_hls_master(&base_url, &body);
        if let Some(ref r) = resolved {
            tracing::info!(playlist = url, resolved = %r, "Resolved HLS master playlist");
        }
        return resolved;
    } else if body.contains("#EXT-X-TARGETDURATION") || body.contains("#EXT-X-MEDIA-SEQUENCE") {
        // HLS media playlist (segment list) — can't handle without HLS client
        tracing::warn!(
            url,
            "HLS media playlist detected — SnapDog does not support HLS segment streaming"
        );
        return None;
    } else {
        // M3U: first non-empty, non-comment line
        for line in body.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                let resolved = resolve_relative(&base_url, line);
                tracing::info!(playlist = url, %resolved, "Resolved M3U playlist");
                return Some(resolved);
            }
        }
    }

    tracing::warn!(url, "Playlist contained no stream URLs");
    None
}

/// Resolve a potentially relative URL against a base URL.
fn resolve_relative(base: &url::Url, target: &str) -> String {
    if target.starts_with("http://") || target.starts_with("https://") {
        target.to_string()
    } else {
        base.join(target)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| target.to_string())
    }
}

/// Extract the highest bitrate variant URL from an HLS master playlist.
fn resolve_hls_master(base: &url::Url, body: &str) -> Option<String> {
    let mut best_bandwidth: u64 = 0;
    let mut best_url: Option<String> = None;
    let mut next_is_url = false;

    for line in body.lines() {
        let line = line.trim();
        if line.starts_with("#EXT-X-STREAM-INF") {
            // Parse BANDWIDTH=nnn from the tag
            if let Some(bw_str) = line.split(',').find_map(|attr| {
                let attr = attr.trim();
                attr.strip_prefix("BANDWIDTH=")
                    .or_else(|| attr.strip_prefix("#EXT-X-STREAM-INF:BANDWIDTH="))
            }) {
                if let Ok(bw) = bw_str.trim().parse::<u64>() {
                    if bw > best_bandwidth {
                        best_bandwidth = bw;
                        next_is_url = true;
                        continue;
                    }
                }
            }
            next_is_url = true;
        } else if next_is_url && !line.is_empty() && !line.starts_with('#') {
            if best_bandwidth > 0 || best_url.is_none() {
                best_url = Some(resolve_relative(base, line));
            }
            next_is_url = false;
            best_bandwidth = 0; // Reset so we only update on higher bandwidth
        }
    }

    best_url
}
