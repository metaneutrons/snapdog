// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

mod airplay;
mod api;
mod audio;
pub mod config;
mod knx;
mod mqtt;
mod process;
mod snapcast;
mod state;
mod subsonic;

use std::path::PathBuf;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let config_path = std::env::args()
        .nth(2)
        .filter(|_| std::env::args().nth(1).as_deref() == Some("--config"))
        .unwrap_or_else(|| "snapdog.toml".into());

    let config = config::load(&PathBuf::from(&config_path))?;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive(format!("snapdog={}", config.system.log_level).parse()?),
        )
        .init();

    tracing::info!(
        zones = config.zones.len(),
        clients = config.clients.len(),
        radios = config.radios.len(),
        "SnapDog loaded configuration from {config_path}"
    );

    for zone in &config.zones {
        tracing::info!(
            index = zone.index,
            name = %zone.name,
            sink = %zone.sink,
            stream = %zone.stream_name,
            port = zone.tcp_source_port,
            "Zone configured"
        );
    }

    for client in &config.clients {
        tracing::info!(
            index = client.index,
            name = %client.name,
            mac = %client.mac,
            zone = client.zone_index,
            "Client configured"
        );
    }

    Ok(())
}
