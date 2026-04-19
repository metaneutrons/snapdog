// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! Device-mode KNX transport — runs as ETS-programmable KNX/IP device.
//!
//! Wraps `DeviceServer` + `Bau`. The BAU runs in a dedicated task;
//! the transport communicates via channels.

use std::net::Ipv4Addr;
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

/// Start the device server and BAU, returning a publisher/listener pair.
pub(crate) async fn start_device_transport(
    individual_address: &str,
    _config: &crate::config::AppConfig,
) -> Result<(DevicePublisher, DeviceListener)> {
    let ia = IndividualAddress::from_str(individual_address)
        .map_err(|e| anyhow::anyhow!("Invalid individual address: {e}"))?;

    let server = DeviceServer::start(Ipv4Addr::UNSPECIFIED)
        .await
        .context("Failed to start KNX device server")?;

    let (cmd_tx, cmd_rx) = mpsc::channel(64);
    let (update_tx, update_rx) = mpsc::channel(64);

    tokio::spawn(bau_task(ia, server, cmd_rx, update_tx));

    tracing::info!(%individual_address, "KNX device mode started");

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
) {
    let mut bau = build_bau(ia);

    loop {
        tokio::select! {
            // Incoming frames from the network (ETS or multicast)
            event = server.recv() => {
                let Some(event) = event else { break };
                let frame = match event {
                    ServerEvent::TunnelFrame(f) | ServerEvent::RoutingFrame(f) => f,
                };
                bau.process_frame(&frame);
                bau.poll();

                // Send outgoing frames (responses to ETS, group value responses)
                while let Some(out) = bau.next_outgoing_frame() {
                    let _ = server.send_frame(out).await;
                }

                // Check for GOs updated from the bus → forward to listener
                dispatch_updated_gos(&mut bau, &update_tx).await;
            }

            // Commands from the publisher (write values to GOs)
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
