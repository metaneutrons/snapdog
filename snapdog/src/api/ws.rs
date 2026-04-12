// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! WebSocket endpoint for real-time state notifications.

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::IntoResponse;
use axum::routing::get;
use serde::Serialize;
use tokio::sync::broadcast;

use crate::api::SharedState;

/// Notification broadcast to all connected WebSocket clients.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Notification {
    /// Zone playback state changed (play/pause/stop, volume, mute, source, shuffle, repeat).
    ZoneStateChanged {
        /// Zone index (1-based).
        zone: usize,
        /// Playback state: "playing", "paused", or "stopped".
        playback: String,
        /// Zone volume (0–100).
        volume: i32,
        /// Whether the zone is muted.
        muted: bool,
        /// Active source name (e.g. radio station, playlist, "airplay").
        source: String,
        /// Whether shuffle is enabled.
        shuffle: bool,
        /// Whether playlist repeat is enabled.
        repeat: bool,
        /// Whether single-track repeat is enabled.
        track_repeat: bool,
    },
    /// Current track metadata changed for a zone.
    ZoneTrackChanged {
        /// Zone index (1-based).
        zone: usize,
        /// Track title.
        title: String,
        /// Track artist.
        artist: String,
        /// Track album.
        album: String,
        /// Total track duration in milliseconds.
        duration_ms: i64,
        /// Current playback position in milliseconds.
        position_ms: i64,
        /// Whether the track supports seeking.
        seekable: bool,
        /// Cover art URL, if available.
        cover_url: Option<String>,
    },
    /// Periodic playback position update for a zone.
    ZoneProgress {
        /// Zone index (1-based).
        zone: usize,
        /// Current playback position in milliseconds.
        position_ms: i64,
        /// Total track duration in milliseconds.
        duration_ms: i64,
    },
    /// Client connection or volume state changed.
    ClientStateChanged {
        /// Client index (1-based).
        client: usize,
        /// Client volume (0–100).
        volume: i32,
        /// Whether the client is muted.
        muted: bool,
        /// Whether the client is connected to Snapcast.
        connected: bool,
        /// Zone the client belongs to (1-based).
        zone: usize,
    },
    /// Zone equalizer configuration changed.
    ZoneEqChanged {
        /// Zone index (1-based).
        zone: usize,
        /// Updated EQ configuration (flattened into the JSON object).
        #[serde(flatten)]
        config: crate::audio::eq::EqConfig,
    },
}

/// Create a broadcast channel for notifications.
pub type NotifySender = broadcast::Sender<Notification>;

/// Create a broadcast channel for notifications.
pub fn notification_channel() -> (
    broadcast::Sender<Notification>,
    broadcast::Receiver<Notification>,
) {
    broadcast::channel(256)
}

/// Build the WebSocket router.
pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state)
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<SharedState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: SharedState) {
    let mut rx = state.notifications.subscribe();
    let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(30));
    tracing::debug!("WebSocket client connected");

    loop {
        tokio::select! {
            Ok(notification) = rx.recv() => {
                let json = match serde_json::to_string(&notification) {
                    Ok(j) => j,
                    Err(_) => continue,
                };
                if socket.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
            _ = ping_interval.tick() => {
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }

    tracing::debug!("WebSocket client disconnected");
}
