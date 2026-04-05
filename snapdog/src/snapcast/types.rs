// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Snapcast domain types.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ── Client ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Client {
    pub id: String,
    pub connected: bool,
    pub config: ClientConfig,
    pub host: Host,
    pub snapclient: Snapclient,
    #[serde(rename = "lastSeen")]
    pub last_seen: LastSeen,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Host {
    pub arch: String,
    pub ip: String,
    pub mac: String,
    pub name: String,
    pub os: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub instance: usize,
    pub latency: usize,
    pub name: String,
    pub volume: ClientVolume,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientVolume {
    pub muted: bool,
    pub percent: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapclient {
    pub name: String,
    #[serde(rename = "protocolVersion")]
    pub protocol_version: usize,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastSeen {
    pub sec: usize,
    pub usec: usize,
}

// ── Group ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub stream_id: String,
    pub muted: bool,
    pub clients: Vec<Client>,
}

// ── Stream ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stream {
    pub id: String,
    pub properties: Option<StreamProperties>,
    pub status: StreamStatus,
    pub uri: StreamUri,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamStatus {
    Idle,
    Playing,
    Disabled,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamUri {
    pub fragment: String,
    pub host: String,
    pub path: String,
    pub query: HashMap<String, String>,
    pub raw: String,
    pub scheme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamProperties {
    pub playback_status: Option<String>,
    pub loop_status: Option<String>,
    pub shuffle: Option<bool>,
    pub volume: Option<usize>,
    pub mute: Option<bool>,
    pub rate: Option<f64>,
    pub position: Option<f64>,
    pub can_go_next: bool,
    pub can_go_previous: bool,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_seek: bool,
    pub can_control: bool,
    pub metadata: Option<serde_json::Value>,
}

// ── Server ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub server: ServerDetails,
    pub groups: Vec<Group>,
    pub streams: Vec<Stream>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerDetails {
    pub host: Host,
    pub snapserver: Snapserver,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapserver {
    pub name: String,
    #[serde(rename = "protocolVersion")]
    pub protocol_version: usize,
    #[serde(rename = "controlProtocolVersion")]
    pub control_protocol_version: usize,
    pub version: String,
}

/// Result of Server.GetStatus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStatus {
    pub server: Server,
}
