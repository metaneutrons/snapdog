// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/health/ready", get(ready))
        .route("/health/live", get(live))
}

async fn health() -> impl IntoResponse {
    Json(HealthResponse { status: "ok" })
}

async fn ready() -> impl IntoResponse {
    // TODO: check snapcast connection, etc.
    (StatusCode::OK, "ready")
}

async fn live() -> impl IntoResponse {
    (StatusCode::OK, "live")
}
