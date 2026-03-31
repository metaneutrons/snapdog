// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! KNX/IP integration via knxkit.
//!
//! Supports tunneling and routing (multicast) connections.
//! Writes status values to KNX group addresses.

use anyhow::{Context, Result};
use knxkit::core::DataPoint;
use knxkit::core::address::GroupAddress;

use crate::config::KnxConfig;

/// Build the KNX remote URL from config.
pub fn remote_url(config: &KnxConfig) -> Result<String> {
    anyhow::ensure!(config.enabled, "KNX is disabled");
    match config.connection.as_str() {
        "tunnel" => {
            let gw = config
                .gateway
                .as_deref()
                .context("KNX tunnel requires gateway")?;
            Ok(format!("udp://{gw}"))
        }
        "router" => Ok(format!("udp://{}", config.multicast)),
        other => anyhow::bail!("Unknown KNX connection type: {other}"),
    }
}

/// Parse "1/2/3" into a GroupAddress.
pub fn parse_group_address(s: &str) -> Result<GroupAddress> {
    let parts: Vec<&str> = s.split('/').collect();
    anyhow::ensure!(
        parts.len() == 3,
        "Invalid group address: {s} (expected x/y/z)"
    );
    let main: u8 = parts[0].parse().context("Invalid main group")?;
    let middle: u8 = parts[1].parse().context("Invalid middle group")?;
    let sub: u8 = parts[2].parse().context("Invalid sub group")?;
    Ok(GroupAddress::from_components((main, middle, sub)))
}

/// Encode a boolean as KNX DPT 1.x.
pub fn encode_bool(value: bool) -> DataPoint {
    DataPoint::Short(u8::from(value))
}

/// Encode a percentage (0-100) as KNX DPT 5.001 (0-255 scaling).
pub fn encode_percent(percent: u8) -> DataPoint {
    DataPoint::Short(((percent as u16) * 255 / 100) as u8)
}

/// Encode a string as KNX DPT 16.001 (14-byte ASCII).
pub fn encode_string(value: &str) -> DataPoint {
    let mut bytes = vec![0u8; 14];
    let src = value.as_bytes();
    let len = src.len().min(14);
    bytes[..len].copy_from_slice(&src[..len]);
    DataPoint::Long(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_group_address() {
        let ga = parse_group_address("1/2/3").unwrap();
        assert_eq!(ga, GroupAddress::from_components((1, 2, 3)));
    }

    #[test]
    fn rejects_invalid_group_address() {
        assert!(parse_group_address("1/2").is_err());
        assert!(parse_group_address("abc").is_err());
    }

    #[test]
    fn encodes_bool() {
        assert_eq!(encode_bool(true), DataPoint::Short(1));
        assert_eq!(encode_bool(false), DataPoint::Short(0));
    }

    #[test]
    fn encodes_percent() {
        assert_eq!(encode_percent(0), DataPoint::Short(0));
        assert_eq!(encode_percent(100), DataPoint::Short(255));
        assert_eq!(encode_percent(50), DataPoint::Short(127));
    }

    #[test]
    fn encodes_string_truncates_to_14() {
        let dp = encode_string("Hello, World!!");
        if let DataPoint::Long(bytes) = dp {
            assert_eq!(bytes.len(), 14);
            assert_eq!(&bytes[..14], b"Hello, World!!");
        } else {
            panic!("Expected Long DataPoint");
        }
    }

    #[test]
    fn remote_url_tunnel() {
        let config = KnxConfig {
            enabled: true,
            connection: "tunnel".into(),
            gateway: Some("knxd:3671".into()),
            multicast: "224.0.23.12".into(),
        };
        assert_eq!(remote_url(&config).unwrap(), "udp://knxd:3671");
    }

    #[test]
    fn remote_url_router() {
        let config = KnxConfig {
            enabled: true,
            connection: "router".into(),
            gateway: None,
            multicast: "224.0.23.12".into(),
        };
        assert_eq!(remote_url(&config).unwrap(), "udp://224.0.23.12");
    }
}
