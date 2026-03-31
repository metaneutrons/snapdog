// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Convention-over-configuration: auto-generate KNX addresses, sink paths, stream names.

use anyhow::{Context, Result};

use super::types::*;

const TCP_SOURCE_BASE_PORT: u16 = 4952;

/// Resolve a raw zone config into a fully populated ZoneConfig.
pub fn resolve_zone(index: usize, raw: RawZoneConfig, audio: &AudioConfig) -> Result<ZoneConfig> {
    let n = index;
    let _ = audio; // Reserved for future sample format in stream name

    Ok(ZoneConfig {
        index,
        name: raw.name,
        icon: raw.icon,
        sink: raw.sink.unwrap_or_else(|| format!("/snapsinks/zone{n}")),
        stream_name: format!("Zone{n}"),
        tcp_source_port: TCP_SOURCE_BASE_PORT + n as u16,
        knx: resolve_zone_knx(n, raw.knx),
    })
}

/// Resolve a raw client config into a fully populated ClientConfig.
pub fn resolve_client(
    index: usize,
    raw: RawClientConfig,
    zone_names: &[&str],
) -> Result<ClientConfig> {
    let zone_index = zone_names
        .iter()
        .position(|&name| name == raw.zone)
        .map(|i| i + 1)
        .with_context(|| {
            format!(
                "Client '{}' references unknown zone '{}'. Available: {:?}",
                raw.name, raw.zone, zone_names
            )
        })?;

    Ok(ClientConfig {
        index,
        name: raw.name,
        mac: raw.mac,
        zone_index,
        icon: raw.icon,
        knx: resolve_client_knx(index, raw.knx),
    })
}

/// Zone N KNX convention: N/1/x (control), N/2/x (volume), N/3/x (track), N/4/x (playlist)
fn resolve_zone_knx(n: usize, raw: RawZoneKnxConfig) -> ZoneKnxAddresses {
    ZoneKnxAddresses {
        play: raw.play.unwrap_or_else(|| format!("{n}/1/1")),
        pause: raw.pause.unwrap_or_else(|| format!("{n}/1/2")),
        stop: raw.stop.unwrap_or_else(|| format!("{n}/1/3")),
        volume: raw.volume.unwrap_or_else(|| format!("{n}/2/1")),
        volume_status: raw.volume_status.unwrap_or_else(|| format!("{n}/2/2")),
        volume_dim: raw.volume_dim.unwrap_or_else(|| format!("{n}/2/3")),
        mute: raw.mute.unwrap_or_else(|| format!("{n}/2/5")),
        mute_status: raw.mute_status.unwrap_or_else(|| format!("{n}/2/6")),
        mute_toggle: raw.mute_toggle.unwrap_or_else(|| format!("{n}/2/7")),
        track_next: raw.track_next.unwrap_or_else(|| format!("{n}/1/4")),
        track_previous: raw.track_previous.unwrap_or_else(|| format!("{n}/1/5")),
        playlist: raw.playlist.unwrap_or_else(|| format!("{n}/4/1")),
        playlist_status: raw.playlist_status.unwrap_or_else(|| format!("{n}/4/2")),
        shuffle: raw.shuffle.unwrap_or_else(|| format!("{n}/4/5")),
        shuffle_status: raw.shuffle_status.unwrap_or_else(|| format!("{n}/4/6")),
        repeat: raw.repeat.unwrap_or_else(|| format!("{n}/4/8")),
        repeat_status: raw.repeat_status.unwrap_or_else(|| format!("{n}/4/9")),
    }
}

/// Client N KNX convention: 3/N/x
fn resolve_client_knx(n: usize, raw: RawClientKnxConfig) -> ClientKnxAddresses {
    ClientKnxAddresses {
        volume: raw.volume.unwrap_or_else(|| format!("3/{n}/1")),
        volume_status: raw.volume_status.unwrap_or_else(|| format!("3/{n}/2")),
        volume_dim: raw.volume_dim.unwrap_or_else(|| format!("3/{n}/3")),
        mute: raw.mute.unwrap_or_else(|| format!("3/{n}/5")),
        mute_status: raw.mute_status.unwrap_or_else(|| format!("3/{n}/6")),
        mute_toggle: raw.mute_toggle.unwrap_or_else(|| format!("3/{n}/7")),
        zone: raw.zone.unwrap_or_else(|| format!("3/{n}/10")),
        zone_status: raw.zone_status.unwrap_or_else(|| format!("3/{n}/11")),
        connected_status: raw.connected_status.unwrap_or_else(|| format!("3/{n}/12")),
    }
}
