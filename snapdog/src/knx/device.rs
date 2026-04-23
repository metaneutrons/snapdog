// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! Device-mode KNX transport — runs as ETS-programmable KNX/IP device.
//!
//! Wraps `DeviceServer` + `Bau`. The BAU runs in a dedicated task;
//! the transport communicates via channels.

use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result};
use knx_core::address::{GroupAddress, IndividualAddress};
use knx_core::dpt::{Dpt, DptValue};
use knx_device::bau::Bau;
use knx_device::device_object;
use knx_ip::tunnel_server::{DeviceServer, ServerEvent};
use tokio::sync::{mpsc, oneshot};

use super::group_objects::mem;
use super::group_objects::{
    self, CGO_CONNECTED, CGO_LATENCY, CGO_LATENCY_STATUS, CGO_MUTE, CGO_MUTE_STATUS,
    CGO_MUTE_TOGGLE, CGO_VOLUME, CGO_VOLUME_DIM, CGO_VOLUME_STATUS, CGO_ZONE, CGO_ZONE_STATUS,
    CLIENT_GOS, MAX_CLIENTS, MAX_ZONES, TOTAL_GO_COUNT, ZGO_CONTROL_STATUS, ZGO_MUTE,
    ZGO_MUTE_STATUS, ZGO_MUTE_TOGGLE, ZGO_PAUSE, ZGO_PLAY, ZGO_PLAYLIST, ZGO_PLAYLIST_NEXT,
    ZGO_PLAYLIST_PREVIOUS, ZGO_PLAYLIST_STATUS, ZGO_PRESENCE, ZGO_PRESENCE_ENABLE,
    ZGO_PRESENCE_SOURCE_OVERRIDE, ZGO_PRESENCE_TIMEOUT, ZGO_PRESENCE_TIMER_ACTIVE, ZGO_REPEAT,
    ZGO_REPEAT_STATUS, ZGO_REPEAT_TOGGLE, ZGO_SHUFFLE, ZGO_SHUFFLE_STATUS, ZGO_SHUFFLE_TOGGLE,
    ZGO_STOP, ZGO_TRACK_ALBUM, ZGO_TRACK_ARTIST, ZGO_TRACK_NEXT, ZGO_TRACK_PLAYING,
    ZGO_TRACK_PREVIOUS, ZGO_TRACK_PROGRESS, ZGO_TRACK_REPEAT, ZGO_TRACK_REPEAT_STATUS,
    ZGO_TRACK_REPEAT_TOGGLE, ZGO_TRACK_TITLE, ZGO_VOLUME, ZGO_VOLUME_DIM, ZGO_VOLUME_STATUS,
    ZONE_GOS,
};
use super::transport::KnxTransport;

/// Parsed ETS parameters from BAU memory.
#[derive(Debug, Default)]
pub(crate) struct EtsParams {
    pub zone_active: [bool; 10],
    pub zone_max_volume: [u8; 10],
    pub client_active: [bool; 10],
    pub client_max_volume: [u8; 10],
    pub client_default_zone: [u8; 10],
    pub client_default_latency: [u8; 10],
}

/// Parse ETS parameters from BAU memory area.
pub(crate) fn parse_ets_memory(data: &[u8]) -> EtsParams {
    let mut p = EtsParams::default();
    if data.len() < mem::TOTAL {
        return p;
    }
    for i in 0..10 {
        p.zone_active[i] = data[mem::ZONE_ACTIVE + i] != 0;
        p.zone_max_volume[i] = data[mem::ZONE_MAX_VOL + i];
        p.client_active[i] = data[mem::CLIENT_ACTIVE + i] != 0;
        p.client_max_volume[i] = data[mem::CLIENT_MAX_VOL + i];
        p.client_default_zone[i] = data[mem::CLIENT_DEF_ZONE + i];
        p.client_default_latency[i] = data[mem::CLIENT_DEF_LAT + i];
    }
    p
}

/// Command sent to the BAU task.
enum BauCmd {
    /// Write a DPT-encoded value to a group object by GA string.
    Write {
        ga: GroupAddress,
        dpt: Dpt,
        value: DptValue,
    },
    /// Poll for the next group object updated from the bus.
    /// Returns (GroupAddress, raw_data) or None.
    NextUpdated {
        reply: oneshot::Sender<Option<(GroupAddress, Vec<u8>)>>,
    },
    /// Process an incoming CEMI frame from the server.
    ProcessFrame { frame: knx_core::cemi::CemiFrame },
}

/// Device-mode publisher: sends values to the BAU task.
pub(crate) struct DevicePublisher {
    cmd_tx: mpsc::Sender<BauCmd>,
}

/// Device-mode listener: receives group object updates from the BAU task.
pub(crate) struct DeviceListener {
    update_rx: mpsc::Receiver<(GroupAddress, Vec<u8>)>,
}

/// Default path for persisted ETS memory.
const ETS_MEMORY_PATH: &str = "knx-memory.bin";

/// Start the device server and BAU, returning a publisher/listener pair.
pub(crate) async fn start_device_transport(
    individual_address: &str,
    config: &crate::config::AppConfig,
) -> Result<(DevicePublisher, DeviceListener)> {
    let ia = IndividualAddress::from_str(individual_address)
        .map_err(|e| anyhow::anyhow!("Invalid individual address: {e}"))?;

    let server = DeviceServer::start(Ipv4Addr::UNSPECIFIED)
        .await
        .context("Failed to start KNX device server")?;

    let persist = config.knx.persist_ets_config.unwrap_or(true);
    let persist_path = if persist {
        Some(PathBuf::from(ETS_MEMORY_PATH))
    } else {
        None
    };

    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    let (update_tx, update_rx) = mpsc::channel(64);

    let config_arc = std::sync::Arc::new(config.clone());
    tokio::spawn(bau_task(
        ia,
        config_arc,
        server,
        cmd_rx,
        update_tx,
        persist_path,
    ));

    tracing::info!(%individual_address, persist, "KNX device mode started");

    Ok((DevicePublisher { cmd_tx }, DeviceListener { update_rx }))
}

impl KnxTransport for DevicePublisher {
    async fn write(&self, ga: GroupAddress, dpt: Dpt, value: &DptValue) {
        let _ = self
            .cmd_tx
            .send(BauCmd::Write {
                ga,
                dpt,
                value: value.clone(),
            })
            .await;
    }

    async fn recv_group_write(&mut self) -> Option<(GroupAddress, Vec<u8>)> {
        // Publisher doesn't receive — block forever
        std::future::pending().await
    }
}

impl KnxTransport for DeviceListener {
    async fn write(&self, _ga: GroupAddress, _dpt: Dpt, _value: &DptValue) {
        // Listener doesn't write
    }

    async fn recv_group_write(&mut self) -> Option<(GroupAddress, Vec<u8>)> {
        self.update_rx.recv().await
    }
}

// ── BAU task ──────────────────────────────────────────────────

async fn bau_task(
    ia: IndividualAddress,
    config: std::sync::Arc<crate::config::AppConfig>,
    mut server: DeviceServer,
    mut cmd_rx: mpsc::Receiver<BauCmd>,
    update_tx: mpsc::Sender<(GroupAddress, Vec<u8>)>,
    persist_path: Option<PathBuf>,
) {
    let mut bau = build_bau(ia, &config);

    // Load persisted ETS memory if available
    if let Some(ref path) = persist_path {
        if let Ok(data) = std::fs::read(path) {
            bau.set_memory_area(data);
            // TODO: load_tables_from_memory requires known offsets —
            // these are determined by the ETS application program layout.
            // For now, the memory area is restored so ETS MemoryRead works.
            tracing::info!(path = %path.display(), "Loaded persisted ETS memory");
        }
    }

    loop {
        tokio::select! {
            event = server.recv() => {
                let Some(event) = event else { break };
                let frame = match event {
                    ServerEvent::TunnelFrame(f) | ServerEvent::RoutingFrame(f) => f,
                };
                bau.process_frame(&frame);
                bau.poll();

                while let Some(out) = bau.next_outgoing_frame() {
                    let _ = server.send_frame(out).await;
                }

                // Persist memory after ETS programming
                if let Some(ref path) = persist_path {
                    let mem = bau.memory_area();
                    if !mem.is_empty() {
                        if let Err(e) = std::fs::write(path, mem) {
                            tracing::warn!(error = %e, "Failed to persist ETS memory");
                        }
                    }
                }

                dispatch_updated_gos(&mut bau, &update_tx).await;
            }

            cmd = cmd_rx.recv() => {
                let Some(cmd) = cmd else { break };
                match cmd {
                    BauCmd::Write { ga, dpt, value } => {
                        handle_write(&mut bau, &server, ga, dpt, &value).await;
                    }
                    BauCmd::ProcessFrame { frame } => {
                        bau.process_frame(&frame);
                        bau.poll();
                        while let Some(out) = bau.next_outgoing_frame() {
                            let _ = server.send_frame(out).await;
                        }
                        dispatch_updated_gos(&mut bau, &update_tx).await;
                    }
                    BauCmd::NextUpdated { reply } => {
                        // Direct poll — used if needed
                        let result = find_next_updated(&mut bau);
                        let _ = reply.send(result);
                    }
                }
            }
        }
    }

    tracing::info!("KNX BAU task ended");
}

/// Build a BAU with 460 group objects configured with correct DPTs,
/// and address/association tables from TOML config.
fn build_bau(ia: IndividualAddress, config: &crate::config::AppConfig) -> Bau {
    let device = device_object::new_device_object(
        [0x00, 0xFA, 0x01, 0x02, 0x03, 0x04], // serial (OpenKNX 0xFA)
        [0x00; 6],                            // hardware type
    );
    let mut bau = Bau::new(device, TOTAL_GO_COUNT as u16, 1);
    device_object::set_individual_address(bau.device_mut(), ia.raw());

    // Configure zone GOs with DPTs
    for zone in 1..=MAX_ZONES {
        for (i, go_def) in ZONE_GOS.iter().enumerate() {
            let asap = group_objects::zone_asap(zone, i);
            if let Some(go) = bau.group_objects.get_mut(asap) {
                go.set_dpt(go_def.dpt);
            }
        }
    }

    // Configure client GOs with DPTs
    for client in 1..=MAX_CLIENTS {
        for (i, go_def) in CLIENT_GOS.iter().enumerate() {
            let asap = group_objects::client_asap(client, i);
            if let Some(go) = bau.group_objects.get_mut(asap) {
                go.set_dpt(go_def.dpt);
            }
        }
    }

    // Build address table and association table from TOML KNX config
    build_tables_from_config(&mut bau, config);

    bau
}

/// Build address and association tables from TOML zone/client KNX addresses.
fn build_tables_from_config(bau: &mut Bau, config: &crate::config::AppConfig) {
    // Collect all (GA string, ASAP) pairs
    let mut ga_asap_pairs: Vec<(u16, u16)> = Vec::new();

    for zone_cfg in &config.zones {
        let idx = zone_cfg.index;
        let knx = &zone_cfg.knx;
        // Map each configured GA to its zone GO ASAP
        let zone_gas: &[(&Option<String>, usize)] = &[
            (&knx.play, ZGO_PLAY),
            (&knx.pause, ZGO_PAUSE),
            (&knx.stop, ZGO_STOP),
            (&knx.track_next, ZGO_TRACK_NEXT),
            (&knx.track_previous, ZGO_TRACK_PREVIOUS),
            (&knx.volume, ZGO_VOLUME),
            (&knx.volume_status, ZGO_VOLUME_STATUS),
            (&knx.volume_dim, ZGO_VOLUME_DIM),
            (&knx.mute, ZGO_MUTE),
            (&knx.mute_status, ZGO_MUTE_STATUS),
            (&knx.mute_toggle, ZGO_MUTE_TOGGLE),
            (&knx.control_status, ZGO_CONTROL_STATUS),
            (&knx.track_playing_status, ZGO_TRACK_PLAYING),
            (&knx.shuffle, ZGO_SHUFFLE),
            (&knx.shuffle_status, ZGO_SHUFFLE_STATUS),
            (&knx.shuffle_toggle, ZGO_SHUFFLE_TOGGLE),
            (&knx.repeat, ZGO_REPEAT),
            (&knx.repeat_status, ZGO_REPEAT_STATUS),
            (&knx.repeat_toggle, ZGO_REPEAT_TOGGLE),
            (&knx.track_repeat, ZGO_TRACK_REPEAT),
            (&knx.track_repeat_status, ZGO_TRACK_REPEAT_STATUS),
            (&knx.track_repeat_toggle, ZGO_TRACK_REPEAT_TOGGLE),
            (&knx.playlist, ZGO_PLAYLIST),
            (&knx.playlist_status, ZGO_PLAYLIST_STATUS),
            (&knx.playlist_next, ZGO_PLAYLIST_NEXT),
            (&knx.playlist_previous, ZGO_PLAYLIST_PREVIOUS),
            (&knx.track_title_status, ZGO_TRACK_TITLE),
            (&knx.track_artist_status, ZGO_TRACK_ARTIST),
            (&knx.track_album_status, ZGO_TRACK_ALBUM),
            (&knx.track_progress_status, ZGO_TRACK_PROGRESS),
            (&knx.presence, ZGO_PRESENCE),
            (&knx.presence_enable, ZGO_PRESENCE_ENABLE),
            (&knx.presence_timeout, ZGO_PRESENCE_TIMEOUT),
            (&knx.presence_timer_status, ZGO_PRESENCE_TIMER_ACTIVE),
            (&knx.presence_source_override, ZGO_PRESENCE_SOURCE_OVERRIDE),
        ];
        for (ga_opt, go_idx) in zone_gas {
            if let Some(ga_str) = ga_opt {
                if let Ok(ga) = GroupAddress::from_str(ga_str) {
                    let asap = group_objects::zone_asap(idx, *go_idx);
                    ga_asap_pairs.push((ga.raw(), asap));
                }
            }
        }
    }

    for client_cfg in &config.clients {
        let idx = client_cfg.index;
        let knx = &client_cfg.knx;
        let client_gas: &[(&Option<String>, usize)] = &[
            (&knx.volume, CGO_VOLUME),
            (&knx.volume_status, CGO_VOLUME_STATUS),
            (&knx.volume_dim, CGO_VOLUME_DIM),
            (&knx.mute, CGO_MUTE),
            (&knx.mute_status, CGO_MUTE_STATUS),
            (&knx.mute_toggle, CGO_MUTE_TOGGLE),
            (&knx.latency, CGO_LATENCY),
            (&knx.latency_status, CGO_LATENCY_STATUS),
            (&knx.zone, CGO_ZONE),
            (&knx.zone_status, CGO_ZONE_STATUS),
            (&knx.connected_status, CGO_CONNECTED),
        ];
        for (ga_opt, go_idx) in client_gas {
            if let Some(ga_str) = ga_opt {
                if let Ok(ga) = GroupAddress::from_str(ga_str) {
                    let asap = group_objects::client_asap(idx, *go_idx);
                    ga_asap_pairs.push((ga.raw(), asap));
                }
            }
        }
    }

    if ga_asap_pairs.is_empty() {
        return;
    }

    // Build address table: unique GAs → TSAPs (1-based)
    let mut unique_gas: Vec<u16> = ga_asap_pairs.iter().map(|(ga, _)| *ga).collect();
    unique_gas.sort_unstable();
    unique_gas.dedup();

    let mut addr_data = Vec::new();
    let count = unique_gas.len() as u16;
    addr_data.extend_from_slice(&count.to_be_bytes());
    for ga in &unique_gas {
        addr_data.extend_from_slice(&ga.to_be_bytes());
    }
    bau.address_table.load(&addr_data);

    // Build association table: (TSAP, ASAP) pairs
    let mut assoc_data = Vec::new();
    let mut assoc_entries: Vec<(u16, u16)> = Vec::new();
    for (ga, asap) in &ga_asap_pairs {
        let tsap = unique_gas
            .iter()
            .position(|g| g == ga)
            .map(|i| (i + 1) as u16);
        if let Some(tsap) = tsap {
            assoc_entries.push((tsap, *asap));
        }
    }
    let assoc_count = assoc_entries.len() as u16;
    assoc_data.extend_from_slice(&assoc_count.to_be_bytes());
    for (tsap, asap) in &assoc_entries {
        assoc_data.extend_from_slice(&tsap.to_be_bytes());
        assoc_data.extend_from_slice(&asap.to_be_bytes());
    }
    bau.association_table.load(&assoc_data);

    tracing::info!(
        gas = unique_gas.len(),
        associations = assoc_entries.len(),
        "KNX address/association tables loaded from TOML"
    );
}

/// Handle a write from the publisher: encode value, write to GO, poll BAU, send frames.
async fn handle_write(
    bau: &mut Bau,
    server: &DeviceServer,
    ga: GroupAddress,
    _dpt: Dpt,
    value: &DptValue,
) {
    // Find the ASAP for this GA via the address table
    let ga_raw = ga.raw();
    let Some(tsap) = bau.address_table.get_tsap(ga_raw) else {
        // GA not in address table — can't map to a GO
        tracing::trace!(ga = %ga, "GA not in address table, skipping write");
        return;
    };

    // Find associated GO and write value
    for asap in bau.association_table.asaps_for_tsap(tsap) {
        if let Some(go) = bau.group_objects.get_mut(asap) {
            let _ = go.set_value_if_changed(value);
        }
    }

    // Poll to send pending writes
    bau.poll();
    while let Some(out) = bau.next_outgoing_frame() {
        let _ = server.send_frame(out).await;
    }
}

/// Check all GOs for Updated flag and forward to the listener channel.
async fn dispatch_updated_gos(bau: &mut Bau, update_tx: &mpsc::Sender<(GroupAddress, Vec<u8>)>) {
    while let Some(asap) = bau.group_objects.next_updated() {
        if let Some(result) = resolve_go_update(bau, asap) {
            let _ = update_tx.send(result).await;
        }
        // Acknowledge the update
        if let Some(go) = bau.group_objects.get_mut(asap) {
            go.set_comm_flag(knx_device::group_object::ComFlag::Ok);
        }
    }
}

/// Find the next updated GO and return its GA + data.
fn find_next_updated(bau: &mut Bau) -> Option<(GroupAddress, Vec<u8>)> {
    let asap = bau.group_objects.next_updated()?;
    let result = resolve_go_update(bau, asap);
    if let Some(go) = bau.group_objects.get_mut(asap) {
        go.set_comm_flag(knx_device::group_object::ComFlag::Ok);
    }
    result
}

/// Resolve an ASAP to (GroupAddress, data) via the association and address tables.
fn resolve_go_update(bau: &Bau, asap: u16) -> Option<(GroupAddress, Vec<u8>)> {
    let go = bau.group_objects.get(asap)?;
    let data = go.value_ref().to_vec();
    let tsap = bau.association_table.translate_asap(asap)?;
    let ga_raw = bau.address_table.get_group_address(tsap)?;
    Some((GroupAddress::from_raw(ga_raw), data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ets_memory_defaults() {
        let data = vec![0u8; mem::TOTAL];
        let p = parse_ets_memory(&data);
        assert!(!p.zone_active[0]);
        assert_eq!(p.zone_max_volume[0], 0);
        assert!(!p.client_active[0]);
    }

    #[test]
    fn parse_ets_memory_values() {
        let mut data = vec![0u8; mem::TOTAL];
        data[mem::ZONE_ACTIVE] = 1;
        data[mem::ZONE_ACTIVE + 2] = 1;
        data[mem::ZONE_MAX_VOL + 0] = 80;
        data[mem::CLIENT_ACTIVE + 0] = 1;
        data[mem::CLIENT_MAX_VOL + 0] = 60;
        data[mem::CLIENT_DEF_ZONE + 0] = 3;
        data[mem::CLIENT_DEF_LAT + 0] = 50;
        let p = parse_ets_memory(&data);
        assert!(p.zone_active[0]);
        assert!(!p.zone_active[1]);
        assert!(p.zone_active[2]);
        assert_eq!(p.zone_max_volume[0], 80);
        assert!(p.client_active[0]);
        assert_eq!(p.client_max_volume[0], 60);
        assert_eq!(p.client_default_zone[0], 3);
        assert_eq!(p.client_default_latency[0], 50);
    }

    #[test]
    fn parse_ets_memory_too_short() {
        let data = vec![0u8; 10]; // too short
        let p = parse_ets_memory(&data);
        // Should return defaults without panic
        assert!(!p.zone_active[0]);
    }

    #[test]
    fn build_tables_from_minimal_config() {
        let raw: crate::config::RawConfig = toml::from_str(
            r#"
            [[zone]]
            name = "Test"
            [zone.knx]
            play = "1/0/1"
            volume = "1/0/2"
            volume_status = "1/0/3"

            [[client]]
            name = "Speaker"
            mac = "00:00:00:00:00:00"
            zone = "Test"
            [client.knx]
            volume = "2/0/1"
            mute = "2/0/2"
        "#,
        )
        .unwrap();
        let config = crate::config::load_raw(raw).unwrap();
        let ia = IndividualAddress::from_raw(0x1101);
        let bau = build_bau(ia, &config);

        // Should have 5 unique GAs
        assert_eq!(bau.address_table.entry_count(), 5);

        // Zone 1 Play (ASAP 1) should be mapped to GA 1/0/1
        let tsap = bau
            .address_table
            .get_tsap(GroupAddress::from_str("1/0/1").unwrap().raw());
        assert!(tsap.is_some());

        // Client 1 Volume (ASAP 351) should be mapped to GA 2/0/1
        let tsap = bau
            .address_table
            .get_tsap(GroupAddress::from_str("2/0/1").unwrap().raw());
        assert!(tsap.is_some());
    }
}
