// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Client endpoints: /api/v1/clients

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;

use crate::api::SharedState;

#[derive(Serialize)]
struct ClientInfo {
    index: usize,
    name: String,
    mac: String,
    zone_index: usize,
    icon: String,
}

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/count", get(get_count))
        .route("/", get(get_all))
        .route("/{client_index}", get(get_client))
        // Volume
        .route("/{client_index}/volume", get(get_volume).put(set_volume))
        .route("/{client_index}/mute", get(get_mute).put(set_mute))
        .route("/{client_index}/mute/toggle", post(toggle_mute))
        // Latency
        .route("/{client_index}/latency", get(get_latency).put(set_latency))
        // Zone assignment
        .route("/{client_index}/zone", get(get_zone).put(set_zone))
        // Info
        .route("/{client_index}/name", get(get_name).put(set_name))
        .route("/{client_index}/icon", get(get_icon))
        .route("/{client_index}/connected", get(get_connected))
        .with_state(state)
}

fn validate_client(
    state: &SharedState,
    idx: usize,
) -> Result<&crate::config::ClientConfig, StatusCode> {
    state
        .config
        .clients
        .get(idx - 1)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn get_count(State(state): State<SharedState>) -> Json<usize> {
    Json(state.config.clients.len())
}

async fn get_all(State(state): State<SharedState>) -> Json<Vec<ClientInfo>> {
    Json(
        state
            .config
            .clients
            .iter()
            .map(|c| ClientInfo {
                index: c.index,
                name: c.name.clone(),
                mac: c.mac.clone(),
                zone_index: c.zone_index,
                icon: c.icon.clone(),
            })
            .collect(),
    )
}

async fn get_client(State(state): State<SharedState>, Path(idx): Path<usize>) -> impl IntoResponse {
    validate_client(&state, idx).map(|c| {
        Json(ClientInfo {
            index: c.index,
            name: c.name.clone(),
            mac: c.mac.clone(),
            zone_index: c.zone_index,
            icon: c.icon.clone(),
        })
    })
}

// Volume
async fn get_volume(State(_state): State<SharedState>, Path(_idx): Path<usize>) -> Json<i32> {
    Json(50)
}
async fn set_volume(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
    Json(v): Json<i32>,
) -> impl IntoResponse {
    tracing::info!(volume = v, "Set client volume");
    StatusCode::NO_CONTENT
}
async fn get_mute(State(_state): State<SharedState>, Path(_idx): Path<usize>) -> Json<bool> {
    Json(false)
}
async fn set_mute(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
    Json(_v): Json<bool>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
async fn toggle_mute(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

// Latency
async fn get_latency(State(_state): State<SharedState>, Path(_idx): Path<usize>) -> Json<i32> {
    Json(0)
}
async fn set_latency(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
    Json(_v): Json<i32>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

// Zone
async fn get_zone(State(state): State<SharedState>, Path(idx): Path<usize>) -> impl IntoResponse {
    validate_client(&state, idx).map(|c| Json(c.zone_index))
}
async fn set_zone(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
    Json(_v): Json<usize>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

// Info
async fn get_name(State(state): State<SharedState>, Path(idx): Path<usize>) -> impl IntoResponse {
    validate_client(&state, idx).map(|c| Json(c.name.clone()))
}
async fn set_name(
    State(_state): State<SharedState>,
    Path(_idx): Path<usize>,
    Json(_v): Json<String>,
) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}
async fn get_icon(State(state): State<SharedState>, Path(idx): Path<usize>) -> impl IntoResponse {
    validate_client(&state, idx).map(|c| Json(c.icon.clone()))
}
async fn get_connected(State(_state): State<SharedState>, Path(_idx): Path<usize>) -> Json<bool> {
    Json(false)
}
