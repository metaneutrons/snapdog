// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! ZonePlayer shared context and Snapcast command types.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::config::AppConfig;
use crate::state;
use crate::state::cover::SharedCoverCache;

pub type NotifySender = tokio::sync::broadcast::Sender<crate::api::ws::Notification>;
pub type ZoneCommandSender = mpsc::Sender<super::ZoneCommand>;
pub type SnapcastCmdSender = mpsc::Sender<SnapcastCmd>;

/// Shared context for all ZonePlayers. Cloned (Arc) per zone task.
pub struct ZonePlayerContext {
    pub config: Arc<AppConfig>,
    pub store: state::SharedState,
    pub covers: SharedCoverCache,
    pub notify: NotifySender,
    pub snap_tx: SnapcastCmdSender,
    /// Pre-extracted: Snapcast client MAC (lowercase) → Snapcast client ID.
    pub client_mac_map: HashMap<String, String>,
    /// Pre-extracted: all Snapcast group IDs.
    pub group_ids: Vec<String>,
    /// Pre-extracted: group ID → client IDs in that group.
    pub group_clients: HashMap<String, Vec<String>>,
}

/// Commands sent to the Snapcast manager task (runs on main thread because
/// SnapcastConnection is !Send).
#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
pub enum SnapcastCmd {
    SetGroupStream {
        group_id: String,
        stream_id: String,
    },
    SetGroupClients {
        group_id: String,
        client_ids: Vec<String>,
    },
    SetGroupName {
        group_id: String,
        name: String,
    },
    SetGroupVolume {
        group_id: String,
        percent: i32,
    },
    SetGroupMute {
        group_id: String,
        muted: bool,
    },
}

/// Stop the current decode task and clear the PCM receiver.
pub async fn stop_decode(
    current: &mut Option<tokio::task::JoinHandle<()>>,
    rx: &mut Option<mpsc::Receiver<Vec<u8>>>,
) {
    if let Some(handle) = current.take() {
        handle.abort();
    }
    *rx = None;
}

/// Update zone state and broadcast a WebSocket notification.
pub async fn update_and_notify(
    store: &state::SharedState,
    zone_index: usize,
    notify: &NotifySender,
    f: impl FnOnce(&mut state::ZoneState),
) {
    let mut s = store.write().await;
    if let Some(zone) = s.zones.get_mut(&zone_index) {
        f(zone);
        let _ = notify.send(crate::api::ws::Notification::ZoneStateChanged {
            zone: zone_index,
            playback: format!("{:?}", zone.playback).to_lowercase(),
            volume: zone.volume,
            muted: zone.muted,
            source: format!("{:?}", zone.source).to_lowercase(),
        });
    }
}

/// Set up Snapcast group for a zone: find clients by MAC, assign to group, set stream.
pub async fn setup_zone_group(zone_index: usize, ctx: &ZonePlayerContext) -> Option<String> {
    let zone_config = &ctx.config.zones[zone_index - 1];
    let zone_macs: Vec<String> = ctx
        .config
        .clients
        .iter()
        .filter(|c| c.zone_index == zone_index)
        .map(|c| c.mac.to_lowercase())
        .collect();

    if zone_macs.is_empty() {
        return None;
    }

    let snap_client_ids: Vec<String> = zone_macs
        .iter()
        .filter_map(|mac| ctx.client_mac_map.get(mac).cloned())
        .collect();

    if snap_client_ids.is_empty() {
        tracing::warn!(zone = zone_index, macs = ?zone_macs, "No Snapcast clients found");
        return None;
    }

    let gid = ctx
        .group_clients
        .iter()
        .find(|(_, clients)| clients.iter().any(|c| snap_client_ids.contains(c)))
        .map(|(id, _)| id.clone())
        .or_else(|| ctx.group_ids.first().cloned());

    let Some(gid) = gid else {
        tracing::warn!(zone = zone_index, "No Snapcast groups available");
        return None;
    };

    let _ = ctx
        .snap_tx
        .send(SnapcastCmd::SetGroupStream {
            group_id: gid.clone(),
            stream_id: zone_config.stream_name.clone(),
        })
        .await;
    let _ = ctx
        .snap_tx
        .send(SnapcastCmd::SetGroupClients {
            group_id: gid.clone(),
            client_ids: snap_client_ids.clone(),
        })
        .await;
    let _ = ctx
        .snap_tx
        .send(SnapcastCmd::SetGroupName {
            group_id: gid.clone(),
            name: zone_config.name.clone(),
        })
        .await;

    tracing::info!(zone = zone_index, group = %gid, clients = ?snap_client_ids, "Zone group configured");
    Some(gid)
}
