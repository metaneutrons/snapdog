// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

mod airplay;
pub mod api;
pub mod audio;
pub mod config;
mod knx;
pub mod mqtt;
mod process;
pub mod snapcast;
pub mod state;
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

    // Start API server (runs in background)
    let api_config = config::load(&PathBuf::from(&config_path))?;
    tokio::spawn(async move {
        if let Err(e) = api::serve(api_config).await {
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
    snapserver.stop().await?;
    Ok(())
}
