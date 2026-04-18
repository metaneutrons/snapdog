// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! KNX protocol enums and types.
//!
//! These types map directly to the KNX specification and the C++ reference
//! implementation in `knx_types.h`. Wire values are preserved as discriminants
//! where applicable.

/// CEMI frame format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FrameFormat {
    /// Extended frame (long format).
    Extended = 0x00,
    /// Standard frame (short format).
    Standard = 0x80,
}

/// Telegram priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Priority {
    /// Mainly used by ETS for device programming.
    System = 0x00,
    /// More important telegrams like central functions.
    Normal = 0x04,
    /// Used for alarms.
    Urgent = 0x08,
    /// Normal priority of group communication.
    Low = 0x0C,
}

/// Data link layer acknowledgement request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AckType {
    /// No acknowledgement requested.
    DontCare = 0x00,
    /// Acknowledgement requested.
    Requested = 0x02,
}

/// TP-UART acknowledgement type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TpAckType {
    /// No acknowledgement.
    None = 0x00,
    /// Positive acknowledgement.
    Ack = 0x01,
    /// Busy (receiver cannot process).
    Busy = 0x02,
    /// Negative acknowledgement.
    Nack = 0x04,
}

/// Address type flag in CEMI control field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AddressType {
    /// Individual (physical) address.
    Individual = 0x00,
    /// Group address.
    Group = 0x80,
}

/// Repetition flag in CEMI control field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Repetition {
    /// Frame was repeated / no repetition allowed.
    WasRepeated = 0x00,
    /// Frame was not repeated / repetition allowed.
    WasNotRepeated = 0x20,
}

/// System broadcast flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SystemBroadcast {
    /// System broadcast (domain-wide).
    System = 0x00,
    /// Normal broadcast.
    Broadcast = 0x10,
}

/// Confirmation flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Confirm {
    /// No error.
    NoError = 0x00,
    /// Error occurred.
    Error = 0x01,
}

/// Hop count handling strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HopCountType {
    /// Hop count set to 7 (frame never expires).
    UnlimitedRouting,
    /// Use network layer parameter as hop count.
    NetworkLayerParameter,
}

/// KNX medium type (DPT 20.1004).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DptMedium {
    /// Twisted pair (TP1).
    Tp1 = 0x00,
    /// Powerline (PL110).
    Pl110 = 0x01,
    /// Radio frequency.
    Rf = 0x02,
    /// IP.
    Ip = 0x05,
}
