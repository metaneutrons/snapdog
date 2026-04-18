// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! DPT encode/decode dispatch and concrete implementations.

use alloc::vec::Vec;

use super::{Dpt, DptError};

/// Round an `f64` to the nearest integer (`no_std` compatible).
fn round(v: f64) -> f64 {
    libm::round(v)
}

/// Clamp, round, and cast `f64` to `u8`.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_u8(v: f64) -> u8 {
    round(v).clamp(0.0, 255.0) as u8
}

/// Clamp, round, and cast `f64` to `i8`.
#[expect(clippy::cast_possible_truncation)]
fn to_i8(v: f64) -> i8 {
    round(v).clamp(-128.0, 127.0) as i8
}

/// Clamp, round, and cast `f64` to `u16`.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_u16(v: f64) -> u16 {
    round(v).clamp(0.0, 65535.0) as u16
}

/// Clamp, round, and cast `f64` to `i16`.
#[expect(clippy::cast_possible_truncation)]
fn to_i16(v: f64) -> i16 {
    round(v).clamp(-32768.0, 32767.0) as i16
}

/// Clamp, round, and cast `f64` to `u32`.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn to_u32(v: f64) -> u32 {
    round(v).clamp(0.0, f64::from(u32::MAX)) as u32
}

/// Clamp, round, and cast `f64` to `i32`.
#[expect(clippy::cast_possible_truncation)]
fn to_i32(v: f64) -> i32 {
    round(v).clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

/// Decode a KNX bus payload into an `f64` value.
pub(super) fn decode(dpt: Dpt, payload: &[u8]) -> Result<f64, DptError> {
    match dpt.main {
        1 => decode_dpt1(payload),
        2 => decode_dpt2(payload),
        3 => decode_dpt3(payload),
        5 => decode_dpt5(dpt, payload),
        6 => decode_dpt6(payload),
        7 => decode_dpt7(payload),
        8 => decode_dpt8(payload),
        9 => decode_dpt9(payload),
        12 => decode_dpt12(payload),
        13 => decode_dpt13(payload),
        14 => decode_dpt14(payload),
        _ => Err(DptError::UnsupportedDpt(dpt)),
    }
}

/// Encode an `f64` value into a KNX bus payload.
pub(super) fn encode(dpt: Dpt, value: f64) -> Result<Vec<u8>, DptError> {
    match dpt.main {
        1 => Ok(encode_dpt1(value)),
        2 => Ok(encode_dpt2(value)),
        3 => Ok(encode_dpt3(value)),
        5 => Ok(encode_dpt5(dpt, value)),
        6 => Ok(encode_dpt6(value)),
        7 => Ok(encode_dpt7(value)),
        8 => Ok(encode_dpt8(value)),
        9 => encode_dpt9(value),
        12 => Ok(encode_dpt12(value)),
        13 => Ok(encode_dpt13(value)),
        14 => Ok(encode_dpt14(value)),
        _ => Err(DptError::UnsupportedDpt(dpt)),
    }
}

// ── DPT 1: Boolean (1 bit) ───────────────────────────────────

fn decode_dpt1(payload: &[u8]) -> Result<f64, DptError> {
    check_len(payload, 1)?;
    Ok(f64::from(payload[0] & 0x01))
}

fn encode_dpt1(value: f64) -> Vec<u8> {
    alloc::vec![u8::from(value != 0.0)]
}

// ── DPT 2: 1-bit controlled (2 bits) ─────────────────────────

fn decode_dpt2(payload: &[u8]) -> Result<f64, DptError> {
    check_len(payload, 1)?;
    Ok(f64::from(payload[0] & 0x03))
}

fn encode_dpt2(value: f64) -> Vec<u8> {
    alloc::vec![to_u8(value) & 0x03]
}

// ── DPT 3: 3-bit controlled (4 bits) ─────────────────────────

fn decode_dpt3(payload: &[u8]) -> Result<f64, DptError> {
    check_len(payload, 1)?;
    Ok(f64::from(payload[0] & 0x0F))
}

fn encode_dpt3(value: f64) -> Vec<u8> {
    alloc::vec![to_u8(value) & 0x0F]
}

// ── DPT 5: Unsigned 8-bit (1 byte) ───────────────────────────

fn decode_dpt5(dpt: Dpt, payload: &[u8]) -> Result<f64, DptError> {
    check_len(payload, 1)?;
    let raw = f64::from(payload[0]);
    Ok(match dpt.sub {
        1 => raw * 100.0 / 255.0, // Scaling: 0..255 → 0..100%
        3 => raw * 360.0 / 255.0, // Angle: 0..255 → 0..360°
        _ => raw,                 // Raw unsigned 8-bit
    })
}

fn encode_dpt5(dpt: Dpt, value: f64) -> Vec<u8> {
    let raw = match dpt.sub {
        1 => value * 255.0 / 100.0, // 0..100% → 0..255
        3 => value * 255.0 / 360.0, // 0..360° → 0..255
        _ => value,
    };
    alloc::vec![to_u8(raw)]
}

// ── DPT 6: Signed 8-bit (1 byte) ─────────────────────────────

#[expect(clippy::cast_possible_wrap)]
fn decode_dpt6(payload: &[u8]) -> Result<f64, DptError> {
    check_len(payload, 1)?;
    Ok(f64::from(payload[0] as i8))
}

#[expect(clippy::cast_sign_loss)]
fn encode_dpt6(value: f64) -> Vec<u8> {
    alloc::vec![to_i8(value) as u8]
}

// ── DPT 7: Unsigned 16-bit (2 bytes) ─────────────────────────

fn decode_dpt7(payload: &[u8]) -> Result<f64, DptError> {
    check_len(payload, 2)?;
    Ok(f64::from(u16::from_be_bytes([payload[0], payload[1]])))
}

fn encode_dpt7(value: f64) -> Vec<u8> {
    to_u16(value).to_be_bytes().to_vec()
}

// ── DPT 8: Signed 16-bit (2 bytes) ───────────────────────────

fn decode_dpt8(payload: &[u8]) -> Result<f64, DptError> {
    check_len(payload, 2)?;
    Ok(f64::from(i16::from_be_bytes([payload[0], payload[1]])))
}

fn encode_dpt8(value: f64) -> Vec<u8> {
    to_i16(value).to_be_bytes().to_vec()
}

// ── DPT 9: 16-bit float (2 bytes, KNX F16) ───────────────────

/// KNX 16-bit float: `0.01 * mantissa * 2^exponent`
///
/// Wire format: `MEEEEMMM MMMMMMMM`
/// - M = sign + 11-bit mantissa (two's complement)
/// - E = 4-bit exponent
fn decode_dpt9(payload: &[u8]) -> Result<f64, DptError> {
    check_len(payload, 2)?;
    let raw = u16::from_be_bytes([payload[0], payload[1]]);
    let exponent = i32::from((raw >> 11) & 0x0F);
    let mantissa = {
        let m = i32::from(raw & 0x07FF);
        if raw & 0x8000 != 0 { m - 0x0800 } else { m }
    };
    Ok(0.01 * f64::from(mantissa) * f64::from(1 << exponent))
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn encode_dpt9(value: f64) -> Result<Vec<u8>, DptError> {
    let mut mantissa = round(value * 100.0) as i32;
    let mut exponent: u16 = 0;

    while mantissa > 2047 {
        mantissa >>= 1;
        exponent += 1;
    }
    while mantissa < -2048 {
        mantissa >>= 1;
        exponent += 1;
    }

    if exponent > 15 {
        return Err(DptError::OutOfRange);
    }

    let m = (mantissa & 0x07FF) as u16;
    let sign: u16 = if mantissa < 0 { 0x8000 } else { 0 };
    let raw = sign | (exponent << 11) | m;
    Ok(raw.to_be_bytes().to_vec())
}

// ── DPT 12: Unsigned 32-bit (4 bytes) ────────────────────────

fn decode_dpt12(payload: &[u8]) -> Result<f64, DptError> {
    check_len(payload, 4)?;
    Ok(f64::from(u32::from_be_bytes([
        payload[0], payload[1], payload[2], payload[3],
    ])))
}

fn encode_dpt12(value: f64) -> Vec<u8> {
    to_u32(value).to_be_bytes().to_vec()
}

// ── DPT 13: Signed 32-bit (4 bytes) ──────────────────────────

fn decode_dpt13(payload: &[u8]) -> Result<f64, DptError> {
    check_len(payload, 4)?;
    Ok(f64::from(i32::from_be_bytes([
        payload[0], payload[1], payload[2], payload[3],
    ])))
}

fn encode_dpt13(value: f64) -> Vec<u8> {
    to_i32(value).to_be_bytes().to_vec()
}

// ── DPT 14: IEEE 754 32-bit float (4 bytes) ──────────────────

fn decode_dpt14(payload: &[u8]) -> Result<f64, DptError> {
    check_len(payload, 4)?;
    Ok(f64::from(f32::from_be_bytes([
        payload[0], payload[1], payload[2], payload[3],
    ])))
}

#[expect(clippy::cast_possible_truncation)]
fn encode_dpt14(value: f64) -> Vec<u8> {
    (value as f32).to_be_bytes().to_vec()
}

// ── Helpers ───────────────────────────────────────────────────

const fn check_len(payload: &[u8], min: usize) -> Result<(), DptError> {
    if payload.len() < min {
        Err(DptError::PayloadTooShort)
    } else {
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn dpt1_roundtrip() {
        let on = encode(DPT_SWITCH, 1.0).unwrap();
        assert_eq!(on, &[1]);
        assert_eq!(decode(DPT_SWITCH, &on).unwrap(), 1.0);

        let off = encode(DPT_SWITCH, 0.0).unwrap();
        assert_eq!(off, &[0]);
        assert_eq!(decode(DPT_SWITCH, &off).unwrap(), 0.0);
    }

    #[test]
    fn dpt5_scaling_roundtrip() {
        let bytes = encode(DPT_SCALING, 50.0).unwrap();
        let val = decode(DPT_SCALING, &bytes).unwrap();
        assert!((val - 50.0).abs() < 1.0, "expected ~50, got {val}");
    }

    #[test]
    fn dpt5_scaling_boundaries() {
        let zero = encode(DPT_SCALING, 0.0).unwrap();
        assert_eq!(zero, &[0]);
        assert_eq!(decode(DPT_SCALING, &zero).unwrap(), 0.0);

        let full = encode(DPT_SCALING, 100.0).unwrap();
        assert_eq!(full, &[255]);
        let val = decode(DPT_SCALING, &full).unwrap();
        assert!((val - 100.0).abs() < 0.5);
    }

    #[test]
    fn dpt5_raw_unsigned() {
        let bytes = encode(DPT_VALUE_1_UCOUNT, 42.0).unwrap();
        assert_eq!(bytes, &[42]);
        assert_eq!(decode(DPT_VALUE_1_UCOUNT, &bytes).unwrap(), 42.0);
    }

    #[test]
    fn dpt9_temperature_roundtrip() {
        let bytes = encode(DPT_VALUE_TEMP, 21.5).unwrap();
        let val = decode(DPT_VALUE_TEMP, &bytes).unwrap();
        assert!((val - 21.5).abs() < 0.1, "expected ~21.5, got {val}");
    }

    #[test]
    fn dpt9_negative() {
        let bytes = encode(DPT_VALUE_TEMP, -10.0).unwrap();
        let val = decode(DPT_VALUE_TEMP, &bytes).unwrap();
        assert!((val - (-10.0)).abs() < 0.1, "expected ~-10, got {val}");
    }

    #[test]
    fn dpt9_zero() {
        let bytes = encode(DPT_VALUE_TEMP, 0.0).unwrap();
        let val = decode(DPT_VALUE_TEMP, &bytes).unwrap();
        assert!(val.abs() < 0.01, "expected ~0, got {val}");
    }

    #[test]
    fn dpt9_known_encoding() {
        let val = decode(DPT_VALUE_TEMP, &[0x0C, 0x34]).unwrap();
        assert!((val - 21.52).abs() < 0.01, "got {val}");
    }

    #[test]
    fn dpt14_roundtrip() {
        let bytes = encode(DPT_VALUE_POWER, 1234.5).unwrap();
        assert_eq!(bytes.len(), 4);
        let val = decode(DPT_VALUE_POWER, &bytes).unwrap();
        assert!((val - 1234.5).abs() < 0.1, "expected ~1234.5, got {val}");
    }

    #[test]
    fn dpt7_unsigned16() {
        let dpt = Dpt::new(7, 1);
        let bytes = encode(dpt, 1000.0).unwrap();
        assert_eq!(bytes, &[0x03, 0xE8]);
        assert_eq!(decode(dpt, &bytes).unwrap(), 1000.0);
    }

    #[test]
    fn dpt8_signed16() {
        let dpt = Dpt::new(8, 1);
        let bytes = encode(dpt, -500.0).unwrap();
        assert_eq!(decode(dpt, &bytes).unwrap(), -500.0);
    }

    #[test]
    fn dpt12_unsigned32() {
        let dpt = Dpt::new(12, 1);
        let bytes = encode(dpt, 100_000.0).unwrap();
        assert_eq!(decode(dpt, &bytes).unwrap(), 100_000.0);
    }

    #[test]
    fn dpt13_signed32() {
        let dpt = Dpt::new(13, 1);
        let bytes = encode(dpt, -100_000.0).unwrap();
        assert_eq!(decode(dpt, &bytes).unwrap(), -100_000.0);
    }

    #[test]
    fn decode_payload_too_short() {
        assert!(decode(DPT_VALUE_TEMP, &[0x0C]).is_err());
        assert!(decode(DPT_VALUE_POWER, &[0x00, 0x00]).is_err());
    }

    #[test]
    fn unsupported_dpt() {
        let dpt = Dpt::new(999, 1);
        assert!(matches!(
            decode(dpt, &[0]),
            Err(DptError::UnsupportedDpt(_))
        ));
    }
}
