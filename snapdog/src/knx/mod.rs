// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! KNX/IP integration via knxkit.
//!
//! Bidirectional:
//! - **Publisher**: writes zone/client status to KNX group addresses on state changes
//! - **Listener**: receives KNX group writes and routes them as zone/client commands

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::str::FromStr;

use anyhow::{Context, Result};
use knxkit::connection::KnxBusConnection;
use knxkit::connection::ops::GroupOps;
use knxkit::core::DataPoint;
use knxkit::core::address::GroupAddress;
use knxkit::net::tunnel::TunnelConnection;

use crate::config::AppConfig;
use crate::player::{ClientAction, SnapcastCmd, ZoneCommand, ZoneCommandSender};
use crate::state;

// ── KNX Bridge ────────────────────────────────────────────────

/// Start the KNX bridge. Spawns a single task that owns the connection
/// and handles both publishing (state → KNX) and listening (KNX → commands).
pub async fn start(
    config: &AppConfig,
    store: state::SharedState,
    notify_rx: tokio::sync::broadcast::Receiver<crate::api::ws::Notification>,
    zone_commands: HashMap<usize, ZoneCommandSender>,
    snap_tx: tokio::sync::mpsc::Sender<SnapcastCmd>,
) -> Result<()> {
    let gw = config
        .knx
        .gateway
        .as_deref()
        .context("KNX requires gateway address")?;
    let addr: std::net::SocketAddrV4 = gw
        .parse()
        .context("Invalid KNX gateway address (expected ip:port)")?;
    let conn = TunnelConnection::start(Ipv4Addr::from_str("0.0.0.0").unwrap(), addr)
        .await
        .context("Failed to connect to KNX gateway")?;
    tracing::info!(gateway = gw, "KNX tunnel connected");

    let config = config.clone();
    tokio::spawn(async move {
        run(conn, config, store, notify_rx, zone_commands, snap_tx).await;
    });

    Ok(())
}

/// Main KNX task: handles both publishing and listening on a single connection.
async fn run(
    mut conn: TunnelConnection,
    config: AppConfig,
    store: state::SharedState,
    mut notify_rx: tokio::sync::broadcast::Receiver<crate::api::ws::Notification>,
    zone_commands: HashMap<usize, ZoneCommandSender>,
    snap_tx: tokio::sync::mpsc::Sender<SnapcastCmd>,
) {
    // Build GA → action lookup for incoming telegrams
    let zone_ga_map = build_zone_ga_map(&config);
    let client_ga_map = build_client_ga_map(&config);

    tracing::info!(
        zone_gas = zone_ga_map.len(),
        client_gas = client_ga_map.len(),
        "KNX bridge active"
    );

    loop {
        tokio::select! {
            // Incoming KNX telegram
            cemi = KnxBusConnection::recv(&mut conn) => {
                let Some(cemi) = cemi else {
                    tracing::warn!("KNX connection closed");
                    break;
                };
                handle_incoming(&cemi, &zone_ga_map, &client_ga_map, &zone_commands, &snap_tx, &store).await;
            }
            // Outgoing state notification → KNX group write
            notification = notify_rx.recv() => {
                match notification {
                    Ok(crate::api::ws::Notification::ZoneStateChanged { zone, .. }) => {
                        publish_zone_state(zone, &config, &store, &mut conn).await;
                    }
                    Ok(crate::api::ws::Notification::ZoneTrackChanged { zone, .. }) => {
                        publish_zone_track(zone, &config, &store, &mut conn).await;
                    }
                    Ok(crate::api::ws::Notification::ClientStateChanged { client, .. }) => {
                        publish_client_state(client, &config, &store, &mut conn).await;
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(missed = n, "KNX publisher lagged");
                    }
                    Err(_) => break,
                }
            }
        }
    }
}

// ── Incoming telegram handling ─────────────────────────────────

fn build_zone_ga_map(config: &AppConfig) -> HashMap<String, (usize, &'static str)> {
    let mut map = HashMap::new();
    for zone_cfg in &config.zones {
        let idx = zone_cfg.index;
        let knx = &zone_cfg.knx;
        let pairs: &[(&Option<String>, &'static str)] = &[
            (&knx.play, "play"),
            (&knx.pause, "pause"),
            (&knx.stop, "stop"),
            (&knx.track_next, "next"),
            (&knx.track_previous, "previous"),
            (&knx.mute, "mute"),
            (&knx.mute_toggle, "mute_toggle"),
            (&knx.shuffle, "shuffle"),
            (&knx.shuffle_toggle, "shuffle_toggle"),
            (&knx.repeat, "repeat"),
            (&knx.repeat_toggle, "repeat_toggle"),
            (&knx.track_repeat, "track_repeat"),
            (&knx.track_repeat_toggle, "track_repeat_toggle"),
        ];
        for (ga_opt, action) in pairs {
            if let Some(ga) = ga_opt {
                map.insert(ga.clone(), (idx, *action));
            }
        }
    }
    map
}

fn build_client_ga_map(config: &AppConfig) -> HashMap<String, (usize, &'static str)> {
    let mut map = HashMap::new();
    for client_cfg in &config.clients {
        let idx = client_cfg.index;
        let knx = &client_cfg.knx;
        let pairs: &[(&Option<String>, &'static str)] =
            &[(&knx.mute, "mute"), (&knx.mute_toggle, "mute_toggle")];
        for (ga_opt, action) in pairs {
            if let Some(ga) = ga_opt {
                map.insert(ga.clone(), (idx, *action));
            }
        }
    }
    map
}

async fn handle_incoming(
    cemi: &knxkit::core::cemi::CEMI,
    zone_ga_map: &HashMap<String, (usize, &str)>,
    client_ga_map: &HashMap<String, (usize, &str)>,
    zone_commands: &HashMap<usize, ZoneCommandSender>,
    snap_tx: &tokio::sync::mpsc::Sender<SnapcastCmd>,
    store: &state::SharedState,
) {
    let ga = match &cemi.destination {
        knxkit::core::address::DestinationAddress::Group(ga) => format!("{ga:?}"),
        _ => return,
    };

    // Extract APDU data from TPDU
    use knxkit::core::tpdu::TPDU;
    let data = match &cemi.npdu.tpdu {
        TPDU::DataGroup(apdu) | TPDU::DataBroadcast(apdu) => apdu.data.clone(),
        _ => return,
    };

    if let Some(&(zone_idx, action)) = zone_ga_map.get(&ga) {
        if let Some(tx) = zone_commands.get(&zone_idx) {
            let cmd = match action {
                "play" => Some(ZoneCommand::Play),
                "pause" => Some(ZoneCommand::Pause),
                "stop" => Some(ZoneCommand::Stop),
                "next" => Some(ZoneCommand::Next),
                "previous" => Some(ZoneCommand::Previous),
                "mute_toggle" => Some(ZoneCommand::ToggleMute),
                "shuffle_toggle" => Some(ZoneCommand::ToggleShuffle),
                "repeat_toggle" => Some(ZoneCommand::ToggleRepeat),
                "track_repeat_toggle" => Some(ZoneCommand::ToggleTrackRepeat),
                "mute" => data.as_ref().map(|d| ZoneCommand::SetMute(decode_bool(d))),
                "shuffle" => data
                    .as_ref()
                    .map(|d| ZoneCommand::SetShuffle(decode_bool(d))),
                "repeat" => data
                    .as_ref()
                    .map(|d| ZoneCommand::SetRepeat(decode_bool(d))),
                "track_repeat" => data
                    .as_ref()
                    .map(|d| ZoneCommand::SetTrackRepeat(decode_bool(d))),
                _ => None,
            };
            if let Some(cmd) = cmd {
                tracing::debug!(zone = zone_idx, ga = %ga, "KNX → zone command");
                let _ = tx.send(cmd).await;
            }
        }
    }

    if let Some(&(client_idx, action)) = client_ga_map.get(&ga) {
        let s = store.read().await;
        if let Some(client) = s.clients.get(&client_idx) {
            if let Some(ref snap_id) = client.snapcast_id {
                let cmd = match action {
                    "mute_toggle" => Some(ClientAction::Mute(!client.muted)),
                    "mute" => data.as_ref().map(|d| ClientAction::Mute(decode_bool(d))),
                    _ => None,
                };
                if let Some(action) = cmd {
                    let snap_id = snap_id.clone();
                    drop(s);
                    tracing::debug!(client = client_idx, ga = %ga, "KNX → client command");
                    let _ = snap_tx
                        .send(SnapcastCmd::Client {
                            client_id: snap_id,
                            action,
                        })
                        .await;
                }
            }
        }
    }
}

// ── Publishing ────────────────────────────────────────────────

async fn publish_zone_state(
    zone_index: usize,
    config: &AppConfig,
    store: &state::SharedState,
    conn: &mut TunnelConnection,
) {
    let s = store.read().await;
    let Some(zone) = s.zones.get(&zone_index) else {
        return;
    };
    let Some(zone_cfg) = config.zones.get(zone_index - 1) else {
        return;
    };
    let knx = &zone_cfg.knx;

    if let Some(ref ga) = knx.volume_status {
        write(conn, ga, encode_percent(zone.volume.clamp(0, 100) as u8)).await;
    }
    if let Some(ref ga) = knx.mute_status {
        write(conn, ga, encode_bool(zone.muted)).await;
    }
    if let Some(ref ga) = knx.shuffle_status {
        write(conn, ga, encode_bool(zone.shuffle)).await;
    }
    if let Some(ref ga) = knx.repeat_status {
        write(conn, ga, encode_bool(zone.repeat)).await;
    }
    if let Some(ref ga) = knx.track_playing_status {
        write(
            conn,
            ga,
            encode_bool(zone.playback.to_string() == "playing"),
        )
        .await;
    }
    if let Some(ref ga) = knx.track_repeat_status {
        write(conn, ga, encode_bool(zone.track_repeat)).await;
    }
}

async fn publish_zone_track(
    zone_index: usize,
    config: &AppConfig,
    store: &state::SharedState,
    conn: &mut TunnelConnection,
) {
    let s = store.read().await;
    let Some(zone) = s.zones.get(&zone_index) else {
        return;
    };
    let Some(zone_cfg) = config.zones.get(zone_index - 1) else {
        return;
    };
    let knx = &zone_cfg.knx;

    if let Some(ref track) = zone.track {
        if let Some(ref ga) = knx.track_title_status {
            write(conn, ga, encode_string(&track.title)).await;
        }
        if let Some(ref ga) = knx.track_artist_status {
            write(conn, ga, encode_string(&track.artist)).await;
        }
        if let Some(ref ga) = knx.track_album_status {
            write(conn, ga, encode_string(&track.album)).await;
        }
    }
}

async fn publish_client_state(
    client_index: usize,
    config: &AppConfig,
    store: &state::SharedState,
    conn: &mut TunnelConnection,
) {
    let s = store.read().await;
    let Some(client) = s.clients.get(&client_index) else {
        return;
    };
    let Some(client_cfg) = config.clients.get(client_index - 1) else {
        return;
    };
    let knx = &client_cfg.knx;

    if let Some(ref ga) = knx.volume_status {
        write(conn, ga, encode_percent(client.volume.clamp(0, 100) as u8)).await;
    }
    if let Some(ref ga) = knx.mute_status {
        write(conn, ga, encode_bool(client.muted)).await;
    }
    if let Some(ref ga) = knx.connected_status {
        write(conn, ga, encode_bool(client.connected)).await;
    }
    if let Some(ref ga) = knx.zone_status {
        write(conn, ga, encode_percent(client.zone_index as u8)).await;
    }
}

async fn write(conn: &mut TunnelConnection, ga_str: &str, dp: DataPoint) {
    let ga = match GroupAddress::from_str(ga_str) {
        Ok(ga) => ga,
        Err(e) => {
            tracing::warn!(ga = ga_str, error = %e, "Invalid KNX GA");
            return;
        }
    };
    if let Err(e) = conn.group_write(ga, dp).await {
        tracing::warn!(ga = ga_str, error = %e, "KNX write failed");
    }
}

// ── Helpers ───────────────────────────────────────────────────

fn decode_bool(dp: &DataPoint) -> bool {
    match dp {
        DataPoint::Short(v) => *v != 0,
        DataPoint::Long(v) => v.first().is_some_and(|b| *b != 0),
    }
}

pub fn encode_bool(value: bool) -> DataPoint {
    DataPoint::Short(u8::from(value))
}

pub fn encode_percent(percent: u8) -> DataPoint {
    DataPoint::Short(((percent as u16) * 255 / 100) as u8)
}

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
    }

    #[test]
    fn encodes_string_truncates_to_14() {
        let dp = encode_string("Hello, World!!");
        if let DataPoint::Long(bytes) = dp {
            assert_eq!(bytes.len(), 14);
        } else {
            panic!("Expected Long");
        }
    }

    #[test]
    fn decodes_bool() {
        assert!(decode_bool(&DataPoint::Short(1)));
        assert!(!decode_bool(&DataPoint::Short(0)));
    }
}
