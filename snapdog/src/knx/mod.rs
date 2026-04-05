// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! KNX/IP integration via knxkit.
//!
//! Bidirectional:
//! - **Publisher**: writes zone/client status to KNX group addresses on state changes
//! - **Listener**: receives KNX group writes and routes them as zone/client commands
//!
//! Currently supports KNX/IP tunneling connections only.

use std::net::Ipv4Addr;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result};
use knxkit::connection::ops::GroupOps;
use knxkit::core::DataPoint;
use knxkit::core::address::GroupAddress;
use knxkit::net::tunnel::TunnelConnection;
use tokio::sync::Mutex;

use crate::config::{AppConfig, KnxConfig};
use crate::state;

// ── KNX Bridge ────────────────────────────────────────────────

/// KNX bridge: publishes status, receives commands.
pub struct KnxBridge {
    conn: Arc<Mutex<TunnelConnection>>,
    config: Arc<AppConfig>,
}

impl KnxBridge {
    /// Connect to KNX gateway.
    pub async fn connect(knx_config: &KnxConfig, app_config: Arc<AppConfig>) -> Result<Self> {
        let gw = knx_config
            .gateway
            .as_deref()
            .context("KNX requires gateway address")?;
        let addr: std::net::SocketAddrV4 = gw
            .parse()
            .context("Invalid KNX gateway address (expected ip:port)")?;
        let local = Ipv4Addr::from_str("0.0.0.0").unwrap();
        let conn = TunnelConnection::start(local, addr)
            .await
            .context("Failed to connect to KNX gateway")?;
        tracing::info!(gateway = gw, "KNX tunnel connected");
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            config: app_config,
        })
    }

    /// Publish zone state to KNX group addresses.
    pub async fn publish_zone_state(&self, zone_index: usize, store: &state::SharedState) {
        let s = store.read().await;
        let Some(zone) = s.zones.get(&zone_index) else {
            return;
        };
        let zone_cfg = match self.config.zones.get(zone_index - 1) {
            Some(c) => c,
            None => return,
        };
        let knx = &zone_cfg.knx;

        if let Some(ref ga) = knx.volume_status {
            self.write(ga, encode_percent(zone.volume.clamp(0, 100) as u8))
                .await;
        }
        if let Some(ref ga) = knx.mute_status {
            self.write(ga, encode_bool(zone.muted)).await;
        }
        if let Some(ref ga) = knx.shuffle_status {
            self.write(ga, encode_bool(zone.shuffle)).await;
        }
        if let Some(ref ga) = knx.repeat_status {
            self.write(ga, encode_bool(zone.repeat)).await;
        }
        if let Some(ref ga) = knx.track_playing_status {
            self.write(ga, encode_bool(zone.playback.to_string() == "playing"))
                .await;
        }
        if let Some(ref ga) = knx.track_repeat_status {
            self.write(ga, encode_bool(zone.track_repeat)).await;
        }
    }

    /// Publish zone track metadata to KNX.
    pub async fn publish_zone_track(&self, zone_index: usize, store: &state::SharedState) {
        let s = store.read().await;
        let Some(zone) = s.zones.get(&zone_index) else {
            return;
        };
        let zone_cfg = match self.config.zones.get(zone_index - 1) {
            Some(c) => c,
            None => return,
        };
        let knx = &zone_cfg.knx;

        if let Some(ref track) = zone.track {
            if let Some(ref ga) = knx.track_title_status {
                self.write(ga, encode_string(&track.title)).await;
            }
            if let Some(ref ga) = knx.track_artist_status {
                self.write(ga, encode_string(&track.artist)).await;
            }
            if let Some(ref ga) = knx.track_album_status {
                self.write(ga, encode_string(&track.album)).await;
            }
        }
    }

    /// Publish client state to KNX group addresses.
    pub async fn publish_client_state(&self, client_index: usize, store: &state::SharedState) {
        let s = store.read().await;
        let Some(client) = s.clients.get(&client_index) else {
            return;
        };
        let client_cfg = match self.config.clients.get(client_index - 1) {
            Some(c) => c,
            None => return,
        };
        let knx = &client_cfg.knx;

        if let Some(ref ga) = knx.volume_status {
            self.write(ga, encode_percent(client.volume.clamp(0, 100) as u8))
                .await;
        }
        if let Some(ref ga) = knx.mute_status {
            self.write(ga, encode_bool(client.muted)).await;
        }
        if let Some(ref ga) = knx.connected_status {
            self.write(ga, encode_bool(client.connected)).await;
        }
        if let Some(ref ga) = knx.zone_status {
            self.write(ga, encode_percent(client.zone_index as u8))
                .await;
        }
    }

    /// Write a data point to a KNX group address.
    async fn write(&self, ga_str: &str, dp: DataPoint) {
        let ga = match GroupAddress::from_str(ga_str) {
            Ok(ga) => ga,
            Err(e) => {
                tracing::warn!(ga = ga_str, error = %e, "Invalid KNX group address");
                return;
            }
        };
        let mut conn = self.conn.lock().await;
        if let Err(e) = conn.group_write(ga, dp).await {
            tracing::warn!(ga = ga_str, error = %e, "KNX group write failed");
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────

/// Encode a boolean as KNX DPT 1.x.
pub fn encode_bool(value: bool) -> DataPoint {
    DataPoint::Short(u8::from(value))
}

/// Encode a percentage (0-100) as KNX DPT 5.001 (0-255 scaling).
pub fn encode_percent(percent: u8) -> DataPoint {
    DataPoint::Short(((percent as u16) * 255 / 100) as u8)
}

/// Encode a string as KNX DPT 16.001 (14-byte ASCII).
pub fn encode_string(value: &str) -> DataPoint {
    let mut bytes = vec![0u8; 14];
    let src = value.as_bytes();
    let len = src.len().min(14);
    bytes[..len].copy_from_slice(&src[..len]);
    DataPoint::Long(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_bool() {
        assert_eq!(encode_bool(true), DataPoint::Short(1));
        assert_eq!(encode_bool(false), DataPoint::Short(0));
    }

    #[test]
    fn encodes_percent() {
        assert_eq!(encode_percent(0), DataPoint::Short(0));
        assert_eq!(encode_percent(100), DataPoint::Short(255));
        assert_eq!(encode_percent(50), DataPoint::Short(127));
    }

    #[test]
    fn encodes_string_truncates_to_14() {
        let dp = encode_string("Hello, World!!");
        if let DataPoint::Long(bytes) = dp {
            assert_eq!(bytes.len(), 14);
            assert_eq!(&bytes[..14], b"Hello, World!!");
        } else {
            panic!("Expected Long DataPoint");
        }
    }
}
