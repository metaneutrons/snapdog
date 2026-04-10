// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Snapcast event handler — maps backend events to state updates and notifications.

use crate::api;
use crate::snapcast::backend::SnapcastEvent;
use crate::state;

/// Spawn a task that receives events from the backend and updates state.
#[cfg(feature = "snapcast-embedded")]
pub fn spawn_event_handler(
    mut event_rx: super::embedded::EmbeddedEventReceiver,
    store: state::SharedState,
    notify: api::ws::NotifySender,
) {
    tokio::spawn(async move {
        tracing::info!("Snapcast event handler started");
        while let Some(event) = event_rx.recv().await {
            handle_event(event, &store, &notify).await;
        }
        tracing::info!("Snapcast event handler stopped");
    });
}

async fn handle_event(
    event: SnapcastEvent,
    store: &state::SharedState,
    notify: &api::ws::NotifySender,
) {
    match event {
        SnapcastEvent::ClientConnected { id, name, mac } => {
            let mut s = store.write().await;
            let matched = if mac.is_empty() {
                // Embedded backend: match by name
                s.clients.values_mut().find(|c| c.name == name)
            } else {
                s.clients.values_mut().find(|c| c.mac.to_lowercase() == mac)
            };
            if let Some(client) = matched {
                client.connected = true;
                client.snapcast_id = Some(id.clone());
                tracing::info!(client = %client.name, id = %id, "Client connected");
            }
            drop(s);
            broadcast_all_clients(store, notify).await;
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
            tracing::debug!("Server state updated");
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
