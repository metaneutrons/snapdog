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

    let store = state::init(&config, Some(&PathBuf::from("state.json")))?;
    let covers = state::cover::new_cache();
    let (notify_tx, _) = api::ws::notification_channel();

    // Start snapserver
    let mut snapserver = process::SnapserverHandle::start(&config).await?;
    if config.snapcast.managed {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    // Connect to Snapcast JSON-RPC
    let mut snap = snapcast::Snapcast::from_config(&config).await?;
    snap.init().await?;
    let snap_state = snap.state().clone();

    // Snapcast command channel (SnapcastConnection is !Send, stays on main task)
    let (snap_cmd_tx, mut snap_cmd_rx) = tokio::sync::mpsc::channel::<player::SnapcastCmd>(64);

    // Spawn ZonePlayers
    let zone_commands = player::spawn_zone_players(player::ZonePlayerContext {
        config: config.clone(),
        store: store.clone(),
        covers: covers.clone(),
        notify: notify_tx.clone(),
        snap_tx: snap_cmd_tx,
        client_mac_map: snap_state
            .clients
            .iter()
            .map(|e| (e.value().host.mac.to_lowercase(), e.key().clone()))
            .collect(),
        group_ids: snap_state.groups.iter().map(|g| g.key().clone()).collect(),
        group_clients: snap_state
            .groups
            .iter()
            .map(|g| (g.key().clone(), g.clients.iter().cloned().collect()))
            .collect(),
    })
    .await?;

    // Start API server
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

    // Zones start idle — user controls playback via API/MQTT/KNX

    // Main loop: process Snapcast commands + wait for shutdown
    loop {
        tokio::select! {
            Some(cmd) = snap_cmd_rx.recv() => {
                let result = match cmd {
                    player::SnapcastCmd::SetGroupStream { group_id, stream_id } =>
                        snap.set_group_stream(&group_id, &stream_id).await,
                    player::SnapcastCmd::SetGroupClients { group_id, client_ids } =>
                        snap.set_group_clients(&group_id, client_ids).await,
                    player::SnapcastCmd::SetGroupName { group_id, name } =>
                        snap.set_group_name(&group_id, &name).await,
                    player::SnapcastCmd::SetGroupVolume { group_id, percent } =>
                        snap.set_group_volume(&group_id, percent).await,
                    player::SnapcastCmd::SetGroupMute { group_id, muted } =>
                        snap.set_group_mute(&group_id, muted).await,
                };
                if let Err(e) = result {
                    tracing::warn!(error = %e, "Snapcast command failed");
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Shutting down");
                break;
            }
        }
    }

    if let Err(e) = store.write().await.persist() {
        tracing::warn!(error = %e, "Failed to persist state");
    }
    snapserver.stop().await?;
    Ok(())
}
