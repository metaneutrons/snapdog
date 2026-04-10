// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Snapcast event handler — maps backend events to state updates and notifications.

use std::sync::Arc;

use crate::api;
use crate::config::AppConfig;
use crate::player::{GroupAction, SnapcastCmd};
use crate::snapcast::backend::{SnapcastBackend, SnapcastEvent};
use crate::state;

/// Spawn a task that receives events from the backend and updates state.
#[cfg(feature = "snapcast-embedded")]
pub fn spawn_event_handler(
    mut event_rx: super::embedded::EmbeddedEventReceiver,
    config: Arc<AppConfig>,
    backend: Arc<dyn SnapcastBackend>,
    store: state::SharedState,
    notify: api::ws::NotifySender,
) {
    tokio::spawn(async move {
        tracing::info!("Snapcast event handler started");
        while let Some(event) = event_rx.recv().await {
            handle_event(event, &config, &*backend, &store, &notify).await;
        }
        tracing::info!("Snapcast event handler stopped");
    });
}

async fn handle_event(
    event: SnapcastEvent,
    config: &AppConfig,
    backend: &dyn SnapcastBackend,
    store: &state::SharedState,
    notify: &api::ws::NotifySender,
) {
    match event {
        SnapcastEvent::ClientConnected { id, name, mac } => {
            let zone_index = {
                let mut s = store.write().await;
                let matched = if mac.is_empty() {
                    s.clients.values_mut().find(|c| c.name == name)
                } else {
                    s.clients.values_mut().find(|c| c.mac.to_lowercase() == mac)
                };
                if let Some(client) = matched {
                    client.connected = true;
                    client.snapcast_id = Some(id.clone());
                    tracing::info!(client = %client.name, id = %id, "Client connected");
                    Some(client.zone_index)
                } else {
                    None
                }
            };
            broadcast_all_clients(store, notify).await;

            // Setup zone group for the connecting client
            if let Some(zone_index) = zone_index {
                setup_zone_group(zone_index, &id, config, backend, store).await;
            }
        }
        SnapcastEvent::ClientDisconnected { id } => {
            let mut s = store.write().await;
            if let Some(client) = s
                .clients
                .values_mut()
                .find(|c| c.snapcast_id.as_deref() == Some(&id))
            {
                client.connected = false;
                tracing::info!(client = %client.name, "Client disconnected");
            }
            drop(s);
            broadcast_all_clients(store, notify).await;
        }
        SnapcastEvent::ClientVolumeChanged { id, volume, muted } => {
            let mut s = store.write().await;
            if let Some((&idx, client)) = s
                .clients
                .iter_mut()
                .find(|(_, c)| c.snapcast_id.as_deref() == Some(&id))
            {
                client.volume = volume;
                client.muted = muted;
                let n = client_notification(idx, client);
                drop(s);
                let _ = notify.send(n);
            }
        }
        SnapcastEvent::ClientLatencyChanged { id, latency } => {
            let mut s = store.write().await;
            if let Some((&idx, client)) = s
                .clients
                .iter_mut()
                .find(|(_, c)| c.snapcast_id.as_deref() == Some(&id))
            {
                client.latency_ms = latency;
                let n = client_notification(idx, client);
                drop(s);
                let _ = notify.send(n);
            }
        }
        SnapcastEvent::ClientNameChanged { id, name } => {
            let mut s = store.write().await;
            if let Some((&idx, client)) = s
                .clients
                .iter_mut()
                .find(|(_, c)| c.snapcast_id.as_deref() == Some(&id))
            {
                client.name = name;
                let n = client_notification(idx, client);
                drop(s);
                let _ = notify.send(n);
            }
        }
        SnapcastEvent::ServerUpdated => {
            sync_group_ids(config, backend, store).await;
        }
    }
}

/// Assign all clients of a zone to the same group, set stream + name.
async fn setup_zone_group(
    zone_index: usize,
    connecting_client_id: &str,
    config: &AppConfig,
    backend: &dyn SnapcastBackend,
    store: &state::SharedState,
) {
    let zone_config = match config.zones.get(zone_index - 1) {
        Some(z) => z,
        None => return,
    };

    // Get current server status to find group IDs
    let status = match backend.get_status().await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get server status for group setup");
            return;
        }
    };

    let groups = match status.get("server").and_then(|s| s.get("groups")) {
        Some(g) => g,
        None => return,
    };

    // Find all Snapcast client IDs that belong to this zone
    let s = store.read().await;
    let snap_client_ids: Vec<String> = s
        .clients
        .values()
        .filter(|c| c.zone_index == zone_index && c.connected)
        .filter_map(|c| c.snapcast_id.clone())
        .collect();
    drop(s);

    if snap_client_ids.is_empty() {
        return;
    }

    // Find the group the connecting client is in
    let gid = groups.as_array().and_then(|groups| {
        groups
            .iter()
            .find(|g| {
                g.get("clients")
                    .and_then(|c| c.as_array())
                    .is_some_and(|clients| {
                        clients.iter().any(|c| {
                            c.get("id").and_then(|id| id.as_str()) == Some(connecting_client_id)
                        })
                    })
            })
            .and_then(|g| g.get("id").and_then(|id| id.as_str()).map(String::from))
    });

    let Some(gid) = gid else {
        tracing::warn!(zone = zone_index, "No group found for connecting client");
        return;
    };

    // Assign clients, stream, and name
    let cmds = [
        SnapcastCmd::Group {
            group_id: gid.clone(),
            action: GroupAction::Clients(snap_client_ids.clone()),
        },
        SnapcastCmd::Group {
            group_id: gid.clone(),
            action: GroupAction::Stream(zone_config.stream_name.clone()),
        },
        SnapcastCmd::Group {
            group_id: gid.clone(),
            action: GroupAction::Name(zone_config.name.clone()),
        },
    ];

    for cmd in cmds {
        if let Err(e) = backend.execute(cmd).await {
            tracing::warn!(error = %e, "Failed to configure zone group");
        }
    }

    // Store group ID
    let mut s = store.write().await;
    if let Some(zone) = s.zones.get_mut(&zone_index) {
        zone.snapcast_group_id = Some(gid.clone());
    }

    tracing::info!(
        zone = zone_index,
        group = %gid,
        clients = ?snap_client_ids,
        stream = %zone_config.stream_name,
        "Zone group configured"
    );
}

/// Re-sync zone group IDs from server status.
///
/// The server may reorganize groups (new client → new group, SetGroupClients → merge/delete).
/// The stream name is the stable identifier; the group ID is ephemeral.
async fn sync_group_ids(
    config: &AppConfig,
    backend: &dyn SnapcastBackend,
    store: &state::SharedState,
) {
    let status = match backend.get_status().await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to get server status for group sync");
            return;
        }
    };

    let groups = match status
        .get("server")
        .and_then(|s| s.get("groups"))
        .and_then(|g| g.as_array())
    {
        Some(g) => g,
        None => return,
    };

    let mut s = store.write().await;
    for zone_cfg in &config.zones {
        // Find the group whose stream matches this zone, prefer the one with most clients
        let best_group = groups
            .iter()
            .filter(|g| g.get("stream_id").and_then(|s| s.as_str()) == Some(&zone_cfg.stream_name))
            .max_by_key(|g| {
                g.get("clients")
                    .and_then(|c| c.as_array())
                    .map(|c| c.len())
                    .unwrap_or(0)
            });

        if let Some(group) = best_group {
            if let Some(gid) = group.get("id").and_then(|id| id.as_str()) {
                if let Some(zone) = s.zones.get_mut(&zone_cfg.index) {
                    if zone.snapcast_group_id.as_deref() != Some(gid) {
                        tracing::debug!(zone = zone_cfg.index, new = %gid, "Zone group ID updated");
                        zone.snapcast_group_id = Some(gid.to_string());
                    }
                }
            }
        }
    }
}

fn client_notification(idx: usize, client: &state::ClientState) -> api::ws::Notification {
    api::ws::Notification::ClientStateChanged {
        client: idx,
        volume: client.volume,
        muted: client.muted,
        connected: client.connected,
        zone: client.zone_index,
    }
}

async fn broadcast_all_clients(store: &state::SharedState, notify: &api::ws::NotifySender) {
    let s = store.read().await;
    for (&idx, client) in s.clients.iter() {
        let _ = notify.send(client_notification(idx, client));
    }
}
