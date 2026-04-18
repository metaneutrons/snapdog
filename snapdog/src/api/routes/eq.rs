// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! EQ endpoints: /api/v1/zones/{zone_index}/eq

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};

use crate::api::SharedState;
use crate::api::error::ApiError;
use crate::audio::eq::{self, EqBand, EqConfig};
use crate::player::ZoneCommand;

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/{zone_index}/eq", get(get_eq).put(set_eq))
        .route("/{zone_index}/eq/{band_index}", put(set_band))
        .route("/{zone_index}/eq/preset", post(apply_preset))
        .with_state(state)
}

async fn get_eq(State(state): State<SharedState>, Path(zone): Path<usize>) -> impl IntoResponse {
    if zone == 0 || zone > state.config.zones.len() {
        return Err(ApiError::NotFound("zone"));
    }
    let config = state.eq_store.lock().unwrap().get(zone);
    Ok(Json(config))
}

async fn set_eq(
    State(state): State<SharedState>,
    Path(zone): Path<usize>,
    Json(config): Json<EqConfig>,
) -> impl IntoResponse {
    if zone == 0 || zone > state.config.zones.len() {
        return Err(ApiError::NotFound("zone"));
    }
    if config.bands.len() > 10 {
        return Err(ApiError::BadRequest("Maximum 10 EQ bands".into()));
    }
    let tx = state
        .zone_commands
        .get(&zone)
        .ok_or(ApiError::NotFound("zone"))?;
    let _ = tx.send(ZoneCommand::SetEq(config.clone())).await;
    Ok(Json(config))
}

async fn set_band(
    State(state): State<SharedState>,
    Path((zone, band_idx)): Path<(usize, usize)>,
    Json(band): Json<EqBand>,
) -> impl IntoResponse {
    if zone == 0 || zone > state.config.zones.len() {
        return Err(ApiError::NotFound("zone"));
    }
    let mut config = state.eq_store.lock().unwrap().get(zone);
    if band_idx >= config.bands.len() {
        return Err(ApiError::NotFound("band"));
    }
    config.bands[band_idx] = band;
    config.preset = None;
    let tx = state
        .zone_commands
        .get(&zone)
        .ok_or(ApiError::NotFound("zone"))?;
    let _ = tx.send(ZoneCommand::SetEq(config.clone())).await;
    Ok(Json(config))
}

async fn apply_preset(
    State(state): State<SharedState>,
    Path(zone): Path<usize>,
    Json(name): Json<String>,
) -> impl IntoResponse {
    if zone == 0 || zone > state.config.zones.len() {
        return Err(ApiError::NotFound("zone"));
    }
    let bands = eq::preset(&name).ok_or_else(|| {
        ApiError::BadRequest(format!(
            "Unknown preset '{}'. Available: {:?}",
            name,
            eq::preset_names()
        ))
    })?;
    let config = EqConfig {
        enabled: true,
        bands,
        preset: Some(name),
    };
    let tx = state
        .zone_commands
        .get(&zone)
        .ok_or(ApiError::NotFound("zone"))?;
    let _ = tx.send(ZoneCommand::SetEq(config.clone())).await;
    Ok(Json(config))
}
