// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

mod airplay;
mod api;
mod audio;
mod config;
mod knx;
mod mqtt;
mod process;
mod snapcast;
mod state;
mod subsonic;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("snapdog=info".parse()?))
        .init();

    tracing::info!("SnapDog starting");

    // TODO: Load config, start services
    Ok(())
}
