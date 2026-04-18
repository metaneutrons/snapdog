// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! Common External Message Interface (cEMI) frame parsing and serialization.
//!
//! A cEMI frame is the standard encoding for KNX telegrams across all media.
//!
//! # Wire Layout
//!
//! ```text
//! Offset  Field              Size
//! ──────  ─────              ────
//!   0     Message Code       1 byte
//!   1     Additional Info Len 1 byte  (usually 0)
//!   2+N   Control Field 1    1 byte
//!   3+N   Control Field 2    1 byte
//!   4+N   Source Address     2 bytes (big-endian)
//!   6+N   Destination Addr   2 bytes (big-endian)
//!   8+N   NPDU Length        1 byte  (octet count of TPDU/APDU)
//!   9+N   TPDU/APDU data    variable
//! ```
//!
//! Where N = additional info length.

use alloc::vec::Vec;
use core::fmt;

use crate::address::{DestinationAddress, GroupAddress, IndividualAddress};
use crate::message::MessageCode;
use crate::tpdu::Tpdu;
use crate::types::{
    AckType, AddressType, Confirm, FrameFormat, Priority, Repetition, SystemBroadcast,
};

/// Minimum cEMI frame size: msg code + add info len + ctrl1 + ctrl2 + src(2) + dst(2) + npdu len.
const MIN_FRAME_SIZE: usize = 9;

/// Offset from start of control fields to NPDU length byte.
const NPDU_LEN_OFFSET: usize = 6;

/// Offset from start of control fields to TPDU/APDU data.
const TPDU_OFFSET: usize = 7;

/// Error returned when parsing a cEMI frame fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CemiError {
    /// Frame is shorter than the minimum required size.
    TooShort,
    /// The declared length does not match the actual data.
    LengthMismatch,
    /// Unknown or unsupported message code.
    UnknownMessageCode(u8),
    /// Control field contains invalid bit combinations.
    InvalidControlField,
}

impl fmt::Display for CemiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => f.write_str("cEMI frame too short"),
            Self::LengthMismatch => f.write_str("cEMI frame length mismatch"),
            Self::UnknownMessageCode(c) => write!(f, "unknown cEMI message code: {c:#04x}"),
            Self::InvalidControlField => f.write_str("invalid cEMI control field"),
        }
    }
}

impl core::error::Error for CemiError {}

/// A parsed cEMI frame providing typed access to all fields.
///
/// This is an owned representation — the raw bytes are copied into an internal
/// buffer on construction. Field accessors decode directly from the buffer,
/// matching the C++ reference implementation's approach.
#[derive(Clone, PartialEq, Eq)]
pub struct CemiFrame {
    /// Raw frame bytes (message code through end of APDU).
    data: Vec<u8>,
    /// Offset to control field 1 (after message code + add info).
    ctrl_offset: usize,
}

impl CemiFrame {
    /// Parse a cEMI frame from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns [`CemiError`] if the data is too short, has a length mismatch,
    /// or contains invalid fields.
    pub fn parse(data: &[u8]) -> Result<Self, CemiError> {
        if data.len() < 2 {
            return Err(CemiError::TooShort);
        }

        let add_info_len = data[1] as usize;
        let ctrl_offset = 2 + add_info_len;

        // Need at least: header(2) + add_info(N) + ctrl1 + ctrl2 + src(2) + dst(2) + npdu_len
        if data.len() < ctrl_offset + 7 {
            return Err(CemiError::TooShort);
        }

        let npdu_octet_count = data[ctrl_offset + NPDU_LEN_OFFSET] as usize;
        // TPDU is always at least 2 bytes (TPCI + APCI), even when npdu_octet_count is 0
        let payload_len = (npdu_octet_count + 1).max(2);
        let expected_len = ctrl_offset + TPDU_OFFSET + payload_len;

        // Allow frames that are exactly the expected length or longer (trailing data ignored)
        if data.len() < expected_len {
            return Err(CemiError::LengthMismatch);
        }

        Ok(Self {
            data: data[..expected_len].to_vec(),
            ctrl_offset,
        })
    }

    /// Create a new cEMI frame for an `L_Data` indication/request with the given APDU payload.
    ///
    /// The frame is initialized with broadcast system broadcast, no additional info.
    pub fn new_l_data(
        message_code: MessageCode,
        source: IndividualAddress,
        destination: DestinationAddress,
        priority: Priority,
        payload: &[u8],
    ) -> Self {
        let npdu_octet_count = if payload.is_empty() {
            0
        } else {
            payload.len() - 1
        };
        let total_len = MIN_FRAME_SIZE + 1 + npdu_octet_count;
        let mut data = alloc::vec![0u8; total_len];

        data[0] = message_code as u8;
        data[1] = 0; // no additional info

        let ctrl_offset = 2;

        // Control field 1: standard frame, broadcast, given priority
        data[ctrl_offset] = FrameFormat::Standard as u8
            | Repetition::WasNotRepeated as u8
            | SystemBroadcast::Broadcast as u8
            | priority as u8;

        // Control field 2: address type + hop count 6
        let addr_type = match destination {
            DestinationAddress::Group(_) => AddressType::Group,
            DestinationAddress::Individual(_) => AddressType::Individual,
        };
        data[ctrl_offset + 1] = addr_type as u8 | (6 << 4);

        // Source address
        let src_bytes = source.to_bytes();
        data[ctrl_offset + 2] = src_bytes[0];
        data[ctrl_offset + 3] = src_bytes[1];

        // Destination address
        let dst_raw = match destination {
            DestinationAddress::Group(ga) => ga.raw(),
            DestinationAddress::Individual(ia) => ia.raw(),
        };
        let dst_bytes = dst_raw.to_be_bytes();
        data[ctrl_offset + 4] = dst_bytes[0];
        data[ctrl_offset + 5] = dst_bytes[1];

        // NPDU length
        #[expect(clippy::cast_possible_truncation)]
        {
            data[ctrl_offset + NPDU_LEN_OFFSET] = npdu_octet_count as u8;
        }

        // TPDU/APDU payload
        if !payload.is_empty() {
            data[ctrl_offset + TPDU_OFFSET..][..payload.len()].copy_from_slice(payload);
        }

        Self { data, ctrl_offset }
    }

    // ── Field accessors ───────────────────────────────────────

    /// The raw message code byte.
    pub fn message_code_raw(&self) -> u8 {
        self.data[0]
    }

    /// Additional info length.
    pub fn additional_info_length(&self) -> u8 {
        self.data[1]
    }

    /// Control field 1 (raw byte).
    fn ctrl1(&self) -> u8 {
        self.data[self.ctrl_offset]
    }

    /// Control field 2 (raw byte).
    fn ctrl2(&self) -> u8 {
        self.data[self.ctrl_offset + 1]
    }

    /// Frame format (standard or extended).
    pub fn frame_type(&self) -> FrameFormat {
        if self.ctrl1() & 0x80 != 0 {
            FrameFormat::Standard
        } else {
            FrameFormat::Extended
        }
    }

    /// Repetition flag.
    pub fn repetition(&self) -> Repetition {
        if self.ctrl1() & 0x20 != 0 {
            Repetition::WasNotRepeated
        } else {
            Repetition::WasRepeated
        }
    }

    /// System broadcast flag.
    pub fn system_broadcast(&self) -> SystemBroadcast {
        if self.ctrl1() & 0x10 != 0 {
            SystemBroadcast::Broadcast
        } else {
            SystemBroadcast::System
        }
    }

    /// Telegram priority.
    pub fn priority(&self) -> Priority {
        match self.ctrl1() & 0x0C {
            0x00 => Priority::System,
            0x04 => Priority::Normal,
            0x08 => Priority::Urgent,
            _ => Priority::Low,
        }
    }

    /// Acknowledgement request flag.
    pub fn ack(&self) -> AckType {
        if self.ctrl1() & 0x02 != 0 {
            AckType::Requested
        } else {
            AckType::DontCare
        }
    }

    /// Confirmation flag.
    pub fn confirm(&self) -> Confirm {
        if self.ctrl1() & 0x01 != 0 {
            Confirm::Error
        } else {
            Confirm::NoError
        }
    }

    /// Destination address type.
    pub fn address_type(&self) -> AddressType {
        if self.ctrl2() & 0x80 != 0 {
            AddressType::Group
        } else {
            AddressType::Individual
        }
    }

    /// Hop count (0–7).
    pub fn hop_count(&self) -> u8 {
        (self.ctrl2() >> 4) & 0x07
    }

    /// Source individual address.
    pub fn source_address(&self) -> IndividualAddress {
        let off = self.ctrl_offset + 2;
        IndividualAddress::from_bytes([self.data[off], self.data[off + 1]])
    }

    /// Raw destination address as 16-bit value.
    pub fn destination_address_raw(&self) -> u16 {
        let off = self.ctrl_offset + 4;
        u16::from_be_bytes([self.data[off], self.data[off + 1]])
    }

    /// Typed destination address (group or individual based on address type flag).
    pub fn destination_address(&self) -> DestinationAddress {
        let raw = self.destination_address_raw();
        match self.address_type() {
            AddressType::Group => DestinationAddress::Group(GroupAddress::from_raw(raw)),
            AddressType::Individual => {
                DestinationAddress::Individual(IndividualAddress::from_raw(raw))
            }
        }
    }

    /// NPDU octet count (length of TPDU/APDU data minus 1 for the TPCI byte).
    pub fn npdu_length(&self) -> u8 {
        self.data[self.ctrl_offset + NPDU_LEN_OFFSET]
    }

    /// The TPDU/APDU payload bytes (starting with the TPCI byte).
    pub fn payload(&self) -> &[u8] {
        let start = self.ctrl_offset + TPDU_OFFSET;
        &self.data[start..]
    }

    /// The complete raw frame bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Total frame length in bytes.
    pub fn total_length(&self) -> usize {
        self.data.len()
    }

    /// Calculate TP CRC over a buffer (XOR of all bytes, inverted).
    pub fn calc_crc_tp(buffer: &[u8]) -> u8 {
        let mut crc: u8 = 0xFF;
        for &b in buffer {
            crc ^= b;
        }
        crc
    }

    /// Parse the TPDU from this frame's payload.
    ///
    /// Returns `None` if the payload cannot be parsed as a valid TPDU.
    pub fn tpdu(&self) -> Option<Tpdu> {
        Tpdu::parse(
            self.payload(),
            self.npdu_length(),
            self.address_type(),
            self.destination_address_raw(),
        )
    }
}

impl fmt::Debug for CemiFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CemiFrame")
            .field("message_code", &self.message_code_raw())
            .field("frame_type", &self.frame_type())
            .field("priority", &self.priority())
            .field("source", &self.source_address())
            .field("destination", &self.destination_address())
            .field("hop_count", &self.hop_count())
            .field("payload_len", &self.npdu_length())
            .finish()
    }
}
