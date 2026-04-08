// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Snapcast domain types.
//!
//! These types mirror the Snapcast JSON-RPC API wire format.
//! Field names match the Snapcast protocol specification.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ── Client ────────────────────────────────────────────────────

/// A Snapcast client (speaker endpoint).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct Client {
    pub id: String,
    pub connected: bool,
    pub config: ClientConfig,
    pub host: Host,
    pub snapclient: Snapclient,
    #[serde(rename = "lastSeen")]
    pub last_seen: LastSeen,
}

/// Host information for a Snapcast client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct Host {
    pub arch: String,
    pub ip: String,
    pub mac: String,
    pub name: String,
    pub os: String,
}

/// Client-side configuration (instance, latency, volume).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct ClientConfig {
    pub instance: usize,
    pub latency: usize,
    pub name: String,
    pub volume: ClientVolume,
}

/// Client volume state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct ClientVolume {
    pub muted: bool,
    pub percent: usize,
}

/// Snapclient software information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct Snapclient {
    pub name: String,
    #[serde(rename = "protocolVersion")]
    pub protocol_version: usize,
    pub version: String,
}

/// Last-seen timestamp (seconds + microseconds since epoch).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct LastSeen {
    pub sec: usize,
    pub usec: usize,
}

// ── Group ─────────────────────────────────────────────────────

/// A Snapcast group (synchronized playback unit).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub stream_id: String,
    pub muted: bool,
    pub clients: Vec<Client>,
}

// ── Stream ────────────────────────────────────────────────────

/// A Snapcast audio stream source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct Stream {
    pub id: String,
    pub properties: Option<StreamProperties>,
    pub status: StreamStatus,
    pub uri: StreamUri,
}

/// Stream playback status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamStatus {
    /// No audio data flowing.
    Idle,
    /// Audio data actively streaming.
    Playing,
    /// Stream disabled by configuration.
    Disabled,
    /// Status not recognized.
    Unknown,
}

/// Stream source URI components.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct StreamUri {
    pub fragment: String,
    pub host: String,
    pub path: String,
    pub query: HashMap<String, String>,
    pub raw: String,
    pub scheme: String,
}

/// Stream properties (MPRIS-style metadata and capabilities).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct StreamProperties {
    pub playback_status: Option<String>,
    pub loop_status: Option<String>,
    pub shuffle: Option<bool>,
    pub volume: Option<usize>,
    pub mute: Option<bool>,
    pub rate: Option<f64>,
    pub position: Option<f64>,
    #[serde(default)]
    pub can_go_next: bool,
    #[serde(default)]
    pub can_go_previous: bool,
    #[serde(default)]
    pub can_play: bool,
    #[serde(default)]
    pub can_pause: bool,
    #[serde(default)]
    pub can_seek: bool,
    #[serde(default)]
    pub can_control: bool,
    pub metadata: Option<serde_json::Value>,
}

// ── Server ────────────────────────────────────────────────────

/// Snapcast server state (groups + streams).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct Server {
    pub server: ServerDetails,
    pub groups: Vec<Group>,
    pub streams: Vec<Stream>,
}

/// Snapserver host and version information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct ServerDetails {
    pub host: Host,
    pub snapserver: Snapserver,
}

/// Snapserver software information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct Snapserver {
    pub name: String,
    #[serde(rename = "protocolVersion")]
    pub protocol_version: usize,
    #[serde(rename = "controlProtocolVersion")]
    pub control_protocol_version: usize,
    pub version: String,
}

/// Result of `Server.GetStatus`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStatus {
    /// Full server state.
    pub server: Server,
}
