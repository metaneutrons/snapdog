// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Snapcast integration: server lifecycle, control API, audio source feeding.
//!
//! - JSON-RPC control via `snapcast-control` crate (auto-managed state)
//! - Feeds PCM audio to TCP sources (loopback-only)

use std::net::SocketAddr;

use anyhow::{Context, Result};
use snapcast_control::{SnapcastConnection, State};
use tokio::net::TcpStream;

use crate::config::AppConfig;

/// Snapcast controller: JSON-RPC connection + TCP audio sources.
pub struct Snapcast {
    conn: SnapcastConnection,
}

impl Snapcast {
    /// Connect to snapserver JSON-RPC and fetch initial state.
    #[tracing::instrument(skip_all, fields(address = %addr))]
    pub async fn connect(addr: SocketAddr) -> Result<Self> {
        let conn = SnapcastConnection::builder()
            .on_connect(|| tracing::info!("Snapcast connected"))
            .on_disconnect(|| tracing::warn!("Snapcast disconnected — reconnecting"))
            .connect(addr)
            .await
            .context("Failed to connect to snapserver JSON-RPC")?;

        tracing::info!("Snapcast JSON-RPC connection established");
        Ok(Self { conn })
    }

    /// Connect using app config.
    pub async fn from_config(config: &AppConfig) -> Result<Self> {
        let addr: SocketAddr = format!(
            "{}:{}",
            config.snapcast.address, config.snapcast.jsonrpc_port
        )
        .parse()
        .context("Invalid snapcast address")?;
        Self::connect(addr).await
    }

    /// Get shared state reference (auto-updated by the crate on every message).
    pub fn state(&self) -> &std::sync::Arc<State> {
        &self.conn.state
    }

    /// Fetch initial server status and populate state.
    pub async fn init(&mut self) -> Result<()> {
        self.conn
            .server_get_status()
            .await
            .context("Failed to get server status")?;
        self.log_state();
        Ok(())
    }

    /// Process incoming messages (call in event loop).
    pub async fn recv(
        &mut self,
    ) -> Option<Vec<Result<snapcast_control::ValidMessage, snapcast_control::ClientError>>> {
        self.conn.recv().await
    }

    /// Set volume for a client by snapcast ID.
    pub async fn set_client_volume(&mut self, id: &str, percent: u8, muted: bool) -> Result<()> {
        self.conn
            .client_set_volume(
                id.to_string(),
                snapcast_control::client::ClientVolume {
                    percent: percent.into(),
                    muted,
                },
            )
            .await
            .context("Failed to set client volume")
    }

    /// Assign a group to a specific stream.
    pub async fn set_group_stream(&mut self, group_id: &str, stream_id: &str) -> Result<()> {
        self.conn
            .group_set_stream(group_id.to_string(), stream_id.to_string())
            .await
            .context("Failed to set group stream")
    }

    /// Set clients for a group.
    pub async fn set_group_clients(
        &mut self,
        group_id: &str,
        client_ids: Vec<String>,
    ) -> Result<()> {
        self.conn
            .group_set_clients(group_id.to_string(), client_ids)
            .await
            .context("Failed to set group clients")
    }

    /// Set group name.
    pub async fn set_group_name(&mut self, group_id: &str, name: &str) -> Result<()> {
        self.conn
            .group_set_name(group_id.to_string(), name.to_string())
            .await
            .context("Failed to set group name")
    }

    fn log_state(&self) {
        let state = self.state();
        let groups: Vec<_> = state.groups.iter().map(|g| g.key().clone()).collect();
        let clients: Vec<_> = state.clients.iter().map(|c| c.key().clone()).collect();
        let streams: Vec<_> = state.streams.iter().map(|s| s.key().clone()).collect();
        tracing::info!(
            groups = ?groups,
            clients = ?clients,
            streams = ?streams,
            "Snapcast state loaded"
        );
    }
}

/// Open a TCP connection to a snapcast TCP source for feeding PCM audio.
pub async fn open_audio_source(port: u16) -> Result<TcpStream> {
    let addr = format!("127.0.0.1:{port}");
    let stream = TcpStream::connect(&addr)
        .await
        .with_context(|| format!("Failed to connect to TCP source at {addr}"))?;
    tracing::info!(port, "Audio source connected");
    Ok(stream)
}
