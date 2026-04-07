// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! REST API and WebSocket server via axum.

mod auth;
pub mod error;
mod health;
mod routes;
mod webui;
pub mod ws;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use tokio::net::TcpListener;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::config::AppConfig;
use crate::player;
use crate::player::ZoneCommandSender;
use crate::state;

/// Shared application state accessible from all handlers.
pub struct AppState {
    pub config: AppConfig,
    pub store: state::SharedState,
    pub zone_commands: HashMap<usize, ZoneCommandSender>,
    pub snap_tx: player::SnapcastCmdSender,
    pub covers: state::cover::SharedCoverCache,
    pub notifications: tokio::sync::broadcast::Sender<ws::Notification>,
    pub playlist_cache:
        tokio::sync::RwLock<Option<(std::time::Instant, Vec<crate::subsonic::PlaylistEntry>)>>,
}

pub type SharedState = Arc<AppState>;

/// Start the HTTP server.
pub async fn serve(
    config: AppConfig,
    store: state::SharedState,
    zone_commands: HashMap<usize, ZoneCommandSender>,
    snap_tx: player::SnapcastCmdSender,
    covers: state::cover::SharedCoverCache,
    notifications: tokio::sync::broadcast::Sender<ws::Notification>,
) -> Result<()> {
    let port = config.http.port;
    let state = Arc::new(AppState {
        config,
        store,
        zone_commands,
        snap_tx,
        covers,
        notifications,
        playlist_cache: tokio::sync::RwLock::new(None),
    });

    let api_key = state.config.http.api_key.clone();

    // Protected routes (API + WebSocket)
    let mut protected = Router::new()
        .merge(ws::router(state.clone()))
        .nest("/api/v1/zones", routes::zones::router(state.clone()))
        .nest("/api/v1/clients", routes::clients::router(state.clone()))
        .nest("/api/v1/media", routes::media::router(state.clone()))
        .nest("/api/v1/system", routes::system::router(state.clone()));

    if let Some(key) = api_key {
        tracing::info!("API key authentication enabled");
        protected = protected
            .layer(axum::Extension(auth::ApiKey(key)))
            .layer(axum::middleware::from_fn(auth::require_api_key));
    }

    let app = Router::new()
        .merge(health::router(state.clone()))
        .merge(protected)
        .fallback(webui::fallback)
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    tracing::info!(port, "Listening");
    axum::serve(listener, app).await?;
    Ok(())
}
