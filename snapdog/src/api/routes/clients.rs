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
use crate::api::routes::zones::VolumeValue;
use crate::state;

#[derive(Serialize)]
struct ClientInfo {
    index: usize,
    name: String,
    mac: String,
    zone_index: usize,
    icon: String,
    volume: i32,
    muted: bool,
    connected: bool,
}

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/count", get(get_count))
        .route("/", get(get_all))
        .route("/{client_index}", get(get_client))
        .route("/{client_index}/volume", get(get_volume).put(set_volume))
        .route("/{client_index}/mute", get(get_mute).put(set_mute))
        .route("/{client_index}/mute/toggle", post(toggle_mute))
        .route("/{client_index}/latency", get(get_latency).put(set_latency))
        .route("/{client_index}/zone", get(get_zone).put(set_zone))
        .route("/{client_index}/name", get(get_name).put(set_name))
        .route("/{client_index}/icon", get(get_icon))
        .route("/{client_index}/connected", get(get_connected))
        .with_state(state)
}

async fn read_client(state: &SharedState, idx: usize) -> Option<state::ClientState> {
    state.store.read().await.clients.get(&idx).cloned()
}

fn not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

async fn get_count(State(state): State<SharedState>) -> Json<usize> {
    Json(state.config.clients.len())
}

async fn get_all(State(state): State<SharedState>) -> Json<Vec<ClientInfo>> {
    let store = state.store.read().await;
    Json(
        state
            .config
            .clients
            .iter()
            .map(|c| {
                let cs = store.clients.get(&c.index);
                ClientInfo {
                    index: c.index,
                    name: c.name.clone(),
                    mac: c.mac.clone(),
                    zone_index: cs.map_or(c.zone_index, |s| s.zone_index),
                    icon: c.icon.clone(),
                    volume: cs.map_or(50, |s| s.volume),
                    muted: cs.is_some_and(|s| s.muted),
                    connected: cs.is_some_and(|s| s.connected),
                }
            })
            .collect(),
    )
}

async fn get_client(State(state): State<SharedState>, Path(idx): Path<usize>) -> impl IntoResponse {
    let store = state.store.read().await;
    let cfg = state.config.clients.get(idx - 1).ok_or(not_found())?;
    let cs = store.clients.get(&idx);
    Ok::<_, StatusCode>(Json(ClientInfo {
        index: cfg.index,
        name: cfg.name.clone(),
        mac: cfg.mac.clone(),
        zone_index: cs.map_or(cfg.zone_index, |s| s.zone_index),
        icon: cfg.icon.clone(),
        volume: cs.map_or(50, |s| s.volume),
        muted: cs.is_some_and(|s| s.muted),
        connected: cs.is_some_and(|s| s.connected),
    }))
}

async fn get_volume(State(state): State<SharedState>, Path(idx): Path<usize>) -> impl IntoResponse {
    read_client(&state, idx)
        .await
        .map(|c| Json(c.volume))
        .ok_or(not_found())
}

async fn set_volume(
    State(state): State<SharedState>,
    Path(idx): Path<usize>,
    Json(value): Json<VolumeValue>,
) -> impl IntoResponse {
    let mut store = state.store.write().await;
    let client = store.clients.get_mut(&idx).ok_or(not_found())?;
    let volume = value
        .resolve(client.volume)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    client.volume = volume;
    tracing::info!(client = idx, volume, "Client volume set");
    Ok::<_, StatusCode>(Json(volume))
}

async fn get_mute(State(state): State<SharedState>, Path(idx): Path<usize>) -> impl IntoResponse {
    read_client(&state, idx)
        .await
        .map(|c| Json(c.muted))
        .ok_or(not_found())
}

async fn set_mute(
    State(state): State<SharedState>,
    Path(idx): Path<usize>,
    Json(v): Json<bool>,
) -> impl IntoResponse {
    crate::state::update_client_and_notify(&state.store, idx, &state.notifications, |c| {
        c.muted = v
    })
    .await;
    Ok::<_, StatusCode>(StatusCode::NO_CONTENT)
}

async fn toggle_mute(
    State(state): State<SharedState>,
    Path(idx): Path<usize>,
) -> impl IntoResponse {
    let muted = read_client(&state, idx).await.is_some_and(|c| !c.muted);
    crate::state::update_client_and_notify(&state.store, idx, &state.notifications, |c| {
        c.muted = muted
    })
    .await;
    Ok::<_, StatusCode>(Json(muted))
}

async fn get_latency(
    State(state): State<SharedState>,
    Path(idx): Path<usize>,
) -> impl IntoResponse {
    read_client(&state, idx)
        .await
        .map(|c| Json(c.latency_ms))
        .ok_or(not_found())
}

async fn set_latency(
    State(state): State<SharedState>,
    Path(idx): Path<usize>,
    Json(v): Json<i32>,
) -> impl IntoResponse {
    crate::state::update_client_and_notify(&state.store, idx, &state.notifications, |c| {
        c.latency_ms = v
    })
    .await;
    Ok::<_, StatusCode>(StatusCode::NO_CONTENT)
}

async fn get_zone(State(state): State<SharedState>, Path(idx): Path<usize>) -> impl IntoResponse {
    read_client(&state, idx)
        .await
        .map(|c| Json(c.zone_index))
        .ok_or(not_found())
}

async fn set_zone(
    State(state): State<SharedState>,
    Path(idx): Path<usize>,
    Json(v): Json<usize>,
) -> impl IntoResponse {
    crate::state::update_client_and_notify(&state.store, idx, &state.notifications, |c| {
        c.zone_index = v
    })
    .await;
    tracing::info!(client = idx, zone = v, "Client zone changed");
    Ok::<_, StatusCode>(StatusCode::NO_CONTENT)
}

async fn get_name(State(state): State<SharedState>, Path(idx): Path<usize>) -> impl IntoResponse {
    read_client(&state, idx)
        .await
        .map(|c| Json(c.name))
        .ok_or(not_found())
}

async fn set_name(
    State(state): State<SharedState>,
    Path(idx): Path<usize>,
    Json(v): Json<String>,
) -> impl IntoResponse {
    crate::state::update_client_and_notify(&state.store, idx, &state.notifications, |c| c.name = v)
        .await;
    Ok::<_, StatusCode>(StatusCode::NO_CONTENT)
}

async fn get_icon(State(state): State<SharedState>, Path(idx): Path<usize>) -> impl IntoResponse {
    read_client(&state, idx)
        .await
        .map(|c| Json(c.icon))
        .ok_or(not_found())
}

async fn get_connected(
    State(state): State<SharedState>,
    Path(idx): Path<usize>,
) -> impl IntoResponse {
    read_client(&state, idx)
        .await
        .map(|c| Json(c.connected))
        .ok_or(not_found())
}
