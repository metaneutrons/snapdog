// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! Client-mode KNX transport — connects to an external KNX/IP gateway.

use knx_core::address::GroupAddress;
use knx_core::dpt::{Dpt, DptValue};
use knx_ip::multiplex::MultiplexHandle;
use knx_ip::ops::GroupOps;

use super::transport::KnxTransport;

/// KNX transport backed by a `MultiplexHandle` (tunnel or router client).
pub(crate) struct ClientTransport {
    handle: MultiplexHandle,
}

impl ClientTransport {
    pub(crate) fn new(handle: MultiplexHandle) -> Self {
        Self { handle }
    }
}

impl KnxTransport for ClientTransport {
    async fn write(&self, ga: GroupAddress, dpt: Dpt, value: &DptValue) {
        if let Err(e) = self.handle.group_write_value(ga, dpt, value).await {
            tracing::warn!(ga = %ga, error = %e, "KNX write failed");
        }
    }

    async fn recv_group_write(&mut self) -> Option<(GroupAddress, Vec<u8>)> {
        loop {
            let cemi = self.handle.recv().await?;
            if let Some(result) = cemi.as_group_write() {
                return Some(result);
            }
        }
    }
}
