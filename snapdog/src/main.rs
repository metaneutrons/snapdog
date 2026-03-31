// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

pub mod airplay;
pub mod api;
pub mod audio;
pub mod config;
pub mod knx;
pub mod mqtt;
mod process;
pub mod snapcast;
pub mod state;
pub mod subsonic;

use std::path::PathBuf;

use anyhow::Result;
use tokio::io::AsyncWriteExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let config_path = std::env::args()
        .nth(2)
        .filter(|_| std::env::args().nth(1).as_deref() == Some("--config"))
        .unwrap_or_else(|| "snapdog.toml".into());

    let config = config::load(&PathBuf::from(&config_path))?;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive(format!("snapdog={}", config.system.log_level).parse()?),
        )
        .init();

    tracing::info!(
        zones = config.zones.len(),
        clients = config.clients.len(),
        radios = config.radios.len(),
        "Configuration loaded from {config_path}"
    );

    // Initialize state store
    let store = state::init(&config, Some(&PathBuf::from("state.json")))?;

    // Start snapserver (or skip if managed=false)
    let mut snapserver = process::SnapserverHandle::start(&config).await?;

    // Give snapserver time to start listening
    if config.snapcast.managed {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    // Start API server (runs in background)
    let api_config = config::load(&PathBuf::from(&config_path))?;
    let api_store = store.clone();
    tokio::spawn(async move {
        if let Err(e) = api::serve(api_config, api_store).await {
            tracing::error!(error = %e, "API server failed");
        }
    });

    // Connect to snapcast JSON-RPC
    let mut snap = snapcast::Snapcast::from_config(&config).await?;
    snap.init().await?;

    // MVP: Play first radio station on first zone
    if let Some(radio) = config.radios.first() {
        let zone = &config.zones[0];
        tracing::info!(radio = %radio.name, zone = %zone.name, "Starting radio playback");

        // Update state
        {
            let mut s = store.write().await;
            if let Some(z) = s.zones.get_mut(&zone.index) {
                z.playback = state::PlaybackState::Playing;
                z.source = state::SourceType::Radio;
                z.radio_index = Some(0);
                z.track = Some(state::TrackInfo {
                    title: radio.name.clone(),
                    artist: "Radio".into(),
                    album: String::new(),
                    album_artist: None,
                    genre: None,
                    year: None,
                    track_number: None,
                    disc_number: None,
                    duration_ms: 0,
                    position_ms: 0,
                    source: state::SourceType::Radio,
                    bitrate_kbps: None,
                    content_type: None,
                    sample_rate: None,
                });
            }
        }

        let mut tcp = snapcast::open_audio_source(zone.tcp_source_port).await?;
        let (tx, mut rx) = audio::pcm_channel(64);

        let url = radio.url.clone();
        let audio_config = config.audio.clone();
        tokio::spawn(async move {
            if let Err(e) = audio::decode_http_stream(url, tx, audio_config).await {
                tracing::error!(error = %e, "Decode stream failed");
            }
        });

        tokio::spawn(async move {
            while let Some(pcm) = rx.recv().await {
                if let Err(e) = tcp.write_all(&pcm).await {
                    tracing::error!(error = %e, "TCP write failed");
                    break;
                }
            }
        });
    }

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down");

    // Persist state before exit
    if let Err(e) = store.write().await.persist() {
        tracing::warn!(error = %e, "Failed to persist state");
    }

    snapserver.stop().await?;
    Ok(())
}
