// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! KNX address types.
//!
//! KNX uses two address types, both encoded as 16-bit values on the wire:
//!
//! - [`IndividualAddress`] — identifies a single device (area.line.device)
//! - [`GroupAddress`] — identifies a communication group (main/middle/sub)

use core::fmt;

/// Error returned when parsing an address from a string or raw bytes fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressParseError {
    /// The string does not contain the expected number of dot- or slash-separated segments.
    InvalidFormat,
    /// A numeric segment is not a valid integer.
    InvalidSegment,
    /// A segment value exceeds the allowed range for its field.
    OutOfRange,
}

impl fmt::Display for AddressParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat => f.write_str("invalid address format"),
            Self::InvalidSegment => f.write_str("invalid numeric segment"),
            Self::OutOfRange => f.write_str("segment value out of range"),
        }
    }
}

impl core::error::Error for AddressParseError {}

// ── Individual Address ────────────────────────────────────────

/// A KNX individual (physical) address identifying a single device.
///
/// Encoded as 16 bits: `AAAA.LLLL.DDDDDDDD` where
/// - area (4 bits, 0–15)
/// - line (4 bits, 0–15)
/// - device (8 bits, 0–255)
///
/// String representation: `area.line.device` (e.g. `1.1.1`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndividualAddress(u16);

impl IndividualAddress {
    /// Create from a raw 16-bit wire encoding.
    #[inline]
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    /// Create from area, line, and device components.
    ///
    /// # Errors
    ///
    /// Returns [`AddressParseError::OutOfRange`] if area > 15, line > 15, or device > 255.
    pub const fn new(area: u8, line: u8, device: u8) -> Result<Self, AddressParseError> {
        if area > 15 || line > 15 {
            return Err(AddressParseError::OutOfRange);
        }
        Ok(Self(
            ((area as u16) << 12) | ((line as u16) << 8) | (device as u16),
        ))
    }

    /// The raw 16-bit wire encoding.
    #[inline]
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// Area component (4 bits, 0–15).
    #[inline]
    pub const fn area(self) -> u8 {
        (self.0 >> 12) as u8 & 0x0F
    }

    /// Line component (4 bits, 0–15).
    #[inline]
    pub const fn line(self) -> u8 {
        (self.0 >> 8) as u8 & 0x0F
    }

    /// Device component (8 bits, 0–255).
    #[inline]
    pub const fn device(self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Encode to big-endian bytes for wire transmission.
    #[inline]
    pub const fn to_bytes(self) -> [u8; 2] {
        self.0.to_be_bytes()
    }

    /// Decode from big-endian wire bytes.
    #[inline]
    pub const fn from_bytes(bytes: [u8; 2]) -> Self {
        Self(u16::from_be_bytes(bytes))
    }
}

impl fmt::Display for IndividualAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.area(), self.line(), self.device())
    }
}

impl fmt::Debug for IndividualAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "IndividualAddress({self})")
    }
}

impl core::str::FromStr for IndividualAddress {
    type Err = AddressParseError;

    /// Parse from `"area.line.device"` notation (e.g. `"1.1.1"`).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(3, '.');
        let area = next_segment(&mut parts)?;
        let line = next_segment(&mut parts)?;
        let device = next_segment(&mut parts)?;
        if parts.next().is_some() {
            return Err(AddressParseError::InvalidFormat);
        }
        Self::new(area, line, device)
    }
}

// ── Group Address ─────────────────────────────────────────────

/// A KNX group address identifying a communication group.
///
/// Encoded as 16 bits on the wire. Supports two notations:
///
/// - **3-level** `main/middle/sub`: 5 bits / 3 bits / 8 bits (e.g. `1/0/1`)
/// - **2-level** `main/sub`: 5 bits / 11 bits (e.g. `1/1`)
///
/// The wire encoding is identical regardless of notation.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupAddress(u16);

impl GroupAddress {
    /// Create from a raw 16-bit wire encoding.
    #[inline]
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    /// Create from 3-level notation (main/middle/sub).
    ///
    /// # Errors
    ///
    /// Returns [`AddressParseError::OutOfRange`] if main > 31, middle > 7, or sub > 255.
    pub const fn new_3level(main: u8, middle: u8, sub: u8) -> Result<Self, AddressParseError> {
        if main > 31 || middle > 7 {
            return Err(AddressParseError::OutOfRange);
        }
        Ok(Self(
            ((main as u16) << 11) | ((middle as u16) << 8) | (sub as u16),
        ))
    }

    /// Create from 2-level notation (main/sub).
    ///
    /// # Errors
    ///
    /// Returns [`AddressParseError::OutOfRange`] if main > 31 or sub > 2047.
    pub const fn new_2level(main: u8, sub: u16) -> Result<Self, AddressParseError> {
        if main > 31 || sub > 2047 {
            return Err(AddressParseError::OutOfRange);
        }
        Ok(Self(((main as u16) << 11) | sub))
    }

    /// The raw 16-bit wire encoding.
    #[inline]
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// Main group (5 bits, 0–31).
    #[inline]
    pub const fn main(self) -> u8 {
        (self.0 >> 11) as u8 & 0x1F
    }

    /// Middle group in 3-level notation (3 bits, 0–7).
    #[inline]
    pub const fn middle(self) -> u8 {
        (self.0 >> 8) as u8 & 0x07
    }

    /// Sub group in 3-level notation (8 bits, 0–255).
    #[inline]
    pub const fn sub(self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Sub group in 2-level notation (11 bits, 0–2047).
    #[inline]
    pub const fn sub_2level(self) -> u16 {
        self.0 & 0x07FF
    }

    /// Encode to big-endian bytes for wire transmission.
    #[inline]
    pub const fn to_bytes(self) -> [u8; 2] {
        self.0.to_be_bytes()
    }

    /// Decode from big-endian wire bytes.
    #[inline]
    pub const fn from_bytes(bytes: [u8; 2]) -> Self {
        Self(u16::from_be_bytes(bytes))
    }
}

impl fmt::Display for GroupAddress {
    /// Formats as 3-level notation: `main/middle/sub`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}/{}", self.main(), self.middle(), self.sub())
    }
}

impl fmt::Debug for GroupAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GroupAddress({self})")
    }
}

impl core::str::FromStr for GroupAddress {
    type Err = AddressParseError;

    /// Parse from `"main/middle/sub"` (3-level) or `"main/sub"` (2-level) notation.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: alloc::vec::Vec<&str> = s.splitn(4, '/').collect();
        match parts.len() {
            2 => {
                let main = parse_u8(parts[0])?;
                let sub = parse_u16(parts[1])?;
                Self::new_2level(main, sub)
            }
            3 => {
                let main = parse_u8(parts[0])?;
                let middle = parse_u8(parts[1])?;
                let sub = parse_u8(parts[2])?;
                Self::new_3level(main, middle, sub)
            }
            _ => Err(AddressParseError::InvalidFormat),
        }
    }
}

// ── Destination Address ───────────────────────────────────────

/// A destination address in a KNX telegram — either individual or group.
///
/// The address type bit in the CEMI frame control field determines interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DestinationAddress {
    /// Unicast to a specific device.
    Individual(IndividualAddress),
    /// Multicast to a group.
    Group(GroupAddress),
}

impl fmt::Display for DestinationAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Individual(addr) => write!(f, "{addr}"),
            Self::Group(addr) => write!(f, "{addr}"),
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────

fn next_segment<'a>(parts: &mut impl Iterator<Item = &'a str>) -> Result<u8, AddressParseError> {
    let s = parts.next().ok_or(AddressParseError::InvalidFormat)?;
    parse_u8(s)
}

fn parse_u8(s: &str) -> Result<u8, AddressParseError> {
    s.parse::<u8>()
        .map_err(|_| AddressParseError::InvalidSegment)
}

fn parse_u16(s: &str) -> Result<u16, AddressParseError> {
    s.parse::<u16>()
        .map_err(|_| AddressParseError::InvalidSegment)
}

// ── Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use core::str::FromStr;

    // -- IndividualAddress --

    #[test]
    fn individual_roundtrip_components() {
        let addr = IndividualAddress::new(1, 1, 1).unwrap();
        assert_eq!(addr.area(), 1);
        assert_eq!(addr.line(), 1);
        assert_eq!(addr.device(), 1);
        assert_eq!(addr.raw(), 0x1101);
    }

    #[test]
    fn individual_max_values() {
        let addr = IndividualAddress::new(15, 15, 255).unwrap();
        assert_eq!(addr.raw(), 0xFFFF);
    }

    #[test]
    fn individual_out_of_range() {
        assert!(IndividualAddress::new(16, 0, 0).is_err());
        assert!(IndividualAddress::new(0, 16, 0).is_err());
    }

    #[test]
    fn individual_display_parse_roundtrip() {
        let addr = IndividualAddress::new(1, 2, 3).unwrap();
        let s = alloc::format!("{addr}");
        assert_eq!(s, "1.2.3");
        let parsed = IndividualAddress::from_str(&s).unwrap();
        assert_eq!(addr, parsed);
    }

    #[test]
    fn individual_bytes_roundtrip() {
        let addr = IndividualAddress::new(1, 1, 1).unwrap();
        let bytes = addr.to_bytes();
        assert_eq!(IndividualAddress::from_bytes(bytes), addr);
    }

    #[test]
    fn individual_from_raw() {
        let addr = IndividualAddress::from_raw(0x1001);
        assert_eq!(addr.area(), 1);
        assert_eq!(addr.line(), 0);
        assert_eq!(addr.device(), 1);
    }

    // -- GroupAddress --

    #[test]
    fn group_3level_roundtrip() {
        let addr = GroupAddress::new_3level(1, 0, 1).unwrap();
        assert_eq!(addr.main(), 1);
        assert_eq!(addr.middle(), 0);
        assert_eq!(addr.sub(), 1);
        assert_eq!(addr.raw(), 0x0801);
    }

    #[test]
    fn group_3level_max() {
        let addr = GroupAddress::new_3level(31, 7, 255).unwrap();
        assert_eq!(addr.raw(), 0xFFFF);
    }

    #[test]
    fn group_3level_out_of_range() {
        assert!(GroupAddress::new_3level(32, 0, 0).is_err());
        assert!(GroupAddress::new_3level(0, 8, 0).is_err());
    }

    #[test]
    fn group_2level_roundtrip() {
        let addr = GroupAddress::new_2level(1, 1).unwrap();
        assert_eq!(addr.main(), 1);
        assert_eq!(addr.sub_2level(), 1);
    }

    #[test]
    fn group_2level_out_of_range() {
        assert!(GroupAddress::new_2level(32, 0).is_err());
        assert!(GroupAddress::new_2level(0, 2048).is_err());
    }

    #[test]
    fn group_display_3level() {
        let addr = GroupAddress::new_3level(1, 2, 3).unwrap();
        assert_eq!(alloc::format!("{addr}"), "1/2/3");
    }

    #[test]
    fn group_parse_3level() {
        let addr = GroupAddress::from_str("1/0/1").unwrap();
        assert_eq!(addr.main(), 1);
        assert_eq!(addr.middle(), 0);
        assert_eq!(addr.sub(), 1);
    }

    #[test]
    fn group_parse_2level() {
        let addr = GroupAddress::from_str("1/1").unwrap();
        assert_eq!(addr.main(), 1);
        assert_eq!(addr.sub_2level(), 1);
    }

    #[test]
    fn group_bytes_roundtrip() {
        let addr = GroupAddress::new_3level(1, 0, 1).unwrap();
        let bytes = addr.to_bytes();
        assert_eq!(GroupAddress::from_bytes(bytes), addr);
    }

    #[test]
    fn group_wire_encoding_matches_cpp() {
        // C++ encodes GroupAddress 1/0/1 as (1 << 11) | (0 << 8) | 1 = 0x0801
        let addr = GroupAddress::new_3level(1, 0, 1).unwrap();
        assert_eq!(addr.to_bytes(), [0x08, 0x01]);

        // C++ encodes GroupAddress 31/7/255 as 0xFFFF
        let addr = GroupAddress::new_3level(31, 7, 255).unwrap();
        assert_eq!(addr.to_bytes(), [0xFF, 0xFF]);

        // C++ encodes GroupAddress 0/0/0 as 0x0000
        let addr = GroupAddress::new_3level(0, 0, 0).unwrap();
        assert_eq!(addr.to_bytes(), [0x00, 0x00]);
    }

    // -- DestinationAddress --

    #[test]
    fn destination_display() {
        let ind = DestinationAddress::Individual(IndividualAddress::new(1, 1, 1).unwrap());
        assert_eq!(alloc::format!("{ind}"), "1.1.1");

        let grp = DestinationAddress::Group(GroupAddress::new_3level(1, 0, 1).unwrap());
        assert_eq!(alloc::format!("{grp}"), "1/0/1");
    }

    // -- Error cases --

    #[test]
    fn parse_invalid_format() {
        assert!(IndividualAddress::from_str("1.2").is_err());
        assert!(IndividualAddress::from_str("1.2.3.4").is_err());
        assert!(IndividualAddress::from_str("").is_err());
        assert!(GroupAddress::from_str("").is_err());
        assert!(GroupAddress::from_str("1").is_err());
        assert!(GroupAddress::from_str("1/2/3/4").is_err());
    }

    #[test]
    fn parse_invalid_segment() {
        assert!(IndividualAddress::from_str("a.b.c").is_err());
        assert!(GroupAddress::from_str("a/b/c").is_err());
    }
}
