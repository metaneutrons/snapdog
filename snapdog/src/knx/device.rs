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

use super::group_objects::{self, CLIENT_GOS, MAX_CLIENTS, MAX_ZONES, TOTAL_GO_COUNT, ZONE_GOS};
use super::transport::KnxTransport;

// ── ETS Memory Layout (must match xtask/src/main.rs) ──────────

/// Byte offsets for ETS parameters in BAU memory.
/// Layout: zone active(10) + zone defvol(10) + zone maxvol(10) + zone airplay(10) +
///         zone spotify(10) + zone presence_en(10) + zone presence_to(20) +
///         zone srate(10) + zone bitd(10) + client active(10) + client defzone(10) +
///         client defvol(10) + client maxvol(10) + client deflat(10) +
///         global httpport(2) + global loglvl(1) + radio active(20)
mod mem {
    pub const ZONE_ACTIVE: usize = 0; // 10 × 1 byte
    pub const ZONE_DEF_VOL: usize = 10; // 10 × 1 byte
    pub const ZONE_MAX_VOL: usize = 20; // 10 × 1 byte
    pub const ZONE_AIRPLAY: usize = 30; // 10 × 1 byte
    pub const ZONE_SPOTIFY: usize = 40; // 10 × 1 byte
    pub const ZONE_PRESENCE_EN: usize = 50; // 10 × 1 byte
    pub const ZONE_PRESENCE_TO: usize = 60; // 10 × 2 bytes
    pub const ZONE_SRATE: usize = 80; // 10 × 1 byte
    pub const ZONE_BITD: usize = 90; // 10 × 1 byte
    pub const CLIENT_ACTIVE: usize = 100; // 10 × 1 byte
    pub const CLIENT_DEF_ZONE: usize = 110; // 10 × 1 byte
    pub const CLIENT_DEF_VOL: usize = 120; // 10 × 1 byte
    pub const CLIENT_MAX_VOL: usize = 130; // 10 × 1 byte
    pub const CLIENT_DEF_LAT: usize = 140; // 10 × 1 byte
    pub const GLOBAL_HTTP_PORT: usize = 150; // 2 bytes
    pub const GLOBAL_LOG_LVL: usize = 152; // 1 byte
    pub const RADIO_ACTIVE: usize = 153; // 20 × 1 byte
    pub const TOTAL: usize = 173;
}

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

    tokio::spawn(bau_task(ia, server, cmd_rx, update_tx, persist_path));

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
    mut server: DeviceServer,
    mut cmd_rx: mpsc::Receiver<BauCmd>,
    update_tx: mpsc::Sender<(GroupAddress, Vec<u8>)>,
    persist_path: Option<PathBuf>,
) {
    let mut bau = build_bau(ia);

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

/// Build a BAU with 410 group objects configured with correct DPTs.
fn build_bau(ia: IndividualAddress) -> Bau {
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

    bau
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
