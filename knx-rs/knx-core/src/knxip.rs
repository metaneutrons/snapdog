// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! KNXnet/IP frame types.
//!
//! Provides parsing and construction of KNXnet/IP protocol frames used for
//! IP-based KNX communication (tunneling, routing, discovery).
//!
//! # Wire Layout
//!
//! ```text
//! Offset  Field              Size
//! ──────  ─────              ────
//!   0     Header Length       1 byte  (always 0x06)
//!   1     Protocol Version    1 byte  (always 0x10)
//!   2     Service Type        2 bytes (big-endian)
//!   4     Total Length         2 bytes (big-endian, includes header)
//!   6     Body                variable
//! ```

use alloc::vec::Vec;
use core::fmt;

/// KNXnet/IP header length (always 6 bytes).
pub const HEADER_LEN: u8 = 0x06;

/// KNXnet/IP protocol version 1.0.
pub const PROTOCOL_VERSION_10: u8 = 0x10;

/// KNXnet/IP service type identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ServiceType {
    /// Search request.
    SearchRequest = 0x0201,
    /// Search response.
    SearchResponse = 0x0202,
    /// Description request.
    DescriptionRequest = 0x0203,
    /// Description response.
    DescriptionResponse = 0x0204,
    /// Connect request.
    ConnectRequest = 0x0205,
    /// Connect response.
    ConnectResponse = 0x0206,
    /// Connection state request.
    ConnectionStateRequest = 0x0207,
    /// Connection state response.
    ConnectionStateResponse = 0x0208,
    /// Disconnect request.
    DisconnectRequest = 0x0209,
    /// Disconnect response.
    DisconnectResponse = 0x020A,
    /// Extended search request.
    SearchRequestExtended = 0x020B,
    /// Extended search response.
    SearchResponseExtended = 0x020C,
    /// Device configuration request.
    DeviceConfigurationRequest = 0x0310,
    /// Device configuration acknowledgement.
    DeviceConfigurationAck = 0x0311,
    /// Tunneling request.
    TunnelingRequest = 0x0420,
    /// Tunneling acknowledgement.
    TunnelingAck = 0x0421,
    /// Routing indication (multicast).
    RoutingIndication = 0x0530,
    /// Routing lost message.
    RoutingLostMessage = 0x0531,
}

impl ServiceType {
    /// Try to convert a raw `u16` to a `ServiceType`.
    pub const fn from_raw(raw: u16) -> Option<Self> {
        Some(match raw {
            0x0201 => Self::SearchRequest,
            0x0202 => Self::SearchResponse,
            0x0203 => Self::DescriptionRequest,
            0x0204 => Self::DescriptionResponse,
            0x0205 => Self::ConnectRequest,
            0x0206 => Self::ConnectResponse,
            0x0207 => Self::ConnectionStateRequest,
            0x0208 => Self::ConnectionStateResponse,
            0x0209 => Self::DisconnectRequest,
            0x020A => Self::DisconnectResponse,
            0x020B => Self::SearchRequestExtended,
            0x020C => Self::SearchResponseExtended,
            0x0310 => Self::DeviceConfigurationRequest,
            0x0311 => Self::DeviceConfigurationAck,
            0x0420 => Self::TunnelingRequest,
            0x0421 => Self::TunnelingAck,
            0x0530 => Self::RoutingIndication,
            0x0531 => Self::RoutingLostMessage,
            _ => return None,
        })
    }
}

/// Host protocol code for HPAI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum HostProtocol {
    /// IPv4 over UDP.
    Ipv4Udp = 0x01,
    /// IPv4 over TCP.
    Ipv4Tcp = 0x02,
}

/// Error returned when parsing a KNXnet/IP frame fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnxIpError {
    /// Frame is shorter than the header.
    TooShort,
    /// Header length field is not 0x06.
    InvalidHeaderLength,
    /// Protocol version is not 0x10.
    InvalidProtocolVersion,
    /// Total length does not match actual data.
    LengthMismatch,
    /// Unknown service type.
    UnknownServiceType(u16),
}

impl fmt::Display for KnxIpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => f.write_str("KNXnet/IP frame too short"),
            Self::InvalidHeaderLength => f.write_str("invalid KNXnet/IP header length"),
            Self::InvalidProtocolVersion => f.write_str("invalid KNXnet/IP protocol version"),
            Self::LengthMismatch => f.write_str("KNXnet/IP frame length mismatch"),
            Self::UnknownServiceType(st) => write!(f, "unknown KNXnet/IP service type: {st:#06x}"),
        }
    }
}

impl core::error::Error for KnxIpError {}

/// A parsed KNXnet/IP frame header + body.
#[derive(Clone, PartialEq, Eq)]
pub struct KnxIpFrame {
    /// The service type.
    pub service_type: ServiceType,
    /// The frame body (after the 6-byte header).
    pub body: Vec<u8>,
}

impl KnxIpFrame {
    /// Parse a KNXnet/IP frame from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns [`KnxIpError`] if the frame is malformed.
    pub fn parse(data: &[u8]) -> Result<Self, KnxIpError> {
        if data.len() < HEADER_LEN as usize {
            return Err(KnxIpError::TooShort);
        }
        if data[0] != HEADER_LEN {
            return Err(KnxIpError::InvalidHeaderLength);
        }
        if data[1] != PROTOCOL_VERSION_10 {
            return Err(KnxIpError::InvalidProtocolVersion);
        }

        let service_raw = u16::from_be_bytes([data[2], data[3]]);
        let total_len = u16::from_be_bytes([data[4], data[5]]) as usize;

        if data.len() < total_len {
            return Err(KnxIpError::LengthMismatch);
        }

        let service_type = ServiceType::from_raw(service_raw)
            .ok_or(KnxIpError::UnknownServiceType(service_raw))?;

        Ok(Self {
            service_type,
            body: data[HEADER_LEN as usize..total_len].to_vec(),
        })
    }

    /// Serialize the frame to bytes (header + body).
    pub fn to_bytes(&self) -> Vec<u8> {
        let total_len = HEADER_LEN as usize + self.body.len();
        let mut buf = Vec::with_capacity(total_len);
        buf.push(HEADER_LEN);
        buf.push(PROTOCOL_VERSION_10);
        buf.extend_from_slice(&(self.service_type as u16).to_be_bytes());
        #[expect(clippy::cast_possible_truncation)]
        let len_bytes = (total_len as u16).to_be_bytes();
        buf.extend_from_slice(&len_bytes);
        buf.extend_from_slice(&self.body);
        buf
    }
}

impl fmt::Debug for KnxIpFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KnxIpFrame")
            .field("service_type", &self.service_type)
            .field("body_len", &self.body.len())
            .finish()
    }
}

/// KNXnet/IP Connection Header (4 bytes).
///
/// Used in tunneling request/ack frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionHeader {
    /// Channel ID.
    pub channel_id: u8,
    /// Sequence counter.
    pub sequence_counter: u8,
    /// Status code.
    pub status: u8,
}

impl ConnectionHeader {
    /// Header length on the wire (always 4).
    pub const LEN: u8 = 4;

    /// Parse from a 4-byte slice.
    pub const fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < Self::LEN as usize {
            return None;
        }
        Some(Self {
            channel_id: data[1],
            sequence_counter: data[2],
            status: data[3],
        })
    }

    /// Serialize to 4 bytes.
    pub const fn to_bytes(self) -> [u8; 4] {
        [
            Self::LEN,
            self.channel_id,
            self.sequence_counter,
            self.status,
        ]
    }
}

/// Host Protocol Address Information (HPAI) — 8 bytes.
///
/// Identifies an IP endpoint (address + port).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hpai {
    /// Host protocol (UDP or TCP).
    pub protocol: HostProtocol,
    /// IPv4 address as 4 bytes.
    pub ip: [u8; 4],
    /// Port number.
    pub port: u16,
}

impl Hpai {
    /// HPAI length on the wire (always 8).
    pub const LEN: u8 = 8;

    /// Parse from an 8-byte slice.
    pub const fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < Self::LEN as usize || data[0] != Self::LEN {
            return None;
        }
        let protocol = match data[1] {
            0x01 => HostProtocol::Ipv4Udp,
            0x02 => HostProtocol::Ipv4Tcp,
            _ => return None,
        };
        Some(Self {
            protocol,
            ip: [data[2], data[3], data[4], data[5]],
            port: u16::from_be_bytes([data[6], data[7]]),
        })
    }

    /// Serialize to 8 bytes.
    pub const fn to_bytes(self) -> [u8; 8] {
        let port = self.port.to_be_bytes();
        [
            Self::LEN,
            self.protocol as u8,
            self.ip[0],
            self.ip[1],
            self.ip[2],
            self.ip[3],
            port[0],
            port[1],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_routing_indication() {
        // KNXnet/IP header + CEMI frame (routing indication)
        let mut frame_data = Vec::new();
        frame_data.push(0x06); // header length
        frame_data.push(0x10); // protocol version
        frame_data.extend_from_slice(&0x0530u16.to_be_bytes()); // RoutingIndication
        let cemi = [
            0x29, 0x00, 0xBC, 0xE0, 0x11, 0x01, 0x08, 0x01, 0x01, 0x00, 0x81,
        ];
        let total_len = u16::from(HEADER_LEN) + cemi.len() as u16;
        frame_data.extend_from_slice(&total_len.to_be_bytes());
        frame_data.extend_from_slice(&cemi);

        let frame = KnxIpFrame::parse(&frame_data).unwrap();
        assert_eq!(frame.service_type, ServiceType::RoutingIndication);
        assert_eq!(frame.body, cemi);
    }

    #[test]
    fn roundtrip_tunneling_request() {
        let cemi = [
            0x29, 0x00, 0xBC, 0xE0, 0x11, 0x01, 0x08, 0x01, 0x01, 0x00, 0x81,
        ];
        let ch = ConnectionHeader {
            channel_id: 1,
            sequence_counter: 5,
            status: 0,
        };

        let mut body = Vec::new();
        body.extend_from_slice(&ch.to_bytes());
        body.extend_from_slice(&cemi);

        let frame = KnxIpFrame {
            service_type: ServiceType::TunnelingRequest,
            body,
        };

        let bytes = frame.to_bytes();
        let reparsed = KnxIpFrame::parse(&bytes).unwrap();
        assert_eq!(reparsed.service_type, ServiceType::TunnelingRequest);

        let ch2 = ConnectionHeader::parse(&reparsed.body).unwrap();
        assert_eq!(ch2.channel_id, 1);
        assert_eq!(ch2.sequence_counter, 5);
        assert_eq!(ch2.status, 0);
    }

    #[test]
    fn roundtrip_tunneling_ack() {
        let ch = ConnectionHeader {
            channel_id: 1,
            sequence_counter: 5,
            status: 0,
        };
        let frame = KnxIpFrame {
            service_type: ServiceType::TunnelingAck,
            body: ch.to_bytes().to_vec(),
        };
        let bytes = frame.to_bytes();
        assert_eq!(
            bytes.len(),
            HEADER_LEN as usize + ConnectionHeader::LEN as usize
        );

        let reparsed = KnxIpFrame::parse(&bytes).unwrap();
        assert_eq!(reparsed.service_type, ServiceType::TunnelingAck);
    }

    #[test]
    fn parse_hpai() {
        let data = [0x08, 0x01, 192, 168, 1, 50, 0x0E, 0x57]; // UDP, 192.168.1.50:3671
        let hpai = Hpai::parse(&data).unwrap();
        assert_eq!(hpai.protocol, HostProtocol::Ipv4Udp);
        assert_eq!(hpai.ip, [192, 168, 1, 50]);
        assert_eq!(hpai.port, 3671);
        assert_eq!(hpai.to_bytes(), data);
    }

    #[test]
    fn parse_too_short() {
        assert!(KnxIpFrame::parse(&[0x06, 0x10]).is_err());
    }

    #[test]
    fn parse_bad_header_length() {
        let data = [0x05, 0x10, 0x05, 0x30, 0x00, 0x06];
        assert!(matches!(
            KnxIpFrame::parse(&data),
            Err(KnxIpError::InvalidHeaderLength)
        ));
    }

    #[test]
    fn parse_bad_version() {
        let data = [0x06, 0x11, 0x05, 0x30, 0x00, 0x06];
        assert!(matches!(
            KnxIpFrame::parse(&data),
            Err(KnxIpError::InvalidProtocolVersion)
        ));
    }

    #[test]
    fn parse_unknown_service() {
        let data = [0x06, 0x10, 0xFF, 0xFF, 0x00, 0x06];
        assert!(matches!(
            KnxIpFrame::parse(&data),
            Err(KnxIpError::UnknownServiceType(0xFFFF))
        ));
    }

    #[test]
    fn connection_header_roundtrip() {
        let ch = ConnectionHeader {
            channel_id: 42,
            sequence_counter: 7,
            status: 0,
        };
        let bytes = ch.to_bytes();
        assert_eq!(bytes, [4, 42, 7, 0]);
        let parsed = ConnectionHeader::parse(&bytes).unwrap();
        assert_eq!(parsed, ch);
    }
}
