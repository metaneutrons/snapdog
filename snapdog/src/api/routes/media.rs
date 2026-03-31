// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Media endpoints: /api/v1/media

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::api::SharedState;

#[derive(Serialize)]
struct PlaylistInfo {
    index: usize,
    name: String,
}

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/playlists", get(get_playlists))
        .route("/playlists/{playlist_index}", get(get_playlist))
        .route(
            "/playlists/{playlist_index}/tracks",
            get(get_playlist_tracks),
        )
        .route(
            "/playlists/{playlist_index}/tracks/{track_index}",
            get(get_playlist_track),
        )
        .route("/tracks/{track_index}", get(get_track))
        .with_state(state)
}

async fn get_playlists(State(_state): State<SharedState>) -> Json<Vec<PlaylistInfo>> {
    Json(vec![]) // TODO: from subsonic
}

async fn get_playlist(
    State(_state): State<SharedState>,
    Path(_idx): Path<String>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({}))
}

async fn get_playlist_tracks(
    State(_state): State<SharedState>,
    Path(_idx): Path<String>,
) -> Json<Vec<serde_json::Value>> {
    Json(vec![])
}

async fn get_playlist_track(
    State(_state): State<SharedState>,
    Path((_playlist, _track)): Path<(String, usize)>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({}))
}

async fn get_track(
    State(_state): State<SharedState>,
    Path(_idx): Path<String>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({}))
}
