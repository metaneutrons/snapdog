// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Snapcast JSON-RPC client and TCP audio source management.

pub mod connection;
pub mod protocol;
pub mod types;

use std::net::SocketAddr;

use anyhow::{Context, Result};
use serde_json::json;
use tokio::io::AsyncWriteExt;
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
