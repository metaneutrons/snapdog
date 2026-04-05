// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Spotify Connect receiver implementing [`ReceiverProvider`].
//!
//! Uses [`librespot`] for Zeroconf discovery, Spotify Connect protocol,
//! and audio decoding. Audio is delivered as F32 interleaved PCM via a
//! custom sink that writes to the receiver's audio channel.

use std::sync::Arc;

use anyhow::Result;
use librespot_connect::config::ConnectConfig;
use librespot_connect::spirc::Spirc;
use librespot_core::config::{DeviceType, SessionConfig};
use librespot_core::session::Session;
use librespot_discovery::Discovery;
use librespot_playback::audio_backend::{Open, Sink, SinkAsBytes, SinkError, SinkResult};
use librespot_playback::config::{AudioFormat as LsAudioFormat, PlayerConfig};
use librespot_playback::convert::Converter;
use librespot_playback::decoder::AudioPacket;
use librespot_playback::mixer::NoOpVolume;
use librespot_playback::player::{Player, PlayerEvent};
use tokio::sync::mpsc;

use super::{AudioFormat, AudioSender, ReceiverEvent, ReceiverEventTx, ReceiverProvider};
use crate::config::SpotifyConfig;

// ── SpotifyReceiver ───────────────────────────────────────────

/// Spotify Connect receiver wrapping librespot.
pub struct SpotifyReceiver {
    config: SpotifyConfig,
    zone_index: usize,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl SpotifyReceiver {
    /// Create a new (stopped) Spotify Connect receiver for the given zone.
    pub fn new(config: SpotifyConfig, zone_index: usize) -> Self {
        Self {
            config,
            zone_index,
            task: None,
        }
    }
}

impl ReceiverProvider for SpotifyReceiver {
    fn name(&self) -> &'static str {
        "Spotify Connect"
    }

    async fn start(&mut self, audio_tx: AudioSender, event_tx: ReceiverEventTx) -> Result<()> {
        let config = self.config.clone();
        let zone_index = self.zone_index;

        let task = tokio::spawn(async move {
            if let Err(e) = run_spotify(config, zone_index, audio_tx, event_tx).await {
                tracing::error!(zone = zone_index, error = %e, "Spotify receiver failed");
            }
        });

        self.task = Some(task);
        tracing::info!(zone = self.zone_index, name = %self.config.name, "Spotify Connect receiver started");
        Ok(())
    }

    async fn stop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }

    fn is_running(&self) -> bool {
        self.task.as_ref().is_some_and(|t| !t.is_finished())
    }
}

// ── Main loop ─────────────────────────────────────────────────

async fn run_spotify(
    config: SpotifyConfig,
    zone_index: usize,
    audio_tx: AudioSender,
    event_tx: ReceiverEventTx,
) -> Result<()> {
    loop {
        // Advertise via Zeroconf and wait for a Spotify app to connect
        tracing::info!(zone = zone_index, name = %config.name, "Waiting for Spotify Connect client");

        let mut discovery = Discovery::builder(&config.name, config.device_id())
            .device_type(DeviceType::Speaker)
            .launch()
            .map_err(|e| anyhow::anyhow!("Discovery failed: {e}"))?;

        // Wait for credentials from a connecting Spotify app
        let credentials = loop {
            use futures_util::StreamExt;
            match discovery.next().await {
                Some(librespot_discovery::DiscoveryEvent::Credentials(creds)) => break creds,
                Some(librespot_discovery::DiscoveryEvent::ServerError(e)) => {
                    tracing::warn!(zone = zone_index, error = %e, "Discovery server error");
                }
                Some(librespot_discovery::DiscoveryEvent::ZeroconfError(e)) => {
                    tracing::warn!(zone = zone_index, error = %e, "Zeroconf error");
                }
                None => return Ok(()), // Discovery stream ended
            }
        };

        tracing::info!(zone = zone_index, "Spotify client connected");

        // Create session
        let session = Session::new(SessionConfig::default(), None);
        session
            .connect(credentials, true)
            .await
            .map_err(|e| anyhow::anyhow!("Session connect failed: {e}"))?;

        // Create player with custom sink
        let tx = audio_tx.clone();
        let player = Player::new(
            PlayerConfig {
                bitrate: config.bitrate(),
                ..PlayerConfig::default()
            },
            session.clone(),
            Box::new(NoOpVolume),
            move || Box::new(ChannelSink::new(tx.clone(), LsAudioFormat::F32)) as Box<dyn Sink>,
        );

        // Subscribe to player events
        let mut event_rx = player.get_player_event_channel();

        // Create Spirc (Spotify Connect protocol handler)
        let (spirc, spirc_task) = Spirc::new(
            ConnectConfig {
                name: config.name.clone(),
                device_type: DeviceType::Speaker,
                initial_volume: Some(u16::MAX / 2),
                ..ConnectConfig::default()
            },
            session.clone(),
            player,
            Box::new(NoOpVolume),
        )
        .await
        .map_err(|e| anyhow::anyhow!("Spirc failed: {e}"))?;

        let _ = event_tx.try_send(ReceiverEvent::SessionStarted {
            format: AudioFormat {
                sample_rate: 44100,
                channels: 2,
            },
        });

        // Run event loop until session ends
        let spirc_handle = tokio::spawn(spirc_task);

        loop {
            tokio::select! {
                event = event_rx.recv() => {
                    match event {
                        Some(event) => handle_player_event(event, &session, &event_tx, zone_index).await,
                        None => break, // Player dropped
                    }
                }
                _ = session.is_invalid() => {
                    tracing::info!(zone = zone_index, "Spotify session ended");
                    break;
                }
            }
        }

        let _ = event_tx.try_send(ReceiverEvent::SessionEnded);
        spirc.shutdown();
        spirc_handle.abort();

        tracing::info!(
            zone = zone_index,
            "Spotify session closed, restarting discovery"
        );
    }
}

async fn handle_player_event(
    event: PlayerEvent,
    session: &Session,
    event_tx: &ReceiverEventTx,
    zone_index: usize,
) {
    match event {
        PlayerEvent::Playing {
            track_id,
            position_ms,
            ..
        } => {
            // Fetch track metadata from Spotify
            if let Ok(track) =
                librespot_metadata::Track::get(session, &track_id.into_spotify_id().unwrap()).await
            {
                let artist = track
                    .artists_with_role
                    .first()
                    .map(|a| a.name.clone())
                    .unwrap_or_default();
                let album = track.album.name.clone();
                let duration_ms = track.duration as u64;

                let _ = event_tx.try_send(ReceiverEvent::Metadata {
                    title: track.name.clone(),
                    artist,
                    album,
                });
                let _ = event_tx.try_send(ReceiverEvent::Progress {
                    position_ms: position_ms as u64,
                    duration_ms,
                });

                // Fetch cover art URL from album
                if let Some(cover) = track.album.covers.first() {
                    let cover_url = format!(
                        "https://i.scdn.co/image/{}",
                        cover.id.to_base16().unwrap_or_default()
                    );
                    // Fetch and send cover bytes
                    if let Some((bytes, _)) = crate::state::cover::fetch_cover(&cover_url).await {
                        let _ = event_tx.try_send(ReceiverEvent::CoverArt { bytes });
                    }
                }
            }
        }
        PlayerEvent::Paused { position_ms, .. } => {
            let _ = event_tx.try_send(ReceiverEvent::Progress {
                position_ms: position_ms as u64,
                duration_ms: 0, // Will be filled from last metadata
            });
        }
        PlayerEvent::VolumeChanged { volume } => {
            let percent = (volume as i32 * 100) / u16::MAX as i32;
            let _ = event_tx.try_send(ReceiverEvent::Volume { percent });
        }
        PlayerEvent::Stopped { .. } | PlayerEvent::EndOfTrack { .. } => {}
        _ => {}
    }
}

// ── Custom audio sink ─────────────────────────────────────────

/// Sink that sends F32 PCM samples to the receiver's audio channel.
struct ChannelSink {
    tx: AudioSender,
    format: LsAudioFormat,
}

impl ChannelSink {
    fn new(tx: AudioSender, format: LsAudioFormat) -> Self {
        Self { tx, format }
    }
}

impl Open for ChannelSink {
    fn open(_device: Option<String>, format: LsAudioFormat) -> Self {
        // This won't be called — we construct directly in the player factory.
        // But the trait requires it.
        panic!("ChannelSink::open should not be called directly");
    }
}

impl Sink for ChannelSink {
    sink_as_bytes!();
}

impl SinkAsBytes for ChannelSink {
    fn write_bytes(&mut self, data: &[u8]) -> SinkResult<()> {
        // Data is F32LE bytes (we configured AudioFormat::F32)
        let samples: Vec<f32> = data
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        self.tx
            .try_send(samples)
            .map_err(|e| SinkError::OnWrite(format!("Channel send failed: {e}")))?;
        Ok(())
    }
}
