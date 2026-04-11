// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Snapcast domain types matching the JSON-RPC wire format.
//!
//! When `snapcast-embedded` is enabled, re-exported from `snapcast-server::status`.
//! When only `snapcast-process` is enabled, defined locally for JSON-RPC deserialization.

#[cfg(feature = "snapcast-embedded")]
pub use snapcast_server::status::*;

#[cfg(not(feature = "snapcast-embedded"))]
mod local {
    use std::collections::HashMap;

    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ServerStatus {
        pub server: Server,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Server {
        #[serde(default)]
        pub server: ServerInfo,
        pub groups: Vec<Group>,
        pub streams: Vec<Stream>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ServerInfo {
        pub host: Host,
        #[serde(default)]
        pub snapserver: Snapserver,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Snapserver {
        #[serde(default)]
        pub name: String,
        #[serde(default, rename = "protocolVersion")]
        pub protocol_version: u32,
        #[serde(default, rename = "controlProtocolVersion")]
        pub control_protocol_version: u32,
        #[serde(default)]
        pub version: String,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Host {
        #[serde(default)]
        pub arch: String,
        #[serde(default)]
        pub ip: String,
        #[serde(default)]
        pub mac: String,
        #[serde(default)]
        pub name: String,
        #[serde(default)]
        pub os: String,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Client {
        pub id: String,
        pub connected: bool,
        pub config: ClientConfig,
        pub host: Host,
        #[serde(default)]
        pub snapclient: Snapclient,
        #[serde(default, rename = "lastSeen")]
        pub last_seen: LastSeen,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ClientConfig {
        #[serde(default)]
        pub instance: u32,
        pub latency: i32,
        pub name: String,
        pub volume: Volume,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Volume {
        pub muted: bool,
        pub percent: u16,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Snapclient {
        #[serde(default)]
        pub name: String,
        #[serde(default, rename = "protocolVersion")]
        pub protocol_version: u32,
        #[serde(default)]
        pub version: String,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct LastSeen {
        #[serde(default)]
        pub sec: u64,
        #[serde(default)]
        pub usec: u64,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Group {
        pub id: String,
        pub name: String,
        pub stream_id: String,
        pub muted: bool,
        pub clients: Vec<Client>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Stream {
        pub id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub properties: Option<StreamProperties>,
        pub status: StreamStatus,
        pub uri: StreamUri,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "lowercase")]
    pub enum StreamStatus {
        #[default]
        Idle,
        Playing,
        Disabled,
        Unknown,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct StreamUri {
        #[serde(default)]
        pub fragment: String,
        #[serde(default)]
        pub host: String,
        #[serde(default)]
        pub path: String,
        #[serde(default)]
        pub query: HashMap<String, String>,
        pub raw: String,
        #[serde(default)]
        pub scheme: String,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct StreamProperties {
        #[serde(default)]
        pub playback_status: Option<String>,
        #[serde(default)]
        pub loop_status: Option<String>,
        #[serde(default)]
        pub shuffle: Option<bool>,
        #[serde(default)]
        pub volume: Option<u16>,
        #[serde(default)]
        pub mute: Option<bool>,
        #[serde(default)]
        pub rate: Option<f64>,
        #[serde(default)]
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
        #[serde(default)]
        pub metadata: Option<serde_json::Value>,
    }
}

#[cfg(not(feature = "snapcast-embedded"))]
pub use local::*;
