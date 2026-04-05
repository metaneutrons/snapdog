// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Snapcast JSON-RPC client and TCP audio source management.

pub mod connection;
pub mod protocol;
pub mod types;

use std::net::SocketAddr;

use anyhow::{Context, Result};
use serde_json::json;
use tokio::net::TcpStream;
use tokio::sync::broadcast;

use crate::config::AppConfig;
use connection::Connection;
pub use protocol::Notification;
pub use types::ServerStatus;

// ── SnapcastClient ────────────────────────────────────────────

/// Snapcast controller: JSON-RPC connection + TCP audio sources.
pub struct SnapcastClient {
    conn: Connection,
}

impl SnapcastClient {
    /// Connect to snapserver JSON-RPC.
    pub async fn connect(addr: SocketAddr) -> Result<Self> {
        let conn = Connection::connect(addr).await?;
        Ok(Self { conn })
    }

    /// Connect using app config.
    pub async fn from_config(config: &AppConfig) -> Result<Self> {
        let tcp_port = config.snapcast.jsonrpc_port;
        let host = &config.snapcast.address;
        let addr: SocketAddr = tokio::net::lookup_host(format!("{host}:{tcp_port}"))
            .await
            .context("Failed to resolve snapcast address")?
            .next()
            .context("No address found for snapcast host")?;
        Self::connect(addr).await
    }

    /// Subscribe to Snapcast server notifications.
    pub fn subscribe(&self) -> broadcast::Receiver<Notification> {
        self.conn.subscribe()
    }

    // ── Server methods ────────────────────────────────────────

    /// Fetch full server status.
    pub async fn server_get_status(&self) -> Result<ServerStatus> {
        let result = self.conn.request("Server.GetStatus", json!({})).await?;
        serde_json::from_value(result).context("Failed to parse ServerStatus")
    }

    /// Get JSON-RPC protocol version.
    pub async fn server_get_rpc_version(&self) -> Result<serde_json::Value> {
        self.conn.request("Server.GetRPCVersion", json!({})).await
    }

    /// Delete a client from the server.
    pub async fn server_delete_client(&self, id: &str) -> Result<()> {
        self.conn
            .request("Server.DeleteClient", json!({ "id": id }))
            .await?;
        Ok(())
    }

    // ── Client methods ────────────────────────────────────────

    /// Get client status.
    pub async fn client_get_status(&self, id: &str) -> Result<types::Client> {
        let result = self
            .conn
            .request("Client.GetStatus", json!({ "id": id }))
            .await?;
        #[derive(serde::Deserialize)]
        struct R {
            client: types::Client,
        }
        let r: R = serde_json::from_value(result)?;
        Ok(r.client)
    }

    /// Set client volume.
    pub async fn client_set_volume(&self, id: &str, percent: u8, muted: bool) -> Result<()> {
        self.conn
            .request(
                "Client.SetVolume",
                json!({ "id": id, "volume": { "percent": percent, "muted": muted } }),
            )
            .await?;
        Ok(())
    }

    /// Set client latency.
    pub async fn client_set_latency(&self, id: &str, latency: i32) -> Result<()> {
        self.conn
            .request("Client.SetLatency", json!({ "id": id, "latency": latency }))
            .await?;
        Ok(())
    }

    /// Set client name.
    pub async fn client_set_name(&self, id: &str, name: &str) -> Result<()> {
        self.conn
            .request("Client.SetName", json!({ "id": id, "name": name }))
            .await?;
        Ok(())
    }

    // ── Group methods ─────────────────────────────────────────

    /// Get group status.
    pub async fn group_get_status(&self, id: &str) -> Result<types::Group> {
        let result = self
            .conn
            .request("Group.GetStatus", json!({ "id": id }))
            .await?;
        #[derive(serde::Deserialize)]
        struct R {
            group: types::Group,
        }
        let r: R = serde_json::from_value(result)?;
        Ok(r.group)
    }

    /// Set group mute.
    pub async fn group_set_mute(&self, id: &str, muted: bool) -> Result<()> {
        self.conn
            .request("Group.SetMute", json!({ "id": id, "mute": muted }))
            .await?;
        Ok(())
    }

    /// Set group stream.
    pub async fn group_set_stream(&self, id: &str, stream_id: &str) -> Result<()> {
        self.conn
            .request(
                "Group.SetStream",
                json!({ "id": id, "stream_id": stream_id }),
            )
            .await?;
        Ok(())
    }

    /// Set group clients.
    pub async fn group_set_clients(&self, id: &str, clients: Vec<String>) -> Result<()> {
        self.conn
            .request("Group.SetClients", json!({ "id": id, "clients": clients }))
            .await?;
        Ok(())
    }

    /// Set group name.
    pub async fn group_set_name(&self, id: &str, name: &str) -> Result<()> {
        self.conn
            .request("Group.SetName", json!({ "id": id, "name": name }))
            .await?;
        Ok(())
    }

    // ── Stream methods ────────────────────────────────────────

    /// Add a stream.
    pub async fn stream_add(&self, stream_uri: &str) -> Result<serde_json::Value> {
        self.conn
            .request("Stream.AddStream", json!({ "streamUri": stream_uri }))
            .await
    }

    /// Remove a stream.
    pub async fn stream_remove(&self, id: &str) -> Result<()> {
        self.conn
            .request("Stream.RemoveStream", json!({ "id": id }))
            .await?;
        Ok(())
    }

    /// Control a stream.
    pub async fn stream_control(
        &self,
        id: &str,
        command: &str,
        params: serde_json::Value,
    ) -> Result<()> {
        self.conn
            .request(
                "Stream.Control",
                json!({ "id": id, "command": command, "params": params }),
            )
            .await?;
        Ok(())
    }

    /// Set stream property.
    pub async fn stream_set_property(&self, id: &str, properties: serde_json::Value) -> Result<()> {
        self.conn
            .request(
                "Stream.SetProperty",
                json!({ "id": id, "properties": properties }),
            )
            .await?;
        Ok(())
    }
}

// ── TCP Audio Source ───────────────────────────────────────────

/// Open a TCP audio source connection to snapserver.
pub async fn open_audio_source(port: u16) -> Result<TcpStream> {
    let stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .context("Failed to connect audio source")?;
    tracing::info!(port, "Audio source connected");
    Ok(stream)
}

// ── State sync helpers (called from main.rs) ──────────────────

use crate::api;
use crate::player;
use crate::state;
use std::collections::HashMap;

/// Sync initial client state from Snapcast server status.
pub async fn sync_initial_state(status: &ServerStatus, store: &state::SharedState) {
    let mut s = store.write().await;
    for group in &status.server.groups {
        for snap_client in &group.clients {
            let mac = snap_client.host.mac.to_lowercase();
            let snap_id = snap_client.id.clone();
            let connected = snap_client.connected;
            if let Some(client) = s.clients.values_mut().find(|c| c.mac.to_lowercase() == mac) {
                client.snapcast_id = Some(snap_id.clone());
                client.connected = connected;
                tracing::info!(client = %client.name, snap_id = %snap_id, connected, "Initial client state synced");
            }
        }
    }
}

/// Build MAC → snapcast_id map from server status.
pub fn build_client_mac_map(status: &ServerStatus) -> HashMap<String, String> {
    status
        .server
        .groups
        .iter()
        .flat_map(|g| &g.clients)
        .map(|c| (c.host.mac.to_lowercase(), c.id.clone()))
        .collect()
}

/// Build list of group IDs from server status.
pub fn build_group_ids(status: &ServerStatus) -> Vec<String> {
    status.server.groups.iter().map(|g| g.id.clone()).collect()
}

/// Build group → client IDs map from server status.
pub fn build_group_clients(status: &ServerStatus) -> HashMap<String, Vec<String>> {
    status
        .server
        .groups
        .iter()
        .map(|g| {
            (
                g.id.clone(),
                g.clients.iter().map(|c| c.id.clone()).collect(),
            )
        })
        .collect()
}

/// Execute a Snapcast command and sync state after success.
pub async fn execute_command(
    snap: &SnapcastClient,
    cmd: player::SnapcastCmd,
    store: &state::SharedState,
    notify: &tokio::sync::broadcast::Sender<api::ws::Notification>,
) {
    let result = match &cmd {
        player::SnapcastCmd::Group { group_id, action } => match action {
            player::GroupAction::Stream(stream_id) => {
                snap.group_set_stream(group_id, stream_id).await
            }
            player::GroupAction::Clients(client_ids) => {
                snap.group_set_clients(group_id, client_ids.clone()).await
            }
            player::GroupAction::Name(name) => snap.group_set_name(group_id, name).await,
            player::GroupAction::Volume(_percent) => {
                // TODO: implement proper group volume
                Ok(())
            }
            player::GroupAction::Mute(muted) => snap.group_set_mute(group_id, *muted).await,
        },
        player::SnapcastCmd::Client { client_id, action } => match action {
            player::ClientAction::Volume(percent) => {
                snap.client_set_volume(client_id, (*percent).clamp(0, 100) as u8, false)
                    .await
            }
            player::ClientAction::Mute(muted) => {
                snap.client_set_volume(client_id, 100, *muted).await
            }
            player::ClientAction::Latency(ms) => snap.client_set_latency(client_id, *ms).await,
        },
    };
    match result {
        Ok(()) => {
            if let player::SnapcastCmd::Client { client_id, .. } = &cmd {
                sync_client_after_command(snap, client_id, store, notify).await;
            }
        }
        Err(e) => tracing::warn!(error = %e, "Snapcast command failed"),
    }
}

/// After a client command, re-fetch the client's state and broadcast changes.
async fn sync_client_after_command(
    snap: &SnapcastClient,
    snap_id: &str,
    store: &state::SharedState,
    notify: &tokio::sync::broadcast::Sender<api::ws::Notification>,
) {
    let snap_client = match snap.client_get_status(snap_id).await {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!(error = %e, "Failed to fetch client status after command");
            return;
        }
    };
    let volume = snap_client.config.volume.percent as i32;
    let muted = snap_client.config.volume.muted;
    let connected = snap_client.connected;
    let latency = snap_client.config.latency as i32;

    let mut s = store.write().await;
    if let Some((&idx, client)) = s
        .clients
        .iter_mut()
        .find(|(_, c)| c.snapcast_id.as_deref() == Some(snap_id))
    {
        let changed = client.volume != volume
            || client.muted != muted
            || client.connected != connected
            || client.latency_ms != latency;
        if changed {
            client.volume = volume;
            client.muted = muted;
            client.connected = connected;
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
            tracing::info!(client = %name, volume, muted, "Client state synced after command");
            let _ = notify.send(notif);
        }
    }
}

/// Handle a Snapcast server notification — update state + send WebSocket notification.
pub async fn handle_notification(
    notification: Notification,
    config: &AppConfig,
    snap: &SnapcastClient,
    store: &state::SharedState,
    notify: &tokio::sync::broadcast::Sender<api::ws::Notification>,
) {
    match notification {
        Notification::ClientOnConnect {
            id: _,
            client: snap_client,
        } => {
            let mac = snap_client.host.mac.to_lowercase();
            let snap_id = snap_client.id.clone();
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

                setup_zone_group_for_client(zone_index, &snap_id, config, snap).await;
            }
        }
        Notification::ClientOnDisconnect { id: snap_id } => {
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
        Notification::ClientOnVolumeChanged {
            id: snap_id,
            volume,
        } => {
            let vol = volume.percent as i32;
            let muted = volume.muted;
            let mut s = store.write().await;
            if let Some((&idx, client)) = s
                .clients
                .iter_mut()
                .find(|(_, c)| c.snapcast_id.as_deref() == Some(&snap_id))
            {
                client.volume = vol;
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
                tracing::info!(client = %name, volume = vol, muted, "Client volume changed");
                let _ = notify.send(notif);
            }
        }
        Notification::ClientOnLatencyChanged {
            id: snap_id,
            latency,
        } => {
            let lat = latency as i32;
            let mut s = store.write().await;
            if let Some((&idx, client)) = s
                .clients
                .iter_mut()
                .find(|(_, c)| c.snapcast_id.as_deref() == Some(&snap_id))
            {
                client.latency_ms = lat;
                let notif = api::ws::Notification::ClientStateChanged {
                    client: idx,
                    volume: client.volume,
                    muted: client.muted,
                    connected: client.connected,
                    zone: client.zone_index,
                };
                let name = client.name.clone();
                drop(s);
                tracing::info!(client = %name, latency = lat, "Client latency changed");
                let _ = notify.send(notif);
            }
        }
        Notification::ClientOnNameChanged {
            id: snap_id,
            name: new_name,
        } => {
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
        Notification::GroupOnMute { id, mute } => {
            tracing::info!(group = %id, mute, "Group mute changed");
        }
        Notification::GroupOnStreamChanged { id, stream_id } => {
            tracing::info!(group = %id, stream = %stream_id, "Group stream changed");
        }
        Notification::GroupOnNameChanged { id, name } => {
            tracing::info!(group = %id, name = %name, "Group name changed");
        }
        Notification::ServerOnUpdate { .. } => {
            tracing::info!("Snapcast server state updated");
        }
        Notification::StreamOnUpdate { id, stream } => {
            tracing::info!(stream = %id, status = ?stream.status, "Stream status updated");
        }
        Notification::StreamOnProperties { id, .. } => {
            tracing::debug!(stream = %id, "Stream properties updated");
        }
        Notification::Unknown { method, .. } => {
            tracing::debug!(method = %method, "Unknown Snapcast notification");
        }
    }
}

/// Setup zone group when a client connects.
async fn setup_zone_group_for_client(
    zone_index: usize,
    snap_client_id: &str,
    config: &AppConfig,
    snap: &SnapcastClient,
) {
    // Re-fetch current state to get fresh group info
    let status = match snap.server_get_status().await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to refresh Snapcast state for group setup");
            return;
        }
    };

    let zone_config = &config.zones[zone_index - 1];

    let zone_macs: Vec<String> = config
        .clients
        .iter()
        .filter(|c| c.zone_index == zone_index)
        .map(|c| c.mac.to_lowercase())
        .collect();

    let snap_client_ids: Vec<String> = zone_macs
        .iter()
        .filter_map(|mac| {
            status
                .server
                .groups
                .iter()
                .flat_map(|g| &g.clients)
                .find(|c| c.host.mac.to_lowercase() == *mac)
                .map(|c| c.id.clone())
        })
        .collect();

    if snap_client_ids.is_empty() {
        return;
    }

    let gid = status
        .server
        .groups
        .iter()
        .find(|g| g.clients.iter().any(|c| c.id == snap_client_id))
        .map(|g| g.id.clone())
        .or_else(|| status.server.groups.first().map(|g| g.id.clone()));

    let Some(gid) = gid else {
        tracing::warn!(
            zone = zone_index,
            "No Snapcast groups available for zone setup"
        );
        return;
    };

    if let Err(e) = snap.group_set_clients(&gid, snap_client_ids.clone()).await {
        tracing::warn!(error = %e, "Failed to set group clients");
    }
    if let Err(e) = snap.group_set_stream(&gid, &zone_config.stream_name).await {
        tracing::warn!(error = %e, "Failed to set group stream");
    }
    if let Err(e) = snap.group_set_name(&gid, &zone_config.name).await {
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
