// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Tests against golden vectors generated from the C++ knx-openknx reference.

use knx_core::cemi::CemiFrame;
use serde::Deserialize;

#[derive(Deserialize)]
struct CemiVector {
    name: String,
    bytes: Vec<u8>,
    message_code: u8,
    frame_type: u8,
    priority: u8,
    repetition: u8,
    system_broadcast: u8,
    ack: u8,
    confirm: u8,
    address_type: u8,
    hop_count: u8,
    source: u16,
    destination: u16,
    npdu_length: u8,
    total_length: u16,
}

#[test]
fn cemi_golden_vectors() {
    let json = include_str!("fixtures/cemi_vectors.json");
    let vectors: Vec<CemiVector> = serde_json::from_str(json).expect("parse cemi_vectors.json");

    for v in &vectors {
        let frame =
            CemiFrame::parse(&v.bytes).unwrap_or_else(|e| panic!("{}: parse failed: {e}", v.name));

        assert_eq!(
            frame.message_code_raw(),
            v.message_code,
            "{}: message_code",
            v.name
        );
        assert_eq!(
            frame.frame_type() as u8,
            v.frame_type,
            "{}: frame_type",
            v.name
        );
        assert_eq!(frame.priority() as u8, v.priority, "{}: priority", v.name);
        assert_eq!(
            frame.repetition() as u8,
            v.repetition,
            "{}: repetition",
            v.name
        );
        assert_eq!(
            frame.system_broadcast() as u8,
            v.system_broadcast,
            "{}: system_broadcast",
            v.name
        );
        assert_eq!(frame.ack() as u8, v.ack, "{}: ack", v.name);
        assert_eq!(frame.confirm() as u8, v.confirm, "{}: confirm", v.name);
        assert_eq!(
            frame.address_type() as u8,
            v.address_type,
            "{}: address_type",
            v.name
        );
        assert_eq!(frame.hop_count(), v.hop_count, "{}: hop_count", v.name);
        assert_eq!(frame.source_address().raw(), v.source, "{}: source", v.name);
        assert_eq!(
            frame.destination_address_raw(),
            v.destination,
            "{}: destination",
            v.name
        );
        assert_eq!(
            frame.npdu_length(),
            v.npdu_length,
            "{}: npdu_length",
            v.name
        );
        assert_eq!(
            frame.total_length() as u16,
            v.total_length,
            "{}: total_length",
            v.name
        );
    }
}
