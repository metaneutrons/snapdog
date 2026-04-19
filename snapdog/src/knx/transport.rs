// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! KNX transport abstraction — shared interface for client and device modes.

use knx_core::address::GroupAddress;
use knx_core::dpt::{Dpt, DptValue};

/// Abstraction over the KNX bus transport.
///
/// - **Client mode**: wraps `MultiplexHandle` + `GroupOps`
/// - **Device mode**: wraps `DeviceServer` + `Bau` + `GroupObjectStore`
pub(crate) trait KnxTransport: Send + Sync + 'static {
    /// Write a typed value to a group address.
    fn write(
        &self,
        ga: GroupAddress,
        dpt: Dpt,
        value: &DptValue,
    ) -> impl std::future::Future<Output = ()> + Send;

    /// Receive the next group write from the bus.
    /// Returns `None` if the connection is closed.
    fn recv_group_write(
        &mut self,
    ) -> impl std::future::Future<Output = Option<(GroupAddress, Vec<u8>)>> + Send;
}
