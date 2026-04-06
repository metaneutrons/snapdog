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
    ZoneStateChanged {
        zone: usize,
        playback: String,
        volume: i32,
        muted: bool,
        source: String,
        shuffle: bool,
        repeat: bool,
        track_repeat: bool,
    },
    ZoneTrackChanged {
        zone: usize,
        title: String,
        artist: String,
        album: String,
        duration_ms: i64,
        position_ms: i64,
    },
    ZoneProgress {
        zone: usize,
        position_ms: i64,
        duration_ms: i64,
    },
    ClientStateChanged {
        client: usize,
        volume: i32,
        muted: bool,
        connected: bool,
        zone: usize,
    },
}

/// Create a broadcast channel for notifications.
pub fn notification_channel() -> (
    broadcast::Sender<Notification>,
    broadcast::Receiver<Notification>,
) {
    broadcast::channel(256)
}

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
    tracing::info!("WebSocket client connected");

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

    tracing::info!("WebSocket client disconnected");
}
