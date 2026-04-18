// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! Transport Protocol Data Unit (TPDU).
//!
//! The TPDU wraps the APDU and adds transport-layer control information
//! (TPCI) in the first byte of the payload.
//!
//! # TPCI byte encoding
//!
//! ```text
//! Bit 7: control flag (1 = control TPDU, 0 = data TPDU)
//! Bit 6: numbered flag (1 = numbered/sequence, 0 = unnumbered)
//! Bits 5..2: sequence number (for numbered TPDUs)
//! Bits 1..0: varies (control type or APCI high bits)
//! ```

use crate::apdu::Apdu;
use crate::message::TpduType;
use crate::types::AddressType;

/// A parsed Transport Protocol Data Unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tpdu {
    /// Data TPDU carrying an APDU (group, broadcast, individual, or connected).
    Data {
        /// The transport-layer type.
        tpdu_type: TpduType,
        /// Sequence number (only meaningful for `DataConnected`).
        sequence_number: u8,
        /// The contained APDU.
        apdu: Apdu,
    },
    /// Control TPDU (connect, disconnect, ack, nack).
    Control {
        /// The transport-layer type.
        tpdu_type: TpduType,
        /// Sequence number (for ack/nack).
        sequence_number: u8,
    },
}

impl Tpdu {
    /// Parse a TPDU from the CEMI payload bytes.
    ///
    /// `payload` starts at the TPCI byte. `npdu_length` is the octet count
    /// from the CEMI frame. `address_type` and `destination_raw` are needed
    /// to distinguish broadcast/group/individual data TPDUs (matching C++ logic).
    pub fn parse(
        payload: &[u8],
        npdu_length: u8,
        address_type: AddressType,
        destination_raw: u16,
    ) -> Option<Self> {
        if payload.is_empty() {
            return None;
        }

        let tpci = payload[0];
        let is_control = tpci & 0x80 != 0;
        let is_numbered = tpci & 0x40 != 0;
        let sequence_number = (tpci >> 2) & 0x0F;

        if is_control {
            let tpdu_type = if is_numbered {
                if tpci & 0x01 == 0 {
                    TpduType::Ack
                } else {
                    TpduType::Nack
                }
            } else if tpci & 0x01 == 0 {
                TpduType::Connect
            } else {
                TpduType::Disconnect
            };
            Some(Self::Control {
                tpdu_type,
                sequence_number,
            })
        } else {
            let tpdu_type = if address_type == AddressType::Group {
                if destination_raw == 0 {
                    TpduType::DataBroadcast
                } else {
                    TpduType::DataGroup
                }
            } else if is_numbered {
                TpduType::DataConnected
            } else {
                TpduType::DataIndividual
            };

            let apdu = Apdu::parse(payload, npdu_length)?;

            Some(Self::Data {
                tpdu_type,
                sequence_number,
                apdu,
            })
        }
    }

    /// The transport-layer type.
    pub const fn tpdu_type(&self) -> TpduType {
        match self {
            Self::Data { tpdu_type, .. } | Self::Control { tpdu_type, .. } => *tpdu_type,
        }
    }

    /// The sequence number.
    pub const fn sequence_number(&self) -> u8 {
        match self {
            Self::Data {
                sequence_number, ..
            }
            | Self::Control {
                sequence_number, ..
            } => *sequence_number,
        }
    }

    /// The contained APDU, if this is a data TPDU.
    pub const fn apdu(&self) -> Option<&Apdu> {
        match self {
            Self::Data { apdu, .. } => Some(apdu),
            Self::Control { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::ApduType;

    #[test]
    fn parse_data_group() {
        let payload = &[0x00, 0x81]; // unnumbered data, GroupValueWrite(1)
        let tpdu = Tpdu::parse(payload, 1, AddressType::Group, 0x0801).unwrap();
        assert_eq!(tpdu.tpdu_type(), TpduType::DataGroup);
        assert_eq!(tpdu.sequence_number(), 0);
        let apdu = tpdu.apdu().unwrap();
        assert_eq!(apdu.apdu_type, ApduType::GroupValueWrite);
    }

    #[test]
    fn parse_data_broadcast() {
        let payload = &[0x00, 0x00]; // GroupValueRead to address 0
        let tpdu = Tpdu::parse(payload, 0, AddressType::Group, 0x0000).unwrap();
        assert_eq!(tpdu.tpdu_type(), TpduType::DataBroadcast);
    }

    #[test]
    fn parse_data_individual() {
        let payload = &[0x03, 0xD5, 0x01, 0x02, 0x03]; // PropertyValueRead
        let tpdu = Tpdu::parse(payload, 4, AddressType::Individual, 0x1101).unwrap();
        assert_eq!(tpdu.tpdu_type(), TpduType::DataIndividual);
    }

    #[test]
    fn parse_data_connected() {
        // Numbered data: TPCI = 0x40 | (seq=2 << 2) = 0x48
        let payload = &[0x48, 0x00]; // numbered, seq=2, GroupValueRead
        let tpdu = Tpdu::parse(payload, 0, AddressType::Individual, 0x1101).unwrap();
        assert_eq!(tpdu.tpdu_type(), TpduType::DataConnected);
        assert_eq!(tpdu.sequence_number(), 2);
    }

    #[test]
    fn parse_connect() {
        let payload = &[0x80]; // control, unnumbered, bit0=0 → Connect
        let tpdu = Tpdu::parse(payload, 0, AddressType::Individual, 0x1101).unwrap();
        assert_eq!(tpdu.tpdu_type(), TpduType::Connect);
    }

    #[test]
    fn parse_disconnect() {
        let payload = &[0x81]; // control, unnumbered, bit0=1 → Disconnect
        let tpdu = Tpdu::parse(payload, 0, AddressType::Individual, 0x1101).unwrap();
        assert_eq!(tpdu.tpdu_type(), TpduType::Disconnect);
    }

    #[test]
    fn parse_ack() {
        // Control + numbered + bit0=0: 0xC0 | (seq=3 << 2) = 0xCC, then | 0x02 for ack pattern
        // Actually C++ sets 0xC2 base for ack: control(0x80) | numbered(0x40) | 0x02
        let payload = &[0xC2]; // ack, seq=0
        let tpdu = Tpdu::parse(payload, 0, AddressType::Individual, 0x1101).unwrap();
        assert_eq!(tpdu.tpdu_type(), TpduType::Ack);
    }

    #[test]
    fn parse_nack() {
        let payload = &[0xC3]; // control + numbered + bit0=1 → Nack
        let tpdu = Tpdu::parse(payload, 0, AddressType::Individual, 0x1101).unwrap();
        assert_eq!(tpdu.tpdu_type(), TpduType::Nack);
    }

    #[test]
    fn parse_empty_payload() {
        assert!(Tpdu::parse(&[], 0, AddressType::Group, 0).is_none());
    }
}
