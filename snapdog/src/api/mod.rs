// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! REST API and WebSocket server via axum.
//!
//! - REST endpoints for zones, clients, media, system
//! - WebSocket for real-time state notifications
//! - Health endpoint

mod health;
mod routes;

use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use tokio::net::TcpListener;

use crate::config::AppConfig;

/// Shared application state accessible from all handlers.
pub struct AppState {
    pub config: AppConfig,
    // pub snapcast: tokio::sync::Mutex<Snapcast>,
    // TODO: add state, snapcast, etc. as needed
}

pub type SharedState = Arc<AppState>;

/// Start the HTTP server.
pub async fn serve(config: AppConfig) -> Result<()> {
    let port = config.http.port;
    let state = Arc::new(AppState { config });

    let app = Router::new()
        .merge(health::router())
        .nest("/api/v1/zones", routes::zones::router(state.clone()))
        .nest("/api/v1/clients", routes::clients::router(state.clone()))
        .nest("/api/v1/media", routes::media::router(state.clone()))
        .nest("/api/v1/system", routes::system::router(state.clone()));

    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!(port, "API server listening");
    axum::serve(listener, app).await?;
    Ok(())
}
