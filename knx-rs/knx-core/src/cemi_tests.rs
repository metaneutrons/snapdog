// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

#[cfg(test)]
mod tests {
    use crate::address::{DestinationAddress, GroupAddress, IndividualAddress};
    use crate::cemi::{CemiError, CemiFrame};
    use crate::message::MessageCode;
    use crate::types::{
        AckType, AddressType, Confirm, FrameFormat, Priority, Repetition, SystemBroadcast,
    };

    // ── Real-world frame: L_Data.ind GroupValueWrite to 1/0/1 ─

    /// Standard frame: L_Data.ind, source 1.1.1, dest 1/0/1, GroupValueWrite(true)
    /// Captured from a real KNX bus.
    const GROUP_WRITE_FRAME: &[u8] = &[
        0x29, // message code: L_Data.ind
        0x00, // additional info length: 0
        0xBC, // ctrl1: standard(0x80) | not-repeated(0x20) | broadcast(0x10) | low-prio(0x0C)
        0xE0, // ctrl2: group(0x80) | hop_count=6(0x60)
        0x11, 0x01, // source: 1.1.1
        0x08, 0x01, // destination: 1/0/1
        0x01, // NPDU length: 1 (1 octet of APDU data)
        0x00, 0x81, // TPDU: GroupValueWrite, value=1
    ];

    #[test]
    fn parse_group_write() {
        let frame = CemiFrame::parse(GROUP_WRITE_FRAME).unwrap();
        assert_eq!(frame.message_code_raw(), 0x29);
        assert_eq!(frame.frame_type(), FrameFormat::Standard);
        assert_eq!(frame.repetition(), Repetition::WasNotRepeated);
        assert_eq!(frame.system_broadcast(), SystemBroadcast::Broadcast);
        assert_eq!(frame.priority(), Priority::Low);
        assert_eq!(frame.ack(), AckType::DontCare);
        assert_eq!(frame.confirm(), Confirm::NoError);
        assert_eq!(frame.address_type(), AddressType::Group);
        assert_eq!(frame.hop_count(), 6);
        assert_eq!(frame.source_address(), IndividualAddress::from_raw(0x1101));
        assert_eq!(
            frame.destination_address(),
            DestinationAddress::Group(GroupAddress::from_raw(0x0801))
        );
        assert_eq!(frame.npdu_length(), 1);
        assert_eq!(frame.payload(), &[0x00, 0x81]);
    }

    // ── Round-trip: construct then parse ──────────────────────

    #[test]
    fn new_l_data_roundtrip() {
        let src = IndividualAddress::new(1, 1, 1).unwrap();
        let dst = DestinationAddress::Group(GroupAddress::new_3level(1, 0, 1).unwrap());
        let payload = &[0x00, 0x80]; // GroupValueWrite, value=0

        let frame = CemiFrame::new_l_data(MessageCode::LDataInd, src, dst, Priority::Low, payload);

        assert_eq!(frame.message_code_raw(), 0x29);
        assert_eq!(frame.source_address(), src);
        assert_eq!(frame.destination_address(), dst);
        assert_eq!(frame.priority(), Priority::Low);
        assert_eq!(frame.address_type(), AddressType::Group);
        assert_eq!(frame.hop_count(), 6);
        assert_eq!(frame.npdu_length(), 1);
        assert_eq!(frame.payload(), payload);

        // Re-parse from bytes
        let reparsed = CemiFrame::parse(frame.as_bytes()).unwrap();
        assert_eq!(reparsed.source_address(), src);
        assert_eq!(reparsed.destination_address(), dst);
        assert_eq!(reparsed.payload(), payload);
    }

    // ── Individual address destination ────────────────────────

    #[test]
    fn individual_destination() {
        let src = IndividualAddress::new(1, 0, 1).unwrap();
        let dst = DestinationAddress::Individual(IndividualAddress::new(1, 1, 5).unwrap());
        let payload = &[0x00]; // minimal TPCI

        let frame =
            CemiFrame::new_l_data(MessageCode::LDataReq, src, dst, Priority::System, payload);

        assert_eq!(frame.address_type(), AddressType::Individual);
        assert_eq!(frame.priority(), Priority::System);
        assert_eq!(
            frame.destination_address(),
            DestinationAddress::Individual(IndividualAddress::new(1, 1, 5).unwrap())
        );
    }

    // ── Priority variants ─────────────────────────────────────

    #[test]
    fn priority_encoding() {
        let src = IndividualAddress::from_raw(0);
        let dst = DestinationAddress::Group(GroupAddress::from_raw(1));
        let payload = &[0x00, 0x00];

        for prio in [
            Priority::System,
            Priority::Normal,
            Priority::Urgent,
            Priority::Low,
        ] {
            let frame = CemiFrame::new_l_data(MessageCode::LDataInd, src, dst, prio, payload);
            assert_eq!(frame.priority(), prio);
        }
    }

    // ── TP CRC ────────────────────────────────────────────────

    #[test]
    fn tp_crc_calculation() {
        // CRC is XOR of all bytes, starting from 0xFF
        assert_eq!(CemiFrame::calc_crc_tp(&[0x00]), 0xFF);
        assert_eq!(CemiFrame::calc_crc_tp(&[0xFF]), 0x00);
        assert_eq!(
            CemiFrame::calc_crc_tp(&[0xBC, 0x11, 0x01, 0x08, 0x01, 0x01, 0x00, 0x81]),
            0xBC ^ 0x11 ^ 0x01 ^ 0x08 ^ 0x01 ^ 0x01 ^ 0x00 ^ 0x81 ^ 0xFF
        );
    }

    // ── Error cases ───────────────────────────────────────────

    #[test]
    fn parse_too_short() {
        assert_eq!(CemiFrame::parse(&[]), Err(CemiError::TooShort));
        assert_eq!(CemiFrame::parse(&[0x29]), Err(CemiError::TooShort));
        assert_eq!(
            CemiFrame::parse(&[0x29, 0x00, 0xBC]),
            Err(CemiError::TooShort)
        );
    }

    #[test]
    fn parse_length_mismatch() {
        // Valid header but NPDU length says 5 octets, only 1 provided
        let bad = &[0x29, 0x00, 0xBC, 0xE0, 0x11, 0x01, 0x08, 0x01, 0x05, 0x00];
        assert_eq!(CemiFrame::parse(bad), Err(CemiError::LengthMismatch));
    }

    #[test]
    fn parse_with_additional_info() {
        // Frame with 2 bytes of additional info
        let frame_data = &[
            0x29, // message code
            0x02, // additional info length: 2
            0xAA, 0xBB, // additional info bytes
            0xBC, // ctrl1
            0xE0, // ctrl2
            0x11, 0x01, // source
            0x08, 0x01, // destination
            0x01, // NPDU length
            0x00, 0x81, // payload
        ];
        let frame = CemiFrame::parse(frame_data).unwrap();
        assert_eq!(frame.additional_info_length(), 2);
        assert_eq!(frame.source_address(), IndividualAddress::from_raw(0x1101));
        assert_eq!(frame.npdu_length(), 1);
    }

    // ── Total length ──────────────────────────────────────────

    #[test]
    fn total_length_matches() {
        let frame = CemiFrame::parse(GROUP_WRITE_FRAME).unwrap();
        assert_eq!(frame.total_length(), GROUP_WRITE_FRAME.len());
        assert_eq!(frame.as_bytes(), GROUP_WRITE_FRAME);
    }
}
