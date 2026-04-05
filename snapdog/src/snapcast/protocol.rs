// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! JSON-RPC 2.0 message types and Snapcast method/notification enums.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::types;

// ── Generic JSON-RPC 2.0 ─────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct Request {
    pub jsonrpc: &'static str,
    pub id: uuid::Uuid,
    pub method: &'static str,
    pub params: Value,
}

impl Request {
    pub fn new(method: &'static str, params: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id: uuid::Uuid::new_v4(),
            method,
            params,
        }
    }
}

/// Raw JSON-RPC message from the server (before routing).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum RawMessage {
    Response {
        id: uuid::Uuid,
        result: Option<Value>,
        error: Option<RpcError>,
    },
    Notification {
        method: String,
        params: Value,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON-RPC error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for RpcError {}

// ── Snapcast Notifications ───────────────────────────────────

/// Typed Snapcast server notification.
#[derive(Debug, Clone)]
pub enum Notification {
    ClientOnConnect {
        id: String,
        client: types::Client,
    },
    ClientOnDisconnect {
        id: String,
    },
    ClientOnVolumeChanged {
        id: String,
        volume: types::ClientVolume,
    },
    ClientOnLatencyChanged {
        id: String,
        latency: usize,
    },
    ClientOnNameChanged {
        id: String,
        name: String,
    },
    GroupOnMute {
        id: String,
        mute: bool,
    },
    GroupOnStreamChanged {
        id: String,
        stream_id: String,
    },
    GroupOnNameChanged {
        id: String,
        name: String,
    },
    ServerOnUpdate {
        server: types::Server,
    },
    StreamOnUpdate {
        id: String,
        stream: types::Stream,
    },
    StreamOnProperties {
        id: String,
        properties: types::StreamProperties,
    },
    Unknown {
        method: String,
        params: Value,
    },
}

impl Notification {
    /// Parse a raw JSON-RPC notification into a typed Snapcast notification.
    pub fn parse(method: &str, params: Value) -> Self {
        match method {
            "Client.OnConnect" => {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    client: types::Client,
                }
                match serde_json::from_value::<P>(params.clone()) {
                    Ok(p) => Self::ClientOnConnect {
                        id: p.id,
                        client: p.client,
                    },
                    Err(_) => Self::Unknown {
                        method: method.to_string(),
                        params,
                    },
                }
            }
            "Client.OnDisconnect" => {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                }
                match serde_json::from_value::<P>(params.clone()) {
                    Ok(p) => Self::ClientOnDisconnect { id: p.id },
                    Err(_) => Self::Unknown {
                        method: method.to_string(),
                        params,
                    },
                }
            }
            "Client.OnVolumeChanged" => {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    volume: types::ClientVolume,
                }
                match serde_json::from_value::<P>(params.clone()) {
                    Ok(p) => Self::ClientOnVolumeChanged {
                        id: p.id,
                        volume: p.volume,
                    },
                    Err(_) => Self::Unknown {
                        method: method.to_string(),
                        params,
                    },
                }
            }
            "Client.OnLatencyChanged" => {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    latency: usize,
                }
                match serde_json::from_value::<P>(params.clone()) {
                    Ok(p) => Self::ClientOnLatencyChanged {
                        id: p.id,
                        latency: p.latency,
                    },
                    Err(_) => Self::Unknown {
                        method: method.to_string(),
                        params,
                    },
                }
            }
            "Client.OnNameChanged" => {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    name: String,
                }
                match serde_json::from_value::<P>(params.clone()) {
                    Ok(p) => Self::ClientOnNameChanged {
                        id: p.id,
                        name: p.name,
                    },
                    Err(_) => Self::Unknown {
                        method: method.to_string(),
                        params,
                    },
                }
            }
            "Group.OnMute" => {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    mute: bool,
                }
                match serde_json::from_value::<P>(params.clone()) {
                    Ok(p) => Self::GroupOnMute {
                        id: p.id,
                        mute: p.mute,
                    },
                    Err(_) => Self::Unknown {
                        method: method.to_string(),
                        params,
                    },
                }
            }
            "Group.OnStreamChanged" => {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    stream_id: String,
                }
                match serde_json::from_value::<P>(params.clone()) {
                    Ok(p) => Self::GroupOnStreamChanged {
                        id: p.id,
                        stream_id: p.stream_id,
                    },
                    Err(_) => Self::Unknown {
                        method: method.to_string(),
                        params,
                    },
                }
            }
            "Group.OnNameChanged" => {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    name: String,
                }
                match serde_json::from_value::<P>(params.clone()) {
                    Ok(p) => Self::GroupOnNameChanged {
                        id: p.id,
                        name: p.name,
                    },
                    Err(_) => Self::Unknown {
                        method: method.to_string(),
                        params,
                    },
                }
            }
            "Server.OnUpdate" => {
                #[derive(Deserialize)]
                struct P {
                    server: types::Server,
                }
                match serde_json::from_value::<P>(params.clone()) {
                    Ok(p) => Self::ServerOnUpdate { server: p.server },
                    Err(_) => Self::Unknown {
                        method: method.to_string(),
                        params,
                    },
                }
            }
            "Stream.OnUpdate" => {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    stream: types::Stream,
                }
                match serde_json::from_value::<P>(params.clone()) {
                    Ok(p) => Self::StreamOnUpdate {
                        id: p.id,
                        stream: p.stream,
                    },
                    Err(_) => Self::Unknown {
                        method: method.to_string(),
                        params,
                    },
                }
            }
            "Stream.OnProperties" => {
                #[derive(Deserialize)]
                struct P {
                    id: String,
                    properties: types::StreamProperties,
                }
                match serde_json::from_value::<P>(params.clone()) {
                    Ok(p) => Self::StreamOnProperties {
                        id: p.id,
                        properties: p.properties,
                    },
                    Err(_) => Self::Unknown {
                        method: method.to_string(),
                        params,
                    },
                }
            }
            _ => Self::Unknown {
                method: method.to_string(),
                params,
            },
        }
    }
}
