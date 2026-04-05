// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! ZonePlayer shared context and Snapcast command types.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::audio;

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
/// Command sent to the main loop for Snapcast JSON-RPC execution.
///
/// The Snapcast connection is `!Send`, so all JSON-RPC calls must happen
/// on the main task. Zone players and API handlers send commands via channel.
#[derive(Debug)]
pub enum SnapcastCmd {
    /// Group-level command (volume, mute, stream, clients, name).
    Group {
        group_id: String,
        action: GroupAction,
    },
    /// Client-level command (volume, mute, latency).
    Client {
        client_id: String,
        action: ClientAction,
    },
}

#[derive(Debug)]
pub enum GroupAction {
    Stream(String),
    Clients(Vec<String>),
    Name(String),
    Volume(i32),
    Mute(bool),
}

#[derive(Debug)]
pub enum ClientAction {
    Volume(i32),
    Mute(bool),
    Latency(i32),
}

/// Stop the current decode task and clear the PCM receiver.
pub async fn stop_decode(
    current: &mut Option<tokio::task::JoinHandle<()>>,
    rx: &mut Option<mpsc::Receiver<audio::PcmMessage>>,
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
    let notifications = {
        let mut s = store.write().await;
        let Some(zone) = s.zones.get_mut(&zone_index) else {
            return;
        };
        let old_track_title = zone.track.as_ref().map(|t| t.title.clone());
        let old_track_artist = zone.track.as_ref().map(|t| t.artist.clone());
        let old_position = zone.track.as_ref().map(|t| t.position_ms);
        f(zone);
        let mut notifs = vec![crate::api::ws::Notification::ZoneStateChanged {
            zone: zone_index,
            playback: zone.playback.to_string(),
            volume: zone.volume,
            muted: zone.muted,
            source: zone.source.to_string(),
            shuffle: zone.shuffle,
            repeat: zone.repeat,
            track_repeat: zone.track_repeat,
        }];
        // Send track changed if title or artist changed
        let new_track_title = zone.track.as_ref().map(|t| t.title.clone());
        let new_track_artist = zone.track.as_ref().map(|t| t.artist.clone());
        if old_track_title != new_track_title || old_track_artist != new_track_artist {
            if let Some(ref t) = zone.track {
                notifs.push(crate::api::ws::Notification::ZoneTrackChanged {
                    zone: zone_index,
                    title: t.title.clone(),
                    artist: t.artist.clone(),
                    album: t.album.clone(),
                    duration_ms: t.duration_ms,
                    position_ms: t.position_ms,
                });
            }
        }
        // Send progress if position changed
        let new_position = zone.track.as_ref().map(|t| t.position_ms);
        if old_position != new_position {
            if let Some(ref t) = zone.track {
                notifs.push(crate::api::ws::Notification::ZoneProgress {
                    zone: zone_index,
                    position_ms: t.position_ms,
                    duration_ms: t.duration_ms,
                });
            }
        }
        notifs
    };
    for n in notifications {
        let _ = notify.send(n);
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
        .send(SnapcastCmd::Group {
            group_id: gid.clone(),
            action: GroupAction::Stream(zone_config.stream_name.clone()),
        })
        .await;
    let _ = ctx
        .snap_tx
        .send(SnapcastCmd::Group {
            group_id: gid.clone(),
            action: GroupAction::Clients(snap_client_ids.clone()),
        })
        .await;
    let _ = ctx
        .snap_tx
        .send(SnapcastCmd::Group {
            group_id: gid.clone(),
            action: GroupAction::Name(zone_config.name.clone()),
        })
        .await;

    tracing::info!(zone = zone_index, group = %gid, clients = ?snap_client_ids, "Zone group configured");
    Some(gid)
}
