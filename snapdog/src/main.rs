// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

use snapdog::*;

#[tokio::main]
async fn main() -> Result<()> {
    // ── Parse config ──────────────────────────────────────────
    let config_path = std::env::args()
        .nth(2)
        .filter(|_| std::env::args().nth(1).as_deref() == Some("--config"))
        .unwrap_or_else(|| "snapdog.toml".into());

    let config = Arc::new(config::load(&PathBuf::from(&config_path))?);

    // ── Initialize logging ────────────────────────────────────
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

    // ── Initialize subsystems ─────────────────────────────────
    let store = state::init(&config, Some(&PathBuf::from("state.json")))?;
    let covers = state::cover::new_cache();
    let (notify_tx, _) = api::ws::notification_channel();

    // Snapserver (managed child process)
    let mut snapserver = process::SnapserverHandle::start(&config).await?;
    if config.snapcast.managed {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    // Snapcast JSON-RPC client
    let snap = snapcast::SnapcastClient::from_config(&config).await?;
    let status = snap.server_get_status().await?;
    snapcast::sync_initial_state(&status, &store).await;
    let mut snap_notifications = snap.subscribe();

    // Snapcast command channel
    let (snap_cmd_tx, mut snap_cmd_rx) = tokio::sync::mpsc::channel::<player::SnapcastCmd>(64);

    // ── Wire subsystems ───────────────────────────────────────

    // Zone players
    let zone_commands = player::spawn_zone_players(player::ZonePlayerContext {
        config: config.clone(),
        store: store.clone(),
        covers: covers.clone(),
        notify: notify_tx.clone(),
        snap_tx: snap_cmd_tx.clone(),
        client_mac_map: snapcast::build_client_mac_map(&status),
        group_ids: snapcast::build_group_ids(&status),
        group_clients: snapcast::build_group_clients(&status),
    })
    .await?;

    // API server
    let api_config = config::load(&PathBuf::from(&config_path))?;
    let api_store = store.clone();
    let api_commands = zone_commands.clone();
    let api_covers = covers.clone();
    let api_notify = notify_tx.clone();
    let api_snap_tx = snap_cmd_tx.clone();
    tokio::spawn(async move {
        if let Err(e) = api::serve(
            api_config,
            api_store,
            api_commands,
            api_snap_tx,
            api_covers,
            api_notify,
        )
        .await
        {
            tracing::error!(error = %e, "API server failed");
        }
    });

    // MQTT bridge
    let mut mqtt_bridge = if let Some(mqtt_config) = &config.mqtt {
        match mqtt::MqttBridge::connect(mqtt_config).await {
            Ok(bridge) => {
                if let Err(e) = bridge.subscribe_commands().await {
                    tracing::warn!(error = %e, "MQTT subscribe failed");
                }
                Some(bridge)
            }
            Err(e) => {
                tracing::warn!(error = %e, "MQTT connection failed — running without MQTT");
                None
            }
        }
    } else {
        None
    };

    // KNX bridge
    if config.knx.enabled {
        let knx_notifications = notify_tx.subscribe();
        match knx::start(
            &config,
            store.clone(),
            knx_notifications,
            zone_commands.clone(),
            snap_cmd_tx.clone(),
        )
        .await
        {
            Ok(()) => {}
            Err(e) => tracing::warn!(error = %e, "KNX connection failed — running without KNX"),
        }
    }

    // ── Main loop ─────────────────────────────────────────────
    let mqtt_zone_cmds = zone_commands.clone();
    let mqtt_store = store.clone();

    // Volume coalescing: buffer rapid volume changes per client (50ms window)
    let mut pending_volumes: std::collections::HashMap<String, i32> =
        std::collections::HashMap::new();
    let mut coalesce_deadline: Option<tokio::time::Instant> = None;

    loop {
        let sleep = async {
            match coalesce_deadline {
                Some(d) => tokio::time::sleep_until(d).await,
                None => std::future::pending().await,
            }
        };

        tokio::select! {
            // Snapcast commands from zone players / API
            Some(cmd) = snap_cmd_rx.recv() => {
                // Coalesce client volume commands
                if let player::SnapcastCmd::Client { ref client_id, action: player::ClientAction::Volume(v) } = cmd {
                    pending_volumes.insert(client_id.clone(), v);
                    coalesce_deadline = Some(tokio::time::Instant::now() + std::time::Duration::from_millis(50));
                } else {
                    snapcast::execute_command(&snap, cmd, &config, &store, &notify_tx).await;
                }
            }
            // Coalesce timer fired — flush pending volumes
            _ = sleep => {
                for (client_id, volume) in pending_volumes.drain() {
                    let cmd = player::SnapcastCmd::Client {
                        client_id,
                        action: player::ClientAction::Volume(volume),
                    };
                    snapcast::execute_command(&snap, cmd, &config, &store, &notify_tx).await;
                }
                coalesce_deadline = None;
            }
            // Snapcast server notifications → state updates
            Ok(notification) = snap_notifications.recv() => {
                snapcast::handle_notification(notification, &config, &snap, &store, &notify_tx).await;
            }
            // MQTT events
            _ = async {
                if let Some(ref mut bridge) = mqtt_bridge {
                    bridge.poll_once(&mqtt_zone_cmds, &mqtt_store).await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {}
            // Shutdown
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Shutting down");
                break;
            }
        }
    }

    // ── Shutdown ──────────────────────────────────────────────
    if let Err(e) = store.write().await.persist() {
        tracing::warn!(error = %e, "Failed to persist state");
    }
    snapserver.stop().await?;
    Ok(())
}
