// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Child process management for snapserver.
//!
//! Generates snapserver.conf from app config, spawns and monitors the process.
//! In dev mode (managed=false), this is a no-op.

use std::path::PathBuf;

use anyhow::{Context, Result};
use tokio::process::{Child, Command};

use crate::config::AppConfig;

/// Handle to a managed snapserver process. Kills the process on drop.
pub struct SnapserverHandle {
    child: Option<Child>,
    config_path: PathBuf,
}

impl SnapserverHandle {
    /// Start snapserver if managed=true, otherwise return a no-op handle.
    pub async fn start(config: &AppConfig) -> Result<Self> {
        if !config.snapcast.managed {
            tracing::info!("Snapserver not managed (managed=false) — skipping");
            return Ok(Self {
                child: None,
                config_path: PathBuf::new(),
            });
        }

        let config_path = generate_config(config)?;
        tracing::info!(path = %config_path.display(), "Generated snapserver.conf");

        let child = Command::new("snapserver")
            .arg("-c")
            .arg(&config_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .context("Failed to start snapserver — is it installed?")?;

        tracing::info!(pid = child.id().unwrap_or(0), "Snapserver started");

        Ok(Self {
            child: Some(child),
            config_path,
        })
    }

    /// Gracefully stop the snapserver process.
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(ref mut child) = self.child {
            tracing::info!("Stopping snapserver");
            child.kill().await.context("Failed to kill snapserver")?;
            self.child = None;
        }
        // Clean up generated config
        if self.config_path.exists() {
            let _ = std::fs::remove_file(&self.config_path);
        }
        Ok(())
    }
}

impl Drop for SnapserverHandle {
    fn drop(&mut self) {
        if self.config_path.exists() {
            let _ = std::fs::remove_file(&self.config_path);
        }
    }
}

/// Generate snapserver.conf from app config. Returns path to the generated file.
fn generate_config(config: &AppConfig) -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!(
        "snapdog-snapserver-{}-{}.conf",
        std::process::id(),
        config.snapcast.streaming_port
    ));
    let content = render_config(config);
    std::fs::write(&path, &content)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(path)
}

fn render_config(config: &AppConfig) -> String {
    let mut out = String::new();

    // HTTP / JSON-RPC (WebSocket)
    out.push_str(&format!(
        "[http]\nenabled = true\nbind_to_address = 127.0.0.1\nport = {}\n\n",
        config.snapcast.jsonrpc_port
    ));

    // TCP control (JSON-RPC raw TCP — used by snapcast-control crate)
    let tcp_control_port = config.snapcast.streaming_port + 1;
    out.push_str(&format!(
        "[tcp-control]\nenabled = true\nbind_to_address = 127.0.0.1\nport = {tcp_control_port}\n\n",
    ));
    // Streaming server
    out.push_str(&format!(
        "[tcp-streaming]\nbind_to_address = 0.0.0.0\nport = {}\n\n",
        config.snapcast.streaming_port
    ));

    // TCP sources — one per zone
    out.push_str("[stream]\n");
    for zone in &config.zones {
        let sf = format!(
            "{}:{}:{}",
            config.audio.sample_rate, config.audio.bit_depth, config.audio.channels
        );
        out.push_str(&format!(
            "source = tcp://127.0.0.1:{}?name={}&sampleformat={}&mode=server\n",
            zone.tcp_source_port, zone.stream_name, sf
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;

    fn test_config() -> AppConfig {
        let raw: config::RawConfig = toml::from_str(
            r#"
            [[zone]]
            name = "Ground Floor"
            [[zone]]
            name = "1st Floor"
            [[client]]
            name = "X"
            mac = "00:00:00:00:00:00"
            zone = "Ground Floor"
        "#,
        )
        .unwrap();
        config::load_raw(raw).unwrap()
    }

    #[test]
    fn generates_correct_sources() {
        let conf = render_config(&test_config());
        assert!(conf.contains(
            "source = tcp://127.0.0.1:4953?name=Zone1&sampleformat=48000:16:2&mode=server"
        ));
        assert!(conf.contains(
            "source = tcp://127.0.0.1:4954?name=Zone2&sampleformat=48000:16:2&mode=server"
        ));
    }

    #[test]
    fn binds_http_to_loopback() {
        let conf = render_config(&test_config());
        assert!(conf.contains("bind_to_address = 127.0.0.1"));
    }

    #[test]
    fn server_binds_to_all_interfaces() {
        let conf = render_config(&test_config());
        assert!(conf.contains("bind_to_address = 0.0.0.0"));
    }
}
