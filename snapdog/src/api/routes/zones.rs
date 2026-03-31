// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Zone endpoints: /api/v1/zones

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;

use crate::api::SharedState;

#[derive(Serialize)]
struct ZoneInfo {
    index: usize,
    name: String,
    icon: String,
}

pub fn router(state: SharedState) -> Router {
    Router::new()
        // Zone listing
        .route("/count", get(get_count))
        .route("/", get(get_all))
        .route("/{zone_index}", get(get_zone))
        // Volume
        .route("/{zone_index}/volume", get(get_volume).put(set_volume))
        .route("/{zone_index}/mute", get(get_mute).put(set_mute))
        .route("/{zone_index}/mute/toggle", post(toggle_mute))
        // Playback control
        .route("/{zone_index}/play", post(play))
        .route("/{zone_index}/pause", post(pause))
        .route("/{zone_index}/stop", post(stop))
        .route("/{zone_index}/next", post(next_track))
        .route("/{zone_index}/previous", post(previous_track))
        // Playlist
        .route("/{zone_index}/playlist", get(get_playlist).put(set_playlist))
        .route("/{zone_index}/next/playlist", post(next_playlist))
        .route("/{zone_index}/previous/playlist", post(previous_playlist))
        .route("/{zone_index}/shuffle", get(get_shuffle).put(set_shuffle))
        .route("/{zone_index}/shuffle/toggle", post(toggle_shuffle))
        .route("/{zone_index}/repeat", get(get_repeat).put(set_repeat))
        .route("/{zone_index}/repeat/toggle", post(toggle_repeat))
        // Track info
        .route("/{zone_index}/track", get(get_track))
        .route("/{zone_index}/track/metadata", get(get_track_metadata))
        .route("/{zone_index}/track/title", get(get_track_title))
        .route("/{zone_index}/track/artist", get(get_track_artist))
        .route("/{zone_index}/track/album", get(get_track_album))
        .route("/{zone_index}/track/cover", get(get_track_cover))
        .route("/{zone_index}/track/duration", get(get_track_duration))
        .route("/{zone_index}/track/position", get(get_track_position).put(seek_position))
        .route("/{zone_index}/track/progress", get(get_track_progress).put(seek_progress))
        .route("/{zone_index}/track/playing", get(get_track_playing))
        .route("/{zone_index}/track/repeat", get(get_track_repeat).put(set_track_repeat))
        .route("/{zone_index}/track/repeat/toggle", post(toggle_track_repeat))
        // Play specific content
        .route("/{zone_index}/play/track", post(play_track))
        .route("/{zone_index}/play/url", post(play_url))
        .route("/{zone_index}/play/playlist/{playlist_index}/track", post(play_playlist_track))
        // Zone info
        .route("/{zone_index}/name", get(get_name))
        .route("/{zone_index}/icon", get(get_icon))
        .route("/{zone_index}/playback", get(get_playback))
        .route("/{zone_index}/playlist/name", get(get_playlist_name))
        .route("/{zone_index}/playlist/info", get(get_playlist_info))
        .route("/{zone_index}/playlist/count", get(get_playlist_count))
        .route("/{zone_index}/clients", get(get_clients))
        .with_state(state)
}

// Helper to validate zone index
fn validate_zone(
    state: &SharedState,
    idx: usize,
) -> Result<&crate::config::ZoneConfig, StatusCode> {
    state.config.zones.get(idx - 1).ok_or(StatusCode::NOT_FOUND)
}

// ── Zone listing ──────────────────────────────────────────────

async fn get_count(State(state): State<SharedState>) -> Json<usize> {
    Json(state.config.zones.len())
}

async fn get_all(State(state): State<SharedState>) -> Json<Vec<ZoneInfo>> {
    Json(
        state
            .config
            .zones
            .iter()
            .map(|z| ZoneInfo {
                index: z.index,
                name: z.name.clone(),
                icon: z.icon.clone(),
            })
            .collect(),
    )
}

async fn get_zone(State(state): State<SharedState>, Path(idx): Path<usize>) -> impl IntoResponse {
    match validate_zone(&state, idx) {
        Ok(z) => Ok(Json(ZoneInfo {
            index: z.index,
            name: z.name.clone(),
            icon: z.icon.clone(),
        })),
        Err(s) => Err(s),
    }
}

// ── Volume ────────────────────────────────────────────────────

async fn get_volume(State(_state): State<SharedState>, Path(_idx): Path<usize>) -> Json<i32> {
    Json(50) // TODO: read from state
}

async fn set_volume(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
    Json(volume): Json<i32>,
) -> impl IntoResponse {
    tracing::info!(volume, "Set zone volume");
    StatusCode::NO_CONTENT // TODO: apply
}

async fn get_mute(State(_state): State<SharedState>, Path(_idx): Path<usize>) -> Json<bool> {
    Json(false) // TODO
}

async fn set_mute(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
    Json(muted): Json<bool>,
) -> impl IntoResponse {
    tracing::info!(muted, "Set zone mute");
    StatusCode::NO_CONTENT
}

async fn toggle_mute(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT // TODO
}

// ── Playback control ──────────────────────────────────────────

async fn play(State(_state): State<SharedState>, Path(_idx): Path<usize>) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
async fn pause(State(_state): State<SharedState>, Path(_idx): Path<usize>) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
async fn stop(State(_state): State<SharedState>, Path(_idx): Path<usize>) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
async fn next_track(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
async fn previous_track(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

// ── Playlist ──────────────────────────────────────────────────

async fn get_playlist(State(_state): State<SharedState>, Path(_idx): Path<usize>) -> Json<i32> {
    Json(0)
}
async fn set_playlist(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
    Json(_v): Json<i32>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
async fn next_playlist(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
async fn previous_playlist(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
async fn get_shuffle(State(_state): State<SharedState>, Path(_idx): Path<usize>) -> Json<bool> {
    Json(false)
}
async fn set_shuffle(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
    Json(_v): Json<bool>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
async fn toggle_shuffle(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
async fn get_repeat(State(_state): State<SharedState>, Path(_idx): Path<usize>) -> Json<bool> {
    Json(false)
}
async fn set_repeat(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
    Json(_v): Json<bool>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
async fn toggle_repeat(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

// ── Track info ────────────────────────────────────────────────

async fn get_track(State(_state): State<SharedState>, Path(_idx): Path<usize>) -> Json<i32> {
    Json(0)
}
async fn get_track_metadata(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({}))
}
async fn get_track_title(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> Json<String> {
    Json(String::new())
}
async fn get_track_artist(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> Json<String> {
    Json(String::new())
}
async fn get_track_album(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> Json<String> {
    Json(String::new())
}
async fn get_track_cover(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> Json<String> {
    Json(String::new())
}
async fn get_track_duration(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> Json<i64> {
    Json(0)
}
async fn get_track_position(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> Json<i64> {
    Json(0)
}
async fn seek_position(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
    Json(_v): Json<i64>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
async fn get_track_progress(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> Json<f64> {
    Json(0.0)
}
async fn seek_progress(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
    Json(_v): Json<f64>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
async fn get_track_playing(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> Json<bool> {
    Json(false)
}
async fn get_track_repeat(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> Json<bool> {
    Json(false)
}
async fn set_track_repeat(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
    Json(_v): Json<bool>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
async fn toggle_track_repeat(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

// ── Play specific content ─────────────────────────────────────

async fn play_track(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
    Json(_v): Json<i32>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
async fn play_url(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
    Json(_v): Json<String>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
async fn play_playlist_track(
    State(_state): State<SharedState>,
    Path((_zone, _playlist)): Path<(usize, usize)>,
    Json(_v): Json<i32>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

// ── Zone info ─────────────────────────────────────────────────

async fn get_name(State(state): State<SharedState>, Path(idx): Path<usize>) -> impl IntoResponse {
    validate_zone(&state, idx).map(|z| Json(z.name.clone()))
}

async fn get_icon(State(state): State<SharedState>, Path(idx): Path<usize>) -> impl IntoResponse {
    validate_zone(&state, idx).map(|z| Json(z.icon.clone()))
}

async fn get_playback(State(_state): State<SharedState>, Path(_idx): Path<usize>) -> Json<String> {
    Json("stopped".into())
}
async fn get_playlist_name(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> Json<String> {
    Json(String::new())
}
async fn get_playlist_info(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({}))
}
async fn get_playlist_count(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> Json<i32> {
    Json(0)
}
async fn get_clients(State(state): State<SharedState>, Path(idx): Path<usize>) -> Json<Vec<usize>> {
    Json(
        state
            .config
            .clients
            .iter()
            .filter(|c| c.zone_index == idx)
            .map(|c| c.index)
            .collect(),
    )
}
