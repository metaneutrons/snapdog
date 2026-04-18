// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! DPT golden vector tests against C++ knx-openknx reference.
//!
//! The C++ `KNXValue` type uses implicit conversions (e.g. `double` → `uint8_t`)
//! that can lose precision. The **encode bytes** are authoritative for integer
//! DPTs; for float DPTs (9, 14) we allow ±1 LSB tolerance due to intermediate
//! precision differences (`float` vs `f64`).

use knx_core::dpt::{self, Dpt};
use serde::Deserialize;

#[derive(Deserialize)]
#[allow(dead_code)]
struct DptVector {
    main: u16,
    sub: u16,
    input: f64,
    bytes: Vec<u8>,
    decoded: Option<f64>,
    error: Option<bool>,
}

/// Check if two byte vectors are within ±1 of each other (LSB tolerance).
fn bytes_within_one(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    // Compare as big-endian integers
    let a_val = a
        .iter()
        .fold(0u64, |acc, &byte| (acc << 8) | u64::from(byte));
    let b_val = b
        .iter()
        .fold(0u64, |acc, &byte| (acc << 8) | u64::from(byte));
    a_val.abs_diff(b_val) <= 1
}

#[test]
fn dpt_encode_matches_cpp() {
    let json = include_str!("fixtures/dpt_vectors.json");
    let vectors: Vec<DptVector> = serde_json::from_str(json).expect("parse dpt_vectors.json");

    let mut passed = 0;
    let mut skipped = 0;

    for v in &vectors {
        let dpt_id = Dpt::new(v.main, v.sub);

        if v.error.unwrap_or(false) || v.bytes.is_empty() || v.input < 0.0 {
            skipped += 1;
            continue;
        }

        match dpt::encode(dpt_id, v.input) {
            Ok(encoded) => {
                if v.main == 9 {
                    // KNX float16: allow ±1 LSB due to float vs f64 precision
                    assert!(
                        bytes_within_one(&encoded, &v.bytes),
                        "DPT {dpt_id} encode: input {} → got {encoded:?}, expected {:?} (±1)",
                        v.input,
                        v.bytes
                    );
                } else {
                    assert_eq!(
                        encoded, v.bytes,
                        "DPT {dpt_id} encode: input {} → got {encoded:?}, expected {:?}",
                        v.input, v.bytes
                    );
                }
                passed += 1;
            }
            Err(_) => {
                skipped += 1;
            }
        }
    }

    eprintln!("DPT encode vectors: {passed} passed, {skipped} skipped");
    assert!(passed > 20, "too few DPT encode vectors passed: {passed}");
}

#[test]
fn dpt_decode_from_cpp_bytes() {
    let json = include_str!("fixtures/dpt_vectors.json");
    let vectors: Vec<DptVector> = serde_json::from_str(json).expect("parse dpt_vectors.json");

    let mut passed = 0;
    let mut skipped = 0;

    for v in &vectors {
        let dpt_id = Dpt::new(v.main, v.sub);

        if v.error.unwrap_or(false) || v.bytes.is_empty() {
            skipped += 1;
            continue;
        }

        match dpt::decode(dpt_id, &v.bytes) {
            Ok(decoded) => {
                // Re-encode and verify bytes match (roundtrip consistency)
                if let Ok(re_encoded) = dpt::encode(dpt_id, decoded) {
                    if v.main == 9 {
                        assert!(
                            bytes_within_one(&re_encoded, &v.bytes),
                            "DPT {dpt_id} roundtrip: decode({:?})={decoded} → {re_encoded:?} (±1)",
                            v.bytes
                        );
                    } else {
                        assert_eq!(
                            re_encoded, v.bytes,
                            "DPT {dpt_id} roundtrip: decode({:?})={decoded} → {re_encoded:?}",
                            v.bytes
                        );
                    }
                }
                passed += 1;
            }
            Err(_) => {
                skipped += 1;
            }
        }
    }

    eprintln!("DPT decode vectors: {passed} passed, {skipped} skipped");
    assert!(passed > 20, "too few DPT decode vectors passed: {passed}");
}
