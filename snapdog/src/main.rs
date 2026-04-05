// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

use snapdog::*;

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

    // Populate snapcast_id and connected status for already-connected clients
    {
        let mut s = store.write().await;
        for snap_client in snap_state.clients.iter() {
            let mac = snap_client.value().host.mac.to_lowercase();
            let snap_id = snap_client.key().clone();
            let connected = snap_client.value().connected;
            if let Some(client) = s.clients.values_mut().find(|c| c.mac.to_lowercase() == mac) {
                client.snapcast_id = Some(snap_id.clone());
                client.connected = connected;
                tracing::info!(client = %client.name, snap_id = %snap_id, connected, "Initial client state synced");
            }
        }
    }

    // Snapcast command channel (SnapcastConnection is !Send, stays on main task)
    let (snap_cmd_tx, mut snap_cmd_rx) = tokio::sync::mpsc::channel::<player::SnapcastCmd>(64); // snapcast command backlog;

    // Spawn ZonePlayers
    let zone_commands = player::spawn_zone_players(player::ZonePlayerContext {
        config: config.clone(),
        store: store.clone(),
        covers: covers.clone(),
        notify: notify_tx.clone(),
        snap_tx: snap_cmd_tx.clone(),
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

    // Zones start idle — user controls playback via API/MQTT/KNX

    // Start MQTT bridge (if configured)
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

    // Main loop: process Snapcast commands, MQTT events, + wait for shutdown
    let mqtt_zone_cmds = zone_commands.clone();
    let mqtt_store = store.clone();
    loop {
        tokio::select! {
            Some(cmd) = snap_cmd_rx.recv() => {
                let result = match cmd {
                    player::SnapcastCmd::Group { group_id, action } => match action {
                        player::GroupAction::Stream(stream_id) =>
                            snap.set_group_stream(&group_id, &stream_id).await,
                        player::GroupAction::Clients(client_ids) =>
                            snap.set_group_clients(&group_id, client_ids).await,
                        player::GroupAction::Name(name) =>
                            snap.set_group_name(&group_id, &name).await,
                        player::GroupAction::Volume(percent) =>
                            snap.set_group_volume(&group_id, percent).await,
                        player::GroupAction::Mute(muted) =>
                            snap.set_group_mute(&group_id, muted).await,
                    },
                    player::SnapcastCmd::Client { client_id, action } => match action {
                        player::ClientAction::Volume(percent) =>
                            snap.set_client_volume(&client_id, percent.clamp(0, 100) as u8, false).await,
                        player::ClientAction::Mute(muted) => {
                            let vol = store.read().await.clients.values()
                                .find(|c| c.snapcast_id.as_deref() == Some(&client_id))
                                .map_or(100, |c| c.volume);
                            snap.set_client_volume(&client_id, vol.clamp(0, 100) as u8, muted).await
                        }
                        player::ClientAction::Latency(ms) =>
                            snap.set_client_latency(&client_id, ms).await,
                    },
                };
                if let Err(e) = result {
                    tracing::warn!(error = %e, "Snapcast command failed");
                } else {
                    tracing::debug!("Snapcast command executed successfully");
                    // After command execution, sync state from snapcast-control's internal state
                    sync_client_state_from_snapcast(&snap, &store, &notify_tx).await;
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Shutting down");
                break;
            }
            // Snapcast server notifications (client connect/disconnect, volume changes, etc.)
            Some(messages) = snap.recv() => {
                tracing::debug!(count = messages.len(), "Snapcast messages received");
                for msg in messages {
                    match msg {
                        Ok(snapcast_control::ValidMessage::Notification { method, .. }) => {
                            handle_snapcast_notification(*method, &store, &notify_tx, &config, &mut snap).await;
                        }
                        Ok(snapcast_control::ValidMessage::Result { .. }) => {
                            tracing::debug!("Snapcast result message (handled internally)");
                        }
                        Err(e) => tracing::warn!(error = %e, "Snapcast message error"),
                    }
                }
            }
            // MQTT event polling
            _ = async {
                if let Some(ref mut bridge) = mqtt_bridge {
                    bridge.poll_once(&mqtt_zone_cmds, &mqtt_store).await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {}
        }
    }

    // Persist state (blocking I/O OK — shutdown path, called once)
    if let Err(e) = store.write().await.persist() {
        tracing::warn!(error = %e, "Failed to persist state");
    }
    snapserver.stop().await?;
    Ok(())
}

async fn handle_snapcast_notification(
    notification: snapcast_control::Notification,
    store: &state::SharedState,
    notify: &tokio::sync::broadcast::Sender<api::ws::Notification>,
    config: &config::AppConfig,
    snap: &mut snapcast::Snapcast,
) {
    use snapcast_control::Notification;

    match notification {
        Notification::ClientOnConnect { params } => {
            let mac = params.client.host.mac.to_lowercase();
            let snap_id = params.client.id.clone();
            tracing::info!(mac = %mac, snap_id = %snap_id, "Snapcast client connected");

            let mut s = store.write().await;
            if let Some((&idx, client)) = s
                .clients
                .iter_mut()
                .find(|(_, c)| c.mac.to_lowercase() == mac)
            {
                client.connected = true;
                client.snapcast_id = Some(snap_id.clone());
                let zone_index = client.zone_index;
                let notif = api::ws::Notification::ClientStateChanged {
                    client: idx,
                    volume: client.volume,
                    muted: client.muted,
                    connected: client.connected,
                    zone: zone_index,
                };
                let name = client.name.clone();
                drop(s);
                tracing::info!(client = %name, "Client matched and marked connected");
                let _ = notify.send(notif);

                // Re-setup zone group: assign this client to the correct group
                setup_zone_group_for_client(zone_index, &snap_id, config, snap).await;
            }
        }
        Notification::ClientOnDisconnect { params } => {
            let snap_id = params.id;
            tracing::info!(snap_id = %snap_id, "Snapcast client disconnected");

            let mut s = store.write().await;
            if let Some((&idx, client)) = s
                .clients
                .iter_mut()
                .find(|(_, c)| c.snapcast_id.as_deref() == Some(&snap_id))
            {
                client.connected = false;
                let notif = api::ws::Notification::ClientStateChanged {
                    client: idx,
                    volume: client.volume,
                    muted: client.muted,
                    connected: client.connected,
                    zone: client.zone_index,
                };
                let name = client.name.clone();
                drop(s);
                tracing::info!(client = %name, "Client marked disconnected");
                let _ = notify.send(notif);
            }
        }
        Notification::ClientOnVolumeChanged { params } => {
            let snap_id = params.id;
            let volume = params.volume.percent as i32;
            let muted = params.volume.muted;

            let mut s = store.write().await;
            if let Some((&idx, client)) = s
                .clients
                .iter_mut()
                .find(|(_, c)| c.snapcast_id.as_deref() == Some(&snap_id))
            {
                client.volume = volume;
                client.muted = muted;
                let notif = api::ws::Notification::ClientStateChanged {
                    client: idx,
                    volume: client.volume,
                    muted: client.muted,
                    connected: client.connected,
                    zone: client.zone_index,
                };
                let name = client.name.clone();
                drop(s);
                tracing::info!(client = %name, volume, muted, "Client volume changed (external)");
                let _ = notify.send(notif);
            }
        }
        Notification::ClientOnLatencyChanged { params } => {
            let snap_id = params.id;
            let latency = params.latency as i32;
            let mut s = store.write().await;
            if let Some((&idx, client)) = s
                .clients
                .iter_mut()
                .find(|(_, c)| c.snapcast_id.as_deref() == Some(&snap_id))
            {
                client.latency_ms = latency;
                let notif = api::ws::Notification::ClientStateChanged {
                    client: idx,
                    volume: client.volume,
                    muted: client.muted,
                    connected: client.connected,
                    zone: client.zone_index,
                };
                let name = client.name.clone();
                drop(s);
                tracing::info!(client = %name, latency, "Client latency changed");
                let _ = notify.send(notif);
            }
        }
        Notification::StreamOnUpdate { params } => {
            tracing::info!(stream = %params.stream.id, status = ?params.stream.status, "Stream status updated");
        }
        Notification::GroupOnMute { params } => {
            tracing::info!(group = %params.id, mute = %params.mute, "Group mute changed");
        }
        Notification::GroupOnStreamChanged { params } => {
            tracing::info!(group = %params.id, stream = %params.stream_id, "Group stream changed");
        }
        Notification::GroupOnNameChanged { params } => {
            tracing::info!(group = %params.id, name = %params.name, "Group name changed");
        }
        Notification::ServerOnUpdate { .. } => {
            tracing::info!("Snapcast server state updated");
        }
        Notification::StreamOnProperties { params } => {
            tracing::debug!(stream = %params.id, "Stream properties updated");
        }
        Notification::ClientOnNameChanged { params } => {
            let snap_id = params.id;
            let new_name = params.name;
            let mut s = store.write().await;
            if let Some((&idx, client)) = s
                .clients
                .iter_mut()
                .find(|(_, c)| c.snapcast_id.as_deref() == Some(&snap_id))
            {
                client.name = new_name.clone();
                let notif = api::ws::Notification::ClientStateChanged {
                    client: idx,
                    volume: client.volume,
                    muted: client.muted,
                    connected: client.connected,
                    zone: client.zone_index,
                };
                drop(s);
                tracing::info!(client = %new_name, "Client name changed");
                let _ = notify.send(notif);
            }
        }
    }
}

async fn setup_zone_group_for_client(
    zone_index: usize,
    snap_client_id: &str,
    config: &config::AppConfig,
    snap: &mut snapcast::Snapcast,
) {
    // Re-fetch current Snapcast state to get fresh group info
    if let Err(e) = snap.init().await {
        tracing::warn!(error = %e, "Failed to refresh Snapcast state for group setup");
        return;
    }
    let snap_state = snap.state().clone();

    let zone_config = &config.zones[zone_index - 1];

    // Collect all snapcast IDs for clients in this zone
    let zone_macs: Vec<String> = config
        .clients
        .iter()
        .filter(|c| c.zone_index == zone_index)
        .map(|c| c.mac.to_lowercase())
        .collect();

    let snap_client_ids: Vec<String> = zone_macs
        .iter()
        .filter_map(|mac| {
            snap_state
                .clients
                .iter()
                .find(|e| e.value().host.mac.to_lowercase() == *mac)
                .map(|e| e.key().clone())
        })
        .collect();

    if snap_client_ids.is_empty() {
        return;
    }

    // Find the group containing the newly connected client, or use first available
    let gid = snap_state
        .groups
        .iter()
        .find(|g| g.clients.iter().any(|c| c == snap_client_id))
        .map(|g| g.key().clone())
        .or_else(|| snap_state.groups.iter().next().map(|g| g.key().clone()));

    let Some(gid) = gid else {
        tracing::warn!(
            zone = zone_index,
            "No Snapcast groups available for zone setup"
        );
        return;
    };

    // Assign all zone clients to this group, set stream and name
    if let Err(e) = snap.set_group_clients(&gid, snap_client_ids.clone()).await {
        tracing::warn!(error = %e, "Failed to set group clients");
    }
    if let Err(e) = snap.set_group_stream(&gid, &zone_config.stream_name).await {
        tracing::warn!(error = %e, "Failed to set group stream");
    }
    if let Err(e) = snap.set_group_name(&gid, &zone_config.name).await {
        tracing::warn!(error = %e, "Failed to set group name");
    }

    tracing::info!(
        zone = zone_index,
        group = %gid,
        clients = ?snap_client_ids,
        stream = %zone_config.stream_name,
        "Zone group configured dynamically"
    );
}

async fn sync_client_state_from_snapcast(
    snap: &snapcast::Snapcast,
    store: &state::SharedState,
    notify: &tokio::sync::broadcast::Sender<api::ws::Notification>,
) {
    let snap_state = snap.state().clone();
    let mut s = store.write().await;

    for snap_client in snap_state.clients.iter() {
        let snap_id = snap_client.key();
        let volume = snap_client.value().config.volume.percent as i32;
        let muted = snap_client.value().config.volume.muted;
        let connected = snap_client.value().connected;

        if let Some((&idx, client)) = s
            .clients
            .iter_mut()
            .find(|(_, c)| c.snapcast_id.as_deref() == Some(snap_id))
        {
            let changed =
                client.volume != volume || client.muted != muted || client.connected != connected;

            if changed {
                client.volume = volume;
                client.muted = muted;
                client.connected = connected;
                let notif = api::ws::Notification::ClientStateChanged {
                    client: idx,
                    volume: client.volume,
                    muted: client.muted,
                    connected: client.connected,
                    zone: client.zone_index,
                };
                let name = client.name.clone();
                tracing::info!(client = %name, volume, muted, connected, "Client state synced from Snapcast");
                let _ = notify.send(notif);
            }
        }
    }
}
