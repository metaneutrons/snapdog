// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! KNX device management routes.

use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};

use crate::api::SharedState;
use crate::api::error::ApiError;
use crate::knx::KnxDeviceControl as _;

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route(
            "/programming-mode",
            get(get_programming_mode).put(set_programming_mode),
        )
        .with_state(state)
}

async fn get_programming_mode(State(state): State<SharedState>) -> impl IntoResponse {
    let Some(ref ctl) = state.knx_device_control else {
        return Err(ApiError::NotFound("KNX device mode not active"));
    };
    Ok(Json(ctl.get_prog_mode().await))
}

async fn set_programming_mode(
    State(state): State<SharedState>,
    Json(enabled): Json<bool>,
) -> impl IntoResponse {
    let Some(ref ctl) = state.knx_device_control else {
        return Err(ApiError::NotFound("KNX device mode not active"));
    };
    ctl.set_prog_mode(enabled).await;
    Ok(Json(enabled))
}
