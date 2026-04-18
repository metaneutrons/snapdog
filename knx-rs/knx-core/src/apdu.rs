// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! Application Protocol Data Unit (APDU).
//!
//! The APDU carries the application-layer service type and data.
//! It is encoded in the TPDU payload, starting at the TPCI/APCI bytes.
//!
//! # Wire encoding
//!
//! The first two bytes of the TPDU data contain the TPCI and APCI:
//!
//! ```text
//! Byte 0: [TPCI bits 7..2] [APCI bits 9..8]
//! Byte 1: [APCI bits 7..0]
//! ```
//!
//! For "short" APCIs (group value read/response/write), the lower 6 bits
//! of byte 1 carry small data values directly.

use alloc::vec::Vec;

use crate::message::ApduType;

/// A parsed Application Protocol Data Unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Apdu {
    /// The APDU service type.
    pub apdu_type: ApduType,
    /// The APDU data bytes (excluding the APCI encoding).
    ///
    /// For short APDUs (e.g. `GroupValueWrite` with ≤6 bits), this contains
    /// the small value in `data[0] & 0x3F`. For longer APDUs, this is the
    /// payload starting after the 2-byte APCI header.
    pub data: Vec<u8>,
}

impl Apdu {
    /// Parse an APDU from raw TPDU payload bytes.
    ///
    /// `payload` starts at the TPCI byte (first byte of the TPDU data).
    /// `npdu_length` is the octet count from the CEMI frame.
    ///
    /// # Errors
    ///
    /// Returns `None` if the payload is too short or the APCI is unrecognized.
    pub fn parse(payload: &[u8], npdu_length: u8) -> Option<Self> {
        if payload.len() < 2 {
            return None;
        }

        let apci_raw = u16::from_be_bytes([payload[0], payload[1]]) & 0x03FF;
        let (apdu_type, data) = decode_apci(apci_raw, payload, npdu_length)?;

        Some(Self { apdu_type, data })
    }

    /// Encode the APDU into TPDU payload bytes.
    ///
    /// Returns the bytes starting from the TPCI/APCI position.
    pub fn to_bytes(&self, tpci_bits: u8) -> Vec<u8> {
        let apci = self.apdu_type as u16;
        let is_short = is_short_apci(apci);
        let byte0 = (tpci_bits & 0xFC) | ((apci >> 8) as u8 & 0x03);
        #[expect(clippy::cast_possible_truncation)]
        let apci_low = apci as u8;

        if is_short && self.data.len() == 1 {
            // Short APDU: data encoded in lower 6 bits of byte 1
            let byte1 = (apci_low & 0xC0) | (self.data[0] & 0x3F);
            alloc::vec![byte0, byte1]
        } else {
            // Long APDU: 2-byte APCI header + data
            let mut buf = alloc::vec![byte0, apci_low];
            buf.extend_from_slice(&self.data);
            buf
        }
    }
}

/// Determine if an APCI value uses the "short" encoding (6-bit data in byte 1).
///
/// Per the C++ reference: APCI values where `(apci >> 6) < 11` and `!= 7`
/// are short — the lower 6 bits are masked off for type identification.
const fn is_short_apci(apci: u16) -> bool {
    let high = apci >> 6;
    high < 11 && high != 7
}

/// Decode APCI value and extract data from payload.
fn decode_apci(apci_raw: u16, payload: &[u8], npdu_length: u8) -> Option<(ApduType, Vec<u8>)> {
    let type_bits = if is_short_apci(apci_raw) {
        apci_raw & 0x03C0
    } else {
        apci_raw
    };

    let apdu_type = match_apdu_type(type_bits)?;

    let data = if is_short_apci(apci_raw) && npdu_length <= 1 {
        // Short APDU: small value in lower 6 bits of byte 1
        alloc::vec![payload[1] & 0x3F]
    } else if payload.len() > 2 {
        // Long APDU: data after the 2-byte APCI header
        payload[2..].to_vec()
    } else {
        Vec::new()
    };

    Some((apdu_type, data))
}

/// Map a (masked) APCI value to an `ApduType` enum variant.
const fn match_apdu_type(bits: u16) -> Option<ApduType> {
    // This covers all variants from the C++ knx_types.h
    Some(match bits {
        0x000 => ApduType::GroupValueRead,
        0x040 => ApduType::GroupValueResponse,
        0x080 => ApduType::GroupValueWrite,
        0x0C0 => ApduType::IndividualAddressWrite,
        0x100 => ApduType::IndividualAddressRead,
        0x140 => ApduType::IndividualAddressResponse,
        0x180 => ApduType::AdcRead,
        0x1C0 => ApduType::AdcResponse,
        0x1C8 => ApduType::SystemNetworkParameterRead,
        0x1C9 => ApduType::SystemNetworkParameterResponse,
        0x1CA => ApduType::SystemNetworkParameterWrite,
        0x1CC => ApduType::PropertyValueExtRead,
        0x1CD => ApduType::PropertyValueExtResponse,
        0x1CE => ApduType::PropertyValueExtWriteCon,
        0x1CF => ApduType::PropertyValueExtWriteConResponse,
        0x1D0 => ApduType::PropertyValueExtWriteUnCon,
        0x1D2 => ApduType::PropertyExtDescriptionRead,
        0x1D3 => ApduType::PropertyExtDescriptionResponse,
        0x1D4 => ApduType::FunctionPropertyExtCommand,
        0x1D5 => ApduType::FunctionPropertyExtState,
        0x1D6 => ApduType::FunctionPropertyExtStateResponse,
        0x1FB => ApduType::MemoryExtWrite,
        0x1FC => ApduType::MemoryExtWriteResponse,
        0x1FD => ApduType::MemoryExtRead,
        0x1FE => ApduType::MemoryExtReadResponse,
        0x200 => ApduType::MemoryRead,
        0x240 => ApduType::MemoryResponse,
        0x280 => ApduType::MemoryWrite,
        0x2C0 => ApduType::UserMemoryRead,
        0x2C1 => ApduType::UserMemoryResponse,
        0x2C2 => ApduType::UserMemoryWrite,
        0x2C5 => ApduType::UserManufacturerInfoRead,
        0x2C6 => ApduType::UserManufacturerInfoResponse,
        0x2C7 => ApduType::FunctionPropertyCommand,
        0x2C8 => ApduType::FunctionPropertyState,
        0x2C9 => ApduType::FunctionPropertyStateResponse,
        0x300 => ApduType::DeviceDescriptorRead,
        0x340 => ApduType::DeviceDescriptorResponse,
        0x380 => ApduType::Restart,
        0x381 => ApduType::RestartMasterReset,
        0x3C0 => ApduType::RoutingTableOpen,
        0x3C1 => ApduType::RoutingTableRead,
        0x3C2 => ApduType::RoutingTableReadResponse,
        0x3C3 => ApduType::RoutingTableWrite,
        0x3C9 => ApduType::MemoryRouterReadResponse,
        0x3CA => ApduType::MemoryRouterWrite,
        0x3D1 => ApduType::AuthorizeRequest,
        0x3D2 => ApduType::AuthorizeResponse,
        0x3D3 => ApduType::KeyWrite,
        0x3D4 => ApduType::KeyResponse,
        0x3D5 => ApduType::PropertyValueRead,
        0x3D6 => ApduType::PropertyValueResponse,
        0x3D7 => ApduType::PropertyValueWrite,
        0x3D8 => ApduType::PropertyDescriptionRead,
        0x3D9 => ApduType::PropertyDescriptionResponse,
        0x3DC => ApduType::IndividualAddressSerialNumberRead,
        0x3DD => ApduType::IndividualAddressSerialNumberResponse,
        0x3DE => ApduType::IndividualAddressSerialNumberWrite,
        0x3E0 => ApduType::DomainAddressWrite,
        0x3E1 => ApduType::DomainAddressRead,
        0x3E2 => ApduType::DomainAddressResponse,
        0x3E3 => ApduType::DomainAddressSelectiveRead,
        0x3EC => ApduType::DomainAddressSerialNumberRead,
        0x3ED => ApduType::DomainAddressSerialNumberResponse,
        0x3EE => ApduType::DomainAddressSerialNumberWrite,
        0x3F1 => ApduType::SecureService,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_group_value_write_short() {
        // GroupValueWrite with value=1 (short APDU, npdu_length=1)
        let payload = &[0x00, 0x81]; // TPCI=0x00, APCI=0x0080 | data=0x01
        let apdu = Apdu::parse(payload, 1).unwrap();
        assert_eq!(apdu.apdu_type, ApduType::GroupValueWrite);
        assert_eq!(apdu.data, &[0x01]);
    }

    #[test]
    fn parse_group_value_read() {
        let payload = &[0x00, 0x00]; // GroupValueRead
        let apdu = Apdu::parse(payload, 0).unwrap();
        assert_eq!(apdu.apdu_type, ApduType::GroupValueRead);
    }

    #[test]
    fn parse_group_value_response_short() {
        let payload = &[0x00, 0x41]; // GroupValueResponse, value=1
        let apdu = Apdu::parse(payload, 1).unwrap();
        assert_eq!(apdu.apdu_type, ApduType::GroupValueResponse);
        assert_eq!(apdu.data, &[0x01]);
    }

    #[test]
    fn parse_group_value_write_long() {
        // GroupValueWrite with 2-byte DPT9 value (npdu_length=3)
        let payload = &[0x00, 0x80, 0x0C, 0x1A];
        let apdu = Apdu::parse(payload, 3).unwrap();
        assert_eq!(apdu.apdu_type, ApduType::GroupValueWrite);
        assert_eq!(apdu.data, &[0x0C, 0x1A]);
    }

    #[test]
    fn roundtrip_short_apdu() {
        let apdu = Apdu {
            apdu_type: ApduType::GroupValueWrite,
            data: alloc::vec![0x01],
        };
        let bytes = apdu.to_bytes(0x00);
        assert_eq!(bytes, &[0x00, 0x81]);

        let parsed = Apdu::parse(&bytes, 1).unwrap();
        assert_eq!(parsed.apdu_type, ApduType::GroupValueWrite);
        assert_eq!(parsed.data, &[0x01]);
    }

    #[test]
    fn roundtrip_long_apdu() {
        let apdu = Apdu {
            apdu_type: ApduType::GroupValueWrite,
            data: alloc::vec![0x0C, 0x1A],
        };
        let bytes = apdu.to_bytes(0x00);
        assert_eq!(bytes, &[0x00, 0x80, 0x0C, 0x1A]);
    }

    #[test]
    fn parse_property_value_read() {
        // PropertyValueRead = 0x3D5 — long APCI
        let payload = &[0x03, 0xD5, 0x01, 0x02, 0x03];
        let apdu = Apdu::parse(payload, 4).unwrap();
        assert_eq!(apdu.apdu_type, ApduType::PropertyValueRead);
        assert_eq!(apdu.data, &[0x01, 0x02, 0x03]);
    }

    #[test]
    fn parse_device_descriptor_read() {
        // DeviceDescriptorRead = 0x300, descriptor type in lower 6 bits
        let payload = &[0x03, 0x00];
        let apdu = Apdu::parse(payload, 1).unwrap();
        assert_eq!(apdu.apdu_type, ApduType::DeviceDescriptorRead);
    }

    #[test]
    fn parse_too_short() {
        assert!(Apdu::parse(&[0x00], 0).is_none());
        assert!(Apdu::parse(&[], 0).is_none());
    }
}
