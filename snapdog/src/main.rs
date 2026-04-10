// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use snapdog::*;

/// Multi-zone audio controller with AirPlay, Snapcast, MQTT, and KNX integration.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "snapdog.toml")]
    config: PathBuf,
}

/// Volume coalescing window — rapid volume changes within this window are merged.
const VOLUME_COALESCE_MS: u64 = 50;
/// Channel capacity for Snapcast commands from zone players, API, MQTT, KNX.
const SNAPCAST_CMD_CHANNEL_SIZE: usize = 64;

#[tokio::main]
async fn main() -> Result<()> {
    // ── Parse config ──────────────────────────────────────────
    let cli = Cli::parse();
    let config_path = &cli.config;

    let config = Arc::new(config::load(config_path)?);

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
        "Configuration loaded from {}",
        config_path.display()
    );

    // ── Initialize subsystems ─────────────────────────────────
    let store = state::init(&config, Some(&PathBuf::from("state.json")))?;
    let covers = state::cover::new_cache();
    let (notify_tx, _) = api::ws::notification_channel();

    // ── Snapcast backend ──────────────────────────────────────
    #[cfg(feature = "snapcast-process")]
    let mut snapserver = process::SnapserverHandle::start(&config).await?;
    #[cfg(feature = "snapcast-process")]
    if config.snapcast.managed {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    #[cfg(feature = "snapcast-process")]
    let snap = snapcast::SnapcastClient::from_config(&config).await?;
    #[cfg(feature = "snapcast-process")]
    let status = snap.server_get_status().await?;
    #[cfg(feature = "snapcast-process")]
    snapcast::sync_initial_state(&status, &config, &snap, &store).await;
    #[cfg(all(feature = "snapcast-process", not(feature = "snapcast-embedded")))]
    let mut snap_notifications = snap.subscribe();

    #[cfg(feature = "snapcast-embedded")]
    let (embedded_backend, embedded_events) =
        snapcast::embedded::EmbeddedBackend::start(&config, store.clone()).await?;
    #[cfg(feature = "snapcast-embedded")]
    let backend: Arc<dyn snapcast::backend::SnapcastBackend> = Arc::new(embedded_backend);
    #[cfg(feature = "snapcast-embedded")]
    snapcast::events::spawn_event_handler(
        embedded_events,
        config.clone(),
        backend.clone(),
        store.clone(),
        notify_tx.clone(),
    );

    #[cfg(all(feature = "snapcast-process", not(feature = "snapcast-embedded")))]
    let process_backend = Arc::new(snapcast::process::ProcessBackend::start(&config, snap).await?);
    #[cfg(all(feature = "snapcast-process", not(feature = "snapcast-embedded")))]
    let backend: Arc<dyn snapcast::backend::SnapcastBackend> = process_backend.clone();

    // Snapcast command channel (used by zone players, API, MQTT, KNX)
    let (snap_cmd_tx, mut snap_cmd_rx) =
        tokio::sync::mpsc::channel::<player::SnapcastCmd>(SNAPCAST_CMD_CHANNEL_SIZE);

    // EQ store
    let eq_store = Arc::new(std::sync::Mutex::new(audio::eq::EqStore::load(
        std::path::Path::new("eq.json"),
    )));

    // ── Zone players ──────────────────────────────────────────
    let zone_commands = player::spawn_zone_players(player::ZonePlayerContext {
        config: config.clone(),
        store: store.clone(),
        covers: covers.clone(),
        notify: notify_tx.clone(),
        snap_tx: snap_cmd_tx.clone(),
        backend: backend.clone(),
        eq_store: eq_store.clone(),
        #[cfg(feature = "snapcast-process")]
        client_mac_map: snapcast::build_client_mac_map(&status),
        #[cfg(not(feature = "snapcast-process"))]
        client_mac_map: std::collections::HashMap::new(),
        #[cfg(feature = "snapcast-process")]
        group_ids: snapcast::build_group_ids(&status),
        #[cfg(not(feature = "snapcast-process"))]
        group_ids: Vec::new(),
        #[cfg(feature = "snapcast-process")]
        group_clients: snapcast::build_group_clients(&status),
        #[cfg(not(feature = "snapcast-process"))]
        group_clients: std::collections::HashMap::new(),
    })
    .await?;

    // ── API server ────────────────────────────────────────────
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
            eq_store.clone(),
        )
        .await
        {
            tracing::error!(error = %e, "API server failed");
        }
    });

    // ── MQTT bridge ───────────────────────────────────────────
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

    // ── KNX bridge ────────────────────────────────────────────
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
    let cmd_backend = backend.clone();

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
            // Snapcast commands from zone players / API / MQTT / KNX
            Some(cmd) = snap_cmd_rx.recv() => {
                // Coalesce client volume commands (50ms window)
                if let player::SnapcastCmd::Client { ref client_id, action: player::ClientAction::Volume(v) } = cmd {
                    pending_volumes.insert(client_id.clone(), v);
                    coalesce_deadline = Some(tokio::time::Instant::now() + std::time::Duration::from_millis(VOLUME_COALESCE_MS));
                } else {
                    if let Err(e) = cmd_backend.execute(cmd).await {
                        tracing::warn!(error = %e, "Snapcast command failed");
                    }
                }
            }
            // Coalesce timer fired — flush pending volumes
            _ = sleep => {
                for (client_id, volume) in pending_volumes.drain() {
                    let cmd = player::SnapcastCmd::Client {
                        client_id,
                        action: player::ClientAction::Volume(volume),
                    };
                    if let Err(e) = cmd_backend.execute(cmd).await {
                        tracing::warn!(error = %e, "Snapcast volume command failed");
                    }
                }
                coalesce_deadline = None;
            }
            // Snapcast server notifications → state updates (process backend only)
            _ = async {
                #[cfg(all(feature = "snapcast-process", not(feature = "snapcast-embedded")))]
                if let Ok(notification) = snap_notifications.recv().await {
                    snapcast::handle_notification(notification, &config, process_backend.client(), &store, &notify_tx).await;
                }
                #[cfg(not(all(feature = "snapcast-process", not(feature = "snapcast-embedded"))))]
                std::future::pending::<()>().await;
            } => {}
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
    let _ = backend.stop().await;
    if let Err(e) = store.write().await.persist() {
        tracing::warn!(error = %e, "Failed to persist state");
    }
    #[cfg(feature = "snapcast-process")]
    snapserver.stop().await?;
    Ok(())
}
