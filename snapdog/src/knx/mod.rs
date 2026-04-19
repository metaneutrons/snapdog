// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! KNX/IP integration via knx-rs.
//!
//! Bidirectional:
//! - **Publisher**: writes zone/client status to KNX group addresses on state changes
//! - **Listener**: receives KNX group writes and routes them as zone/client commands
//!
//! Uses knx-ip's [`Multiplexer`] to fan out a single connection into
//! independent publisher and listener handles. Supports both tunnel
//! (unicast) and router (multicast) connections via URL auto-detection.

use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{Context, Result};
use knx_core::address::GroupAddress;
use knx_core::cemi::CemiFrame;
use knx_core::dpt::{self, DPT_SCALING, DPT_STRING_8859_1, DPT_SWITCH, Dpt, DptValue};
use knx_ip::multiplex::{MultiplexHandle, Multiplexer};
use knx_ip::ops::GroupOps;

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
    let spec = knx_ip::parse_url(url).context("Invalid KNX URL")?;
    let conn = knx_ip::connect(spec)
        .await
        .context("KNX connection failed")?;
    tracing::info!(%url, "KNX connected");

    let mux = Multiplexer::new(conn);
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

    Ok(())
}

// ── Publisher ─────────────────────────────────────────────────

async fn publisher(
    handle: MultiplexHandle,
    config: AppConfig,
    store: state::SharedState,
    mut notify_rx: tokio::sync::broadcast::Receiver<crate::api::ws::Notification>,
) {
    tracing::info!("KNX publisher started");
    loop {
        match notify_rx.recv().await {
            Ok(crate::api::ws::Notification::ZoneStateChanged { zone, .. }) => {
                publish_zone_state(zone, &config, &store, &handle).await;
            }
            Ok(crate::api::ws::Notification::ZoneTrackChanged { zone, .. }) => {
                publish_zone_track(zone, &config, &store, &handle).await;
            }
            Ok(crate::api::ws::Notification::ZoneProgress { zone, .. }) => {
                publish_zone_progress(zone, &config, &store, &handle).await;
            }
            Ok(crate::api::ws::Notification::ClientStateChanged { client, .. }) => {
                publish_client_state(client, &config, &store, &handle).await;
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
    handle: &MultiplexHandle,
) {
    let s = store.read().await;
    let Some(zone) = s.zones.get(&zone_index) else {
        return;
    };
    let Some(zone_cfg) = config.zones.get(zone_index - 1) else {
        return;
    };
    let knx = &zone_cfg.knx;
    let playing = zone.playback.to_string() == "playing";

    if let Some(ref ga) = knx.volume_status {
        write(
            handle,
            ga,
            DPT_SCALING,
            &DptValue::Float(f64::from(zone.volume.clamp(0, 100) as u8)),
        )
        .await;
    }
    if let Some(ref ga) = knx.mute_status {
        write(handle, ga, DPT_SWITCH, &zone.muted.into()).await;
    }
    if let Some(ref ga) = knx.shuffle_status {
        write(handle, ga, DPT_SWITCH, &zone.shuffle.into()).await;
    }
    if let Some(ref ga) = knx.repeat_status {
        write(handle, ga, DPT_SWITCH, &zone.repeat.into()).await;
    }
    if let Some(ref ga) = knx.track_playing_status {
        write(handle, ga, DPT_SWITCH, &playing.into()).await;
    }
    if let Some(ref ga) = knx.track_repeat_status {
        write(handle, ga, DPT_SWITCH, &zone.track_repeat.into()).await;
    }
    if let Some(ref ga) = knx.control_status {
        write(handle, ga, DPT_SWITCH, &playing.into()).await;
    }
    if let Some(ref ga) = knx.playlist_status {
        write(
            handle,
            ga,
            DPT_SCALING,
            &DptValue::Float(f64::from(zone.playlist_index.unwrap_or(0) as u8)),
        )
        .await;
    }
}

async fn publish_zone_track(
    zone_index: usize,
    config: &AppConfig,
    store: &state::SharedState,
    handle: &MultiplexHandle,
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
            write(
                handle,
                ga,
                DPT_STRING_8859_1,
                &DptValue::from(track.title.as_str()),
            )
            .await;
        }
        if let Some(ref ga) = knx.track_artist_status {
            write(
                handle,
                ga,
                DPT_STRING_8859_1,
                &DptValue::from(track.artist.as_str()),
            )
            .await;
        }
        if let Some(ref ga) = knx.track_album_status {
            write(
                handle,
                ga,
                DPT_STRING_8859_1,
                &DptValue::from(track.album.as_str()),
            )
            .await;
        }
        if let Some(ref ga) = knx.track_progress_status {
            let pct = if track.duration_ms > 0 {
                ((track.position_ms as f64 / track.duration_ms as f64) * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            };
            write(handle, ga, DPT_SCALING, &DptValue::Float(pct)).await;
        }
    }
}

async fn publish_zone_progress(
    zone_index: usize,
    config: &AppConfig,
    store: &state::SharedState,
    handle: &MultiplexHandle,
) {
    let s = store.read().await;
    let Some(zone) = s.zones.get(&zone_index) else {
        return;
    };
    let Some(zone_cfg) = config.zones.get(zone_index - 1) else {
        return;
    };
    if let Some(ref ga) = zone_cfg.knx.track_progress_status {
        let pct = zone.track.as_ref().map_or(0.0, |t| {
            if t.duration_ms > 0 {
                ((t.position_ms as f64 / t.duration_ms as f64) * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            }
        });
        write(handle, ga, DPT_SCALING, &DptValue::Float(pct)).await;
    }
}

async fn publish_client_state(
    client_index: usize,
    config: &AppConfig,
    store: &state::SharedState,
    handle: &MultiplexHandle,
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
            DPT_SCALING,
            &DptValue::Float(f64::from(client.base_volume.clamp(0, 100) as u8)),
        )
        .await;
    }
    if let Some(ref ga) = knx.mute_status {
        write(handle, ga, DPT_SWITCH, &client.muted.into()).await;
    }
    if let Some(ref ga) = knx.connected_status {
        write(handle, ga, DPT_SWITCH, &client.connected.into()).await;
    }
    if let Some(ref ga) = knx.zone_status {
        write(
            handle,
            ga,
            DPT_SCALING,
            &DptValue::Float(f64::from(client.zone_index as u8)),
        )
        .await;
    }
    if let Some(ref ga) = knx.latency_status {
        write(
            handle,
            ga,
            DPT_SCALING,
            &DptValue::Float(f64::from(client.latency_ms.clamp(0, 255) as u8)),
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
    cemi: &CemiFrame,
    zone_ga_map: &HashMap<String, (usize, &str)>,
    client_ga_map: &HashMap<String, (usize, &str)>,
    zone_commands: &HashMap<usize, ZoneCommandSender>,
    snap_tx: &tokio::sync::mpsc::Sender<SnapcastCmd>,
    store: &state::SharedState,
) {
    let Some((ga, data)) = cemi.as_group_write() else {
        return;
    };
    let ga_str = format!("{ga}");

    if let Some(&(zone_idx, action)) = zone_ga_map.get(&ga_str) {
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
                "mute" => Some(ZoneCommand::SetMute(decode_bool(&data))),
                "shuffle" => Some(ZoneCommand::SetShuffle(decode_bool(&data))),
                "repeat" => Some(ZoneCommand::SetRepeat(decode_bool(&data))),
                "track_repeat" => Some(ZoneCommand::SetTrackRepeat(decode_bool(&data))),
                "volume" => decode_percent(&data).map(|v| ZoneCommand::SetVolume(v as i32)),
                "playlist" => {
                    decode_percent(&data).map(|v| ZoneCommand::SetPlaylist(v as usize, 0))
                }
                "playlist_next" => Some(ZoneCommand::NextPlaylist),
                "playlist_previous" => Some(ZoneCommand::PreviousPlaylist),
                _ => None,
            };
            if let Some(cmd) = cmd {
                tracing::debug!(zone = zone_idx, ga = %ga_str, "KNX → zone command");
                let _ = tx.send(cmd).await;
            }
        }
    }

    if let Some(&(client_idx, action)) = client_ga_map.get(&ga_str) {
        let s = store.read().await;
        if let Some(client) = s.clients.get(&client_idx) {
            if let Some(ref snap_id) = client.snapcast_id {
                let cmd = match action {
                    "mute_toggle" => Some(ClientAction::Mute(!client.muted)),
                    "mute" => Some(ClientAction::Mute(decode_bool(&data))),
                    "volume" => decode_percent(&data).map(|v| ClientAction::Volume(v as i32)),
                    "latency" => decode_percent(&data).map(|v| ClientAction::Latency(v as i32)),
                    "zone" => {
                        if let Some(target_zone) = decode_percent(&data) {
                            drop(s);
                            if let Some(c) = store.write().await.clients.get_mut(&client_idx) {
                                c.zone_index = target_zone as usize;
                            }
                            let _ = snap_tx.send(SnapcastCmd::ReconcileZones).await;
                            tracing::debug!(client = client_idx, zone = target_zone, ga = %ga_str, "KNX → client zone change");
                        }
                        return;
                    }
                    _ => None,
                };
                if let Some(action) = cmd {
                    let snap_id = snap_id.clone();
                    drop(s);
                    tracing::debug!(client = client_idx, ga = %ga_str, "KNX → client command");
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

// ── DPT decode helpers ────────────────────────────────────────

fn decode_bool(payload: &[u8]) -> bool {
    dpt::decode(DPT_SWITCH, payload)
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn decode_percent(payload: &[u8]) -> Option<u8> {
    dpt::decode(DPT_SCALING, payload)
        .ok()
        .and_then(|v| v.as_f64())
        .map(|v| v.clamp(0.0, 100.0).round() as u8)
}

// ── Write helper ──────────────────────────────────────────────

async fn write(handle: &MultiplexHandle, ga_str: &str, dpt: Dpt, value: &DptValue) {
    let ga = match GroupAddress::from_str(ga_str) {
        Ok(ga) => ga,
        Err(e) => {
            tracing::warn!(ga = ga_str, error = %e, "Invalid KNX GA");
            return;
        }
    };
    if let Err(e) = handle.group_write_value(ga, dpt, value).await {
        tracing::warn!(ga = ga_str, error = %e, "KNX write failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::ZoneCommand;
    use knx_core::address::{DestinationAddress, IndividualAddress};
    use knx_core::message::MessageCode;
    use knx_core::types::Priority;
    use std::sync::Arc;
    use tokio::sync::{RwLock, mpsc};

    fn encode_bool(value: bool) -> Vec<u8> {
        dpt::encode(DPT_SWITCH, &value.into()).unwrap()
    }

    fn encode_percent(percent: u8) -> Vec<u8> {
        dpt::encode(DPT_SCALING, &DptValue::Float(f64::from(percent))).unwrap()
    }

    fn encode_string(value: &str) -> Vec<u8> {
        dpt::encode(DPT_STRING_8859_1, &DptValue::from(value)).unwrap()
    }

    /// Build a GroupValueWrite CemiFrame for testing.
    fn make_cemi(ga_str: &str, data: Option<&[u8]>) -> CemiFrame {
        let ga = GroupAddress::from_str(ga_str).unwrap();
        let mut payload = vec![0x00]; // TPCI: unnumbered data
        match data {
            Some(d) if d.len() == 1 && d[0] <= 0x3F => {
                payload.push(0x80 | (d[0] & 0x3F));
            }
            Some(d) => {
                payload.push(0x80);
                payload.extend_from_slice(d);
            }
            None => {
                payload.push(0x80); // GroupValueWrite, no data
            }
        }
        CemiFrame::new_l_data(
            MessageCode::LDataInd,
            IndividualAddress::from_raw(0x1001),
            DestinationAddress::Group(ga),
            Priority::Low,
            &payload,
        )
    }

    #[test]
    fn encodes_bool_via_dpt() {
        assert_eq!(encode_bool(true), vec![0x01]);
        assert_eq!(encode_bool(false), vec![0x00]);
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
        assert_eq!(decode_percent(&encode_percent(50)), Some(50));
    }

    #[test]
    fn encodes_string_via_dpt() {
        let encoded = encode_string("Hello");
        assert_eq!(encoded.len(), 14); // DPT 16 is always 14 bytes
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
        let cemi = make_cemi("1/0/6", Some(&encode_percent(75)));
        handle_incoming(
            &cemi,
            &zone_ga_map(),
            &client_ga_map(),
            &cmds,
            &snap_tx,
            &state,
        )
        .await;
        assert!(matches!(rx.recv().await, Some(ZoneCommand::SetVolume(75))));
    }

    #[tokio::test]
    async fn zone_mute_from_knx() {
        let (tx, mut rx) = mpsc::channel(16);
        let mut cmds = HashMap::new();
        cmds.insert(1, tx);
        let (snap_tx, _snap_rx) = mpsc::channel(16);
        let state = test_state();
        let cemi = make_cemi("1/0/7", Some(&encode_bool(true)));
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
        let cemi = make_cemi("1/0/10", Some(&encode_percent(3)));
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
            rx.recv().await,
            Some(ZoneCommand::SetPlaylist(3, 0))
        ));
    }

    #[tokio::test]
    async fn client_volume_from_knx() {
        let (tx, _rx) = mpsc::channel(16);
        let mut cmds = HashMap::new();
        cmds.insert(1, tx);
        let (snap_tx, mut snap_rx) = mpsc::channel(16);
        let state = test_state();
        let cemi = make_cemi("2/0/1", Some(&encode_percent(80)));
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
                action: ClientAction::Volume(80),
                ..
            })
        ));
    }

    #[tokio::test]
    async fn client_mute_from_knx() {
        let (tx, _rx) = mpsc::channel(16);
        let mut cmds = HashMap::new();
        cmds.insert(1, tx);
        let (snap_tx, mut snap_rx) = mpsc::channel(16);
        let state = test_state();
        let cemi = make_cemi("2/0/2", Some(&encode_bool(true)));
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
        let cemi = make_cemi("3/0/1", Some(&encode_bool(true)));
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
