// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! KNX/IP integration via knxkit.
//!
//! Bidirectional:
//! - **Publisher**: writes zone/client status to KNX group addresses on state changes
//! - **Listener**: receives KNX group writes and routes them as zone/client commands
//!
//! Uses knxkit's [`Multiplexer`] to fan out a single connection into
//! independent publisher and listener handles. Supports both tunnel
//! (unicast) and router (multicast) connections via URL auto-detection.

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::str::FromStr;

use anyhow::{Context, Result};
use knxkit::connection::KnxBusConnection;
use knxkit::connection::multiplex::{MultiplexHandle, Multiplexer};
use knxkit::connection::ops::GroupOps;
use knxkit::connection::{RemoteSpec, parse_remote};
use knxkit::core::DataPoint;
use knxkit::core::address::GroupAddress;
use knxkit_dpt::specific::{DPT_1_1, DPT_5_1, DPT_16_1, SpecificDataPoint};

use crate::config::AppConfig;
use crate::player::{ClientAction, SnapcastCmd, ZoneCommand, ZoneCommandSender};
use crate::state;

// ── Start ─────────────────────────────────────────────────────

/// Start the KNX bridge. Parses the URL to auto-detect tunnel vs router,
/// creates a multiplexer, and spawns independent publisher and listener tasks.
pub async fn start(
    config: &AppConfig,
    store: state::SharedState,
    notify_rx: tokio::sync::broadcast::Receiver<crate::api::ws::Notification>,
    zone_commands: HashMap<usize, ZoneCommandSender>,
    snap_tx: tokio::sync::mpsc::Sender<SnapcastCmd>,
) -> Result<()> {
    let url = config
        .knx
        .url
        .as_deref()
        .context("KNX requires url (e.g. udp://192.168.1.50:3671)")?;
    let remote = parse_remote(url).context("Invalid KNX URL")?;
    let local = Ipv4Addr::UNSPECIFIED;

    match remote {
        RemoteSpec::KnxIpTunnel(addr) => {
            let conn = knxkit::net::tunnel::TunnelConnection::start(local, addr)
                .await
                .context("KNX tunnel connection failed")?;
            tracing::info!(%url, "KNX tunnel connected");
            spawn_bridge(
                Multiplexer::new(conn),
                config,
                store,
                notify_rx,
                zone_commands,
                snap_tx,
            );
        }
        RemoteSpec::KnxIpMulticast(addr) => {
            let conn = knxkit::net::router::RouterConnection::start(local, addr)
                .await
                .context("KNX router connection failed")?;
            tracing::info!(%url, "KNX router connected");
            spawn_bridge(
                Multiplexer::new(conn),
                config,
                store,
                notify_rx,
                zone_commands,
                snap_tx,
            );
        }
        _ => anyhow::bail!("Unsupported KNX connection type — use udp:// URL"),
    }

    Ok(())
}

fn spawn_bridge<T: KnxBusConnection + Send + 'static>(
    mux: Multiplexer<T>,
    config: &AppConfig,
    store: state::SharedState,
    notify_rx: tokio::sync::broadcast::Receiver<crate::api::ws::Notification>,
    zone_commands: HashMap<usize, ZoneCommandSender>,
    snap_tx: tokio::sync::mpsc::Sender<SnapcastCmd>,
) {
    let pub_handle = mux.handle();
    let listen_handle = mux.handle();
    tokio::spawn(mux.run());

    let pub_config = config.clone();
    let pub_store = store.clone();
    tokio::spawn(async move {
        publisher(pub_handle, pub_config, pub_store, notify_rx).await;
    });

    let listen_config = config.clone();
    tokio::spawn(async move {
        listener(listen_handle, listen_config, store, zone_commands, snap_tx).await;
    });
}

// ── Publisher ─────────────────────────────────────────────────

async fn publisher(
    mut handle: MultiplexHandle,
    config: AppConfig,
    store: state::SharedState,
    mut notify_rx: tokio::sync::broadcast::Receiver<crate::api::ws::Notification>,
) {
    tracing::info!("KNX publisher started");
    loop {
        match notify_rx.recv().await {
            Ok(crate::api::ws::Notification::ZoneStateChanged { zone, .. }) => {
                publish_zone_state(zone, &config, &store, &mut handle).await;
            }
            Ok(crate::api::ws::Notification::ZoneTrackChanged { zone, .. }) => {
                publish_zone_track(zone, &config, &store, &mut handle).await;
            }
            Ok(crate::api::ws::Notification::ZoneProgress { zone, .. }) => {
                publish_zone_progress(zone, &config, &store, &mut handle).await;
            }
            Ok(crate::api::ws::Notification::ClientStateChanged { client, .. }) => {
                publish_client_state(client, &config, &store, &mut handle).await;
            }
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(missed = n, "KNX publisher lagged");
            }
            Err(_) => break,
        }
    }
}

async fn publish_zone_state(
    zone_index: usize,
    config: &AppConfig,
    store: &state::SharedState,
    handle: &mut MultiplexHandle,
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
        write(handle, ga, encode_percent(zone.volume.clamp(0, 100) as u8)).await;
    }
    if let Some(ref ga) = knx.mute_status {
        write(handle, ga, encode_bool(zone.muted)).await;
    }
    if let Some(ref ga) = knx.shuffle_status {
        write(handle, ga, encode_bool(zone.shuffle)).await;
    }
    if let Some(ref ga) = knx.repeat_status {
        write(handle, ga, encode_bool(zone.repeat)).await;
    }
    if let Some(ref ga) = knx.track_playing_status {
        write(
            handle,
            ga,
            encode_bool(zone.playback.to_string() == "playing"),
        )
        .await;
    }
    if let Some(ref ga) = knx.track_repeat_status {
        write(handle, ga, encode_bool(zone.track_repeat)).await;
    }
    if let Some(ref ga) = knx.control_status {
        write(
            handle,
            ga,
            encode_bool(zone.playback.to_string() == "playing"),
        )
        .await;
    }
    if let Some(ref ga) = knx.playlist_status {
        write(
            handle,
            ga,
            encode_percent(zone.playlist_index.unwrap_or(0) as u8),
        )
        .await;
    }
}

async fn publish_zone_track(
    zone_index: usize,
    config: &AppConfig,
    store: &state::SharedState,
    handle: &mut MultiplexHandle,
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
            write(handle, ga, encode_string(&track.title)).await;
        }
        if let Some(ref ga) = knx.track_artist_status {
            write(handle, ga, encode_string(&track.artist)).await;
        }
        if let Some(ref ga) = knx.track_album_status {
            write(handle, ga, encode_string(&track.album)).await;
        }
        if let Some(ref ga) = knx.track_progress_status {
            let pct = if track.duration_ms > 0 {
                ((track.position_ms as f64 / track.duration_ms as f64) * 100.0).clamp(0.0, 100.0)
                    as u8
            } else {
                0
            };
            write(handle, ga, encode_percent(pct)).await;
        }
    }
}

async fn publish_zone_progress(
    zone_index: usize,
    config: &AppConfig,
    store: &state::SharedState,
    handle: &mut MultiplexHandle,
) {
    let s = store.read().await;
    let Some(zone) = s.zones.get(&zone_index) else {
        return;
    };
    let Some(zone_cfg) = config.zones.get(zone_index - 1) else {
        return;
    };
    if let Some(ref ga) = zone_cfg.knx.track_progress_status {
        let pct = zone.track.as_ref().map_or(0u8, |t| {
            if t.duration_ms > 0 {
                ((t.position_ms as f64 / t.duration_ms as f64) * 100.0).clamp(0.0, 100.0) as u8
            } else {
                0
            }
        });
        write(handle, ga, encode_percent(pct)).await;
    }
}

async fn publish_client_state(
    client_index: usize,
    config: &AppConfig,
    store: &state::SharedState,
    handle: &mut MultiplexHandle,
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
        write(
            handle,
            ga,
            encode_percent(client.base_volume.clamp(0, 100) as u8),
        )
        .await;
    }
    if let Some(ref ga) = knx.mute_status {
        write(handle, ga, encode_bool(client.muted)).await;
    }
    if let Some(ref ga) = knx.connected_status {
        write(handle, ga, encode_bool(client.connected)).await;
    }
    if let Some(ref ga) = knx.zone_status {
        write(handle, ga, encode_percent(client.zone_index as u8)).await;
    }
    if let Some(ref ga) = knx.latency_status {
        write(
            handle,
            ga,
            encode_percent(client.latency_ms.clamp(0, 255) as u8),
        )
        .await;
    }
}

// ── Listener ──────────────────────────────────────────────────

async fn listener(
    mut handle: MultiplexHandle,
    config: AppConfig,
    store: state::SharedState,
    zone_commands: HashMap<usize, ZoneCommandSender>,
    snap_tx: tokio::sync::mpsc::Sender<SnapcastCmd>,
) {
    let zone_ga_map = build_zone_ga_map(&config);
    let client_ga_map = build_client_ga_map(&config);

    tracing::info!(
        zone_gas = zone_ga_map.len(),
        client_gas = client_ga_map.len(),
        "KNX listener started"
    );

    loop {
        let Some(cemi) = handle.recv().await else {
            tracing::warn!("KNX connection closed");
            break;
        };
        handle_incoming(
            &cemi,
            &zone_ga_map,
            &client_ga_map,
            &zone_commands,
            &snap_tx,
            &store,
        )
        .await;
    }
}

pub(crate) fn build_zone_ga_map(config: &AppConfig) -> HashMap<String, (usize, &'static str)> {
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
            (&knx.volume, "volume"),
            (&knx.playlist, "playlist"),
            (&knx.playlist_next, "playlist_next"),
            (&knx.playlist_previous, "playlist_previous"),
        ];
        for (ga_opt, action) in pairs {
            if let Some(ga) = ga_opt {
                map.insert(ga.clone(), (idx, *action));
            }
        }
    }
    map
}

pub(crate) fn build_client_ga_map(config: &AppConfig) -> HashMap<String, (usize, &'static str)> {
    let mut map = HashMap::new();
    for client_cfg in &config.clients {
        let idx = client_cfg.index;
        let knx = &client_cfg.knx;
        let pairs: &[(&Option<String>, &'static str)] = &[
            (&knx.mute, "mute"),
            (&knx.mute_toggle, "mute_toggle"),
            (&knx.volume, "volume"),
            (&knx.latency, "latency"),
            (&knx.zone, "zone"),
        ];
        for (ga_opt, action) in pairs {
            if let Some(ga) = ga_opt {
                map.insert(ga.clone(), (idx, *action));
            }
        }
    }
    map
}

pub(crate) async fn handle_incoming(
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
                "volume" => data
                    .as_ref()
                    .and_then(|d| decode_percent(d).map(|v| ZoneCommand::SetVolume(v as i32))),
                "playlist" => data.as_ref().and_then(|d| {
                    decode_percent(d).map(|v| ZoneCommand::SetPlaylist(v as usize, 0))
                }),
                "playlist_next" => Some(ZoneCommand::NextPlaylist),
                "playlist_previous" => Some(ZoneCommand::PreviousPlaylist),
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
                    "volume" => data
                        .as_ref()
                        .and_then(|d| decode_percent(d).map(|v| ClientAction::Volume(v as i32))),
                    "latency" => data
                        .as_ref()
                        .and_then(|d| decode_percent(d).map(|v| ClientAction::Latency(v as i32))),
                    "zone" => {
                        if let Some(target_zone) = data.as_ref().and_then(decode_percent) {
                            drop(s);
                            if let Some(c) = store.write().await.clients.get_mut(&client_idx) {
                                c.zone_index = target_zone as usize;
                            }
                            let _ = snap_tx.send(SnapcastCmd::ReconcileZones).await;
                            tracing::debug!(client = client_idx, zone = target_zone, ga = %ga, "KNX → client zone change");
                        }
                        return;
                    }
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

// ── DPT encode/decode ─────────────────────────────────────────

fn encode_bool(value: bool) -> DataPoint {
    DPT_1_1(value).to_data_point()
}

fn encode_percent(percent: u8) -> DataPoint {
    DPT_5_1(((percent as u16) * 255 / 100) as u8).to_data_point()
}

fn encode_string(value: &str) -> DataPoint {
    DPT_16_1(value.to_string()).to_data_point()
}

fn decode_bool(dp: &DataPoint) -> bool {
    DPT_1_1::from_data_point(dp).map(|v| v.0).unwrap_or(false)
}

fn decode_percent(dp: &DataPoint) -> Option<u8> {
    DPT_5_1::from_data_point(dp)
        .ok()
        .map(|v| ((v.0 as u16) * 100 / 255) as u8)
}

async fn write(handle: &mut MultiplexHandle, ga_str: &str, dp: DataPoint) {
    let ga = match GroupAddress::from_str(ga_str) {
        Ok(ga) => ga,
        Err(e) => {
            tracing::warn!(ga = ga_str, error = %e, "Invalid KNX GA");
            return;
        }
    };
    if let Err(e) = handle.group_write(ga, dp).await {
        tracing::warn!(ga = ga_str, error = %e, "KNX write failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_bool_via_dpt() {
        assert_eq!(encode_bool(true), DataPoint::Short(1));
        assert_eq!(encode_bool(false), DataPoint::Short(0));
    }

    #[test]
    fn round_trips_bool() {
        assert!(decode_bool(&encode_bool(true)));
        assert!(!decode_bool(&encode_bool(false)));
    }

    #[test]
    fn round_trips_percent() {
        assert_eq!(decode_percent(&encode_percent(0)), Some(0));
        assert_eq!(decode_percent(&encode_percent(100)), Some(100));
        let mid = decode_percent(&encode_percent(50)).unwrap();
        assert!((48..=52).contains(&mid), "50% round-tripped to {mid}");
    }

    #[test]
    fn encodes_string_via_dpt() {
        let dp = encode_string("Hello");
        assert!(matches!(dp, DataPoint::Long(_)));
    }

    // ── Integration tests for handle_incoming ─────────────────

    use crate::player::ZoneCommand;
    use knxkit::core::address::{DestinationAddress, GroupAddress, IndividualAddress};
    use knxkit::core::apdu::{APDU, Service};
    use knxkit::core::cemi::{CEMI, CEMIFlags, Priority};
    use knxkit::core::npdu::NPDU;
    use knxkit::core::tpdu::TPDU;
    use std::sync::Arc;
    use tokio::sync::{RwLock, mpsc};

    fn make_cemi(ga_str: &str, data: Option<DataPoint>) -> CEMI {
        let ga = GroupAddress::from_str(ga_str).unwrap();
        CEMI {
            mc: 0x29,
            flags: CEMIFlags::empty(),
            hops: 6,
            prio: Priority::Low,
            source: IndividualAddress::new(0x1001),
            destination: DestinationAddress::Group(ga),
            npdu: NPDU {
                tpdu: TPDU::DataGroup(APDU {
                    service: Service::GroupValueWrite,
                    data,
                }),
            },
        }
    }

    fn test_state() -> state::SharedState {
        let store: state::Store = serde_json::from_value(serde_json::json!({
            "zones": {},
            "clients": {
                "1": {
                    "name": "Test", "icon": "", "mac": "", "zone_index": 1,
                    "volume": 50, "base_volume": 50, "muted": false,
                    "latency_ms": 0, "connected": true, "snapcast_id": "snap-1"
                }
            }
        }))
        .unwrap();
        Arc::new(RwLock::new(store))
    }

    fn zone_ga_map() -> HashMap<String, (usize, &'static str)> {
        let mut m = HashMap::new();
        m.insert("1/0/1".into(), (1, "play"));
        m.insert("1/0/2".into(), (1, "pause"));
        m.insert("1/0/3".into(), (1, "stop"));
        m.insert("1/0/4".into(), (1, "next"));
        m.insert("1/0/5".into(), (1, "previous"));
        m.insert("1/0/6".into(), (1, "volume"));
        m.insert("1/0/7".into(), (1, "mute"));
        m.insert("1/0/8".into(), (1, "mute_toggle"));
        m.insert("1/0/9".into(), (1, "shuffle_toggle"));
        m.insert("1/0/10".into(), (1, "playlist"));
        m
    }

    fn client_ga_map() -> HashMap<String, (usize, &'static str)> {
        let mut m = HashMap::new();
        m.insert("2/0/1".into(), (1, "volume"));
        m.insert("2/0/2".into(), (1, "mute"));
        m.insert("2/0/3".into(), (1, "mute_toggle"));
        m
    }

    #[tokio::test]
    async fn zone_play_command() {
        let (tx, mut rx) = mpsc::channel(16);
        let mut cmds = HashMap::new();
        cmds.insert(1, tx);
        let (snap_tx, _snap_rx) = mpsc::channel(16);
        let state = test_state();
        let cemi = make_cemi("1/0/1", None);
        handle_incoming(
            &cemi,
            &zone_ga_map(),
            &client_ga_map(),
            &cmds,
            &snap_tx,
            &state,
        )
        .await;
        assert!(matches!(rx.recv().await, Some(ZoneCommand::Play)));
    }

    #[tokio::test]
    async fn zone_volume_from_knx() {
        let (tx, mut rx) = mpsc::channel(16);
        let mut cmds = HashMap::new();
        cmds.insert(1, tx);
        let (snap_tx, _snap_rx) = mpsc::channel(16);
        let state = test_state();
        let cemi = make_cemi("1/0/6", Some(encode_percent(75)));
        handle_incoming(
            &cemi,
            &zone_ga_map(),
            &client_ga_map(),
            &cmds,
            &snap_tx,
            &state,
        )
        .await;
        match rx.recv().await {
            Some(ZoneCommand::SetVolume(v)) => {
                assert!((73..=77).contains(&v), "expected ~75, got {v}")
            }
            other => panic!("expected SetVolume, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn zone_mute_from_knx() {
        let (tx, mut rx) = mpsc::channel(16);
        let mut cmds = HashMap::new();
        cmds.insert(1, tx);
        let (snap_tx, _snap_rx) = mpsc::channel(16);
        let state = test_state();
        let cemi = make_cemi("1/0/7", Some(encode_bool(true)));
        handle_incoming(
            &cemi,
            &zone_ga_map(),
            &client_ga_map(),
            &cmds,
            &snap_tx,
            &state,
        )
        .await;
        assert!(matches!(rx.recv().await, Some(ZoneCommand::SetMute(true))));
    }

    #[tokio::test]
    async fn zone_mute_toggle() {
        let (tx, mut rx) = mpsc::channel(16);
        let mut cmds = HashMap::new();
        cmds.insert(1, tx);
        let (snap_tx, _snap_rx) = mpsc::channel(16);
        let state = test_state();
        let cemi = make_cemi("1/0/8", None);
        handle_incoming(
            &cemi,
            &zone_ga_map(),
            &client_ga_map(),
            &cmds,
            &snap_tx,
            &state,
        )
        .await;
        assert!(matches!(rx.recv().await, Some(ZoneCommand::ToggleMute)));
    }

    #[tokio::test]
    async fn zone_playlist_from_knx() {
        let (tx, mut rx) = mpsc::channel(16);
        let mut cmds = HashMap::new();
        cmds.insert(1, tx);
        let (snap_tx, _snap_rx) = mpsc::channel(16);
        let state = test_state();
        let cemi = make_cemi("1/0/10", Some(encode_percent(3)));
        handle_incoming(
            &cemi,
            &zone_ga_map(),
            &client_ga_map(),
            &cmds,
            &snap_tx,
            &state,
        )
        .await;
        match rx.recv().await {
            Some(ZoneCommand::SetPlaylist(idx, 0)) => {
                assert!((2..=4).contains(&idx), "expected ~3, got {idx}")
            }
            other => panic!("expected SetPlaylist, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn client_volume_from_knx() {
        let (tx, _rx) = mpsc::channel(16);
        let mut cmds = HashMap::new();
        cmds.insert(1, tx);
        let (snap_tx, mut snap_rx) = mpsc::channel(16);
        let state = test_state();
        let cemi = make_cemi("2/0/1", Some(encode_percent(80)));
        handle_incoming(
            &cemi,
            &zone_ga_map(),
            &client_ga_map(),
            &cmds,
            &snap_tx,
            &state,
        )
        .await;
        match snap_rx.recv().await {
            Some(SnapcastCmd::Client {
                action: ClientAction::Volume(v),
                ..
            }) => {
                assert!((78..=82).contains(&v), "expected ~80, got {v}");
            }
            other => panic!("expected Client Volume, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn client_mute_from_knx() {
        let (tx, _rx) = mpsc::channel(16);
        let mut cmds = HashMap::new();
        cmds.insert(1, tx);
        let (snap_tx, mut snap_rx) = mpsc::channel(16);
        let state = test_state();
        let cemi = make_cemi("2/0/2", Some(encode_bool(true)));
        handle_incoming(
            &cemi,
            &zone_ga_map(),
            &client_ga_map(),
            &cmds,
            &snap_tx,
            &state,
        )
        .await;
        assert!(matches!(
            snap_rx.recv().await,
            Some(SnapcastCmd::Client {
                action: ClientAction::Mute(true),
                ..
            })
        ));
    }

    #[tokio::test]
    async fn client_mute_toggle_from_knx() {
        let (tx, _rx) = mpsc::channel(16);
        let mut cmds = HashMap::new();
        cmds.insert(1, tx);
        let (snap_tx, mut snap_rx) = mpsc::channel(16);
        let state = test_state();
        // Client starts unmuted, toggle should mute
        let cemi = make_cemi("2/0/3", None);
        handle_incoming(
            &cemi,
            &zone_ga_map(),
            &client_ga_map(),
            &cmds,
            &snap_tx,
            &state,
        )
        .await;
        assert!(matches!(
            snap_rx.recv().await,
            Some(SnapcastCmd::Client {
                action: ClientAction::Mute(true),
                ..
            })
        ));
    }

    #[tokio::test]
    async fn unknown_ga_ignored() {
        let (tx, mut rx) = mpsc::channel(16);
        let mut cmds = HashMap::new();
        cmds.insert(1, tx);
        let (snap_tx, mut snap_rx) = mpsc::channel(16);
        let state = test_state();
        let cemi = make_cemi("3/0/1", Some(encode_bool(true)));
        handle_incoming(
            &cemi,
            &zone_ga_map(),
            &client_ga_map(),
            &cmds,
            &snap_tx,
            &state,
        )
        .await;
        assert!(rx.try_recv().is_err());
        assert!(snap_rx.try_recv().is_err());
    }
}
