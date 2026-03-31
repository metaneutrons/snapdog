// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::api::SharedState;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    zones: usize,
    clients: usize,
}

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/health/ready", get(ready))
        .route("/health/live", get(live))
        .with_state(state)
}

async fn health(State(state): State<SharedState>) -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok",
        zones: state.config.zones.len(),
        clients: state.config.clients.len(),
    })
}

async fn ready() -> impl IntoResponse {
    (StatusCode::OK, "ready")
}

async fn live() -> impl IntoResponse {
    (StatusCode::OK, "live")
}
