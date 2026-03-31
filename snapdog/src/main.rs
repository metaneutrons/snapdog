// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

pub mod airplay;
pub mod api;
pub mod audio;
pub mod config;
pub mod knx;
pub mod mqtt;
pub mod player;
mod process;
pub mod snapcast;
pub mod state;
pub mod subsonic;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let config_path = std::env::args()
        .nth(2)
        .filter(|_| std::env::args().nth(1).as_deref() == Some("--config"))
        .unwrap_or_else(|| "snapdog.toml".into());

    let config = Arc::new(config::load(&PathBuf::from(&config_path))?);

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
    let covers = state::cover::new_cache();
    let (notify_tx, _) = api::ws::notification_channel();

    // Start snapserver (or skip if managed=false)
    let mut snapserver = process::SnapserverHandle::start(&config).await?;

    if config.snapcast.managed {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    // Connect to snapcast JSON-RPC
    let mut snap = snapcast::Snapcast::from_config(&config).await?;
    snap.init().await?;

    // Spawn ZonePlayers
    let zone_commands = player::spawn_zone_players(
        config.clone(),
        store.clone(),
        covers.clone(),
        notify_tx.clone(),
    )
    .await?;

    // Start API server (needs zone_commands)
    let api_config = config::load(&PathBuf::from(&config_path))?;
    let api_store = store.clone();
    let api_commands = zone_commands.clone();
    let api_covers = covers.clone();
    let api_notify = notify_tx.clone();
    tokio::spawn(async move {
        if let Err(e) =
            api::serve(api_config, api_store, api_commands, api_covers, api_notify).await
        {
            tracing::error!(error = %e, "API server failed");
        }
    });

    // Auto-start first radio on first zone
    if !config.radios.is_empty() {
        if let Some(tx) = zone_commands.get(&1) {
            tx.send(player::ZoneCommand::PlayRadio(0)).await?;
        }
    }

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down");

    if let Err(e) = store.write().await.persist() {
        tracing::warn!(error = %e, "Failed to persist state");
    }

    snapserver.stop().await?;
    Ok(())
}
