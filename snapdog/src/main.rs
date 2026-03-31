// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

mod airplay;
mod api;
pub mod audio;
pub mod config;
mod knx;
mod mqtt;
mod process;
pub mod snapcast;
mod state;
mod subsonic;

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

    // Start snapserver (or skip if managed=false)
    let mut snapserver = process::SnapserverHandle::start(&config).await?;

    // Connect to snapcast JSON-RPC
    let mut snap = snapcast::Snapcast::from_config(&config).await?;
    snap.init().await?;

    // MVP: Play first radio station on first zone
    if let Some(radio) = config.radios.first() {
        let zone = &config.zones[0];
        tracing::info!(
            radio = %radio.name,
            zone = %zone.name,
            port = zone.tcp_source_port,
            "Starting radio playback"
        );

        // Open TCP connection to snapcast source
        let mut tcp = snapcast::open_audio_source(zone.tcp_source_port).await?;

        // PCM channel: decoder → TCP writer
        let (tx, mut rx) = audio::pcm_channel(64);

        // Spawn decoder task
        let url = radio.url.clone();
        let audio_config = config.audio.clone();
        let decode_handle = tokio::spawn(async move {
            if let Err(e) = audio::decode_http_stream(url, tx, audio_config).await {
                tracing::error!(error = %e, "Decode stream failed");
            }
        });

        // Spawn TCP writer task
        let write_handle = tokio::spawn(async move {
            while let Some(pcm) = rx.recv().await {
                if let Err(e) = tcp.write_all(&pcm).await {
                    tracing::error!(error = %e, "TCP write failed");
                    break;
                }
            }
            tracing::info!("PCM writer stopped");
        });

        // Wait for Ctrl+C
        tokio::signal::ctrl_c().await?;
        tracing::info!("Shutting down");

        decode_handle.abort();
        write_handle.abort();
    } else {
        tracing::warn!("No radio stations configured — waiting for Ctrl+C");
        tokio::signal::ctrl_c().await?;
    }

    snapserver.stop().await?;
    Ok(())
}
