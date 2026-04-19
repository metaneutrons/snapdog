// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Fabian Schmieder

//! Configuration loading and validation.
//!
//! Single TOML file → all derived config (KNX addresses, sink paths, snapserver.conf).
//! Convention over configuration: sensible defaults, auto-generated where possible.

mod convention;
mod types;

pub use types::*;

use std::path::Path;

use anyhow::{Context, Result};

/// Load, validate, and resolve configuration from a TOML file.
pub fn load(path: &Path) -> Result<AppConfig> {
    let content = std::fs::read_to_string(path) /* blocking OK: called once at startup */
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let raw: RawConfig =
        toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))?;
    load_raw(raw)
}

/// Resolve raw TOML config into fully populated AppConfig with conventions applied.
pub fn load_raw(raw: RawConfig) -> Result<AppConfig> {
    anyhow::ensure!(
        !raw.zone.is_empty(),
        "At least one [[zone]] must be configured"
    );
    anyhow::ensure!(
        !raw.client.is_empty(),
        "At least one [[client]] must be configured"
    );

    let zones: Vec<ZoneConfig> = raw
        .zone
        .into_iter()
        .enumerate()
        .map(|(i, z)| convention::resolve_zone(i + 1, z, &raw.audio))
        .collect::<Result<_>>()?;

    let zone_names: Vec<&str> = zones.iter().map(|z| z.name.as_str()).collect();

    let clients: Vec<ClientConfig> = raw
        .client
        .into_iter()
        .enumerate()
        .map(|(i, c)| convention::resolve_client(i + 1, c, &zone_names))
        .collect::<Result<_>>()?;

    let radios: Vec<RadioConfig> = raw.radio.into_iter().map(Into::into).collect();

    // ── KNX mode validation ───────────────────────────────────
    if raw.knx.enabled {
        match raw.knx.mode.as_str() {
            "client" => {
                anyhow::ensure!(
                    raw.knx.url.is_some(),
                    "KNX client mode requires 'url' (e.g. udp://192.168.1.50:3671)"
                );
            }
            "device" => {
                anyhow::ensure!(
                    raw.knx.individual_address.is_some(),
                    "KNX device mode requires 'individual_address' (e.g. 1.1.100)"
                );
            }
            other => anyhow::bail!("Unknown KNX mode '{other}' — use 'client' or 'device'"),
        }
    }

    Ok(AppConfig {
        system: raw.system,
        audio: raw.audio,
        http: raw.http,
        snapcast: raw.snapcast,
        airplay: raw.airplay,
        subsonic: raw.subsonic,
        spotify: raw.spotify,
        mqtt: raw.mqtt,
        knx: raw.knx,
        zones,
        clients,
        radios,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_toml() -> &'static str {
        r#"
            [[zone]]
            name = "Ground Floor"

            [[client]]
            name = "Living Room"
            mac = "02:42:ac:11:00:10"
            zone = "Ground Floor"
        "#
    }

    #[test]
    fn parses_minimal_config() {
        let raw: RawConfig = toml::from_str(minimal_toml()).unwrap();
        let config = load_raw(raw).unwrap();
        assert_eq!(config.zones.len(), 1);
        assert_eq!(config.clients.len(), 1);
    }

    #[test]
    fn zone_conventions_applied() {
        let raw: RawConfig = toml::from_str(minimal_toml()).unwrap();
        let config = load_raw(raw).unwrap();
        let zone = &config.zones[0];
        assert_eq!(zone.index, 1);
        assert_eq!(zone.sink, "/snapsinks/zone1");
        assert_eq!(zone.stream_name, "Zone1");
        assert_eq!(zone.tcp_source_port, 4953);
    }

    #[test]
    fn knx_zone_no_defaults() {
        let raw: RawConfig = toml::from_str(minimal_toml()).unwrap();
        let config = load_raw(raw).unwrap();
        let knx = &config.zones[0].knx;
        // No convention defaults — all None unless explicitly configured
        assert!(knx.play.is_none());
        assert!(knx.volume.is_none());
        assert!(knx.mute.is_none());
    }

    #[test]
    fn knx_client_no_defaults() {
        let raw: RawConfig = toml::from_str(minimal_toml()).unwrap();
        let config = load_raw(raw).unwrap();
        let knx = &config.clients[0].knx;
        assert!(knx.volume.is_none());
        assert!(knx.mute.is_none());
    }

    #[test]
    fn client_zone_resolved_by_name() {
        let raw: RawConfig = toml::from_str(minimal_toml()).unwrap();
        let config = load_raw(raw).unwrap();
        assert_eq!(config.clients[0].zone_index, 1);
    }

    #[test]
    fn rejects_empty_zones() {
        let raw: RawConfig = toml::from_str(
            r#"
            [[client]]
            name = "X"
            mac = "00:00:00:00:00:00"
            zone = "X"
        "#,
        )
        .unwrap();
        assert!(load_raw(raw).is_err());
    }

    #[test]
    fn rejects_invalid_zone_reference() {
        let raw: RawConfig = toml::from_str(
            r#"
            [[zone]]
            name = "Ground Floor"

            [[client]]
            name = "X"
            mac = "00:00:00:00:00:00"
            zone = "Nonexistent"
        "#,
        )
        .unwrap();
        assert!(load_raw(raw).is_err());
    }

    #[test]
    fn zone_sink_override() {
        let raw: RawConfig = toml::from_str(
            r#"
            [[zone]]
            name = "Custom"
            sink = "/custom/path"

            [[client]]
            name = "X"
            mac = "00:00:00:00:00:00"
            zone = "Custom"
        "#,
        )
        .unwrap();
        let config = load_raw(raw).unwrap();
        assert_eq!(config.zones[0].sink, "/custom/path");
    }

    #[test]
    fn second_zone_gets_correct_indices() {
        let raw: RawConfig = toml::from_str(
            r#"
            [[zone]]
            name = "A"
            [[zone]]
            name = "B"

            [[client]]
            name = "X"
            mac = "00:00:00:00:00:00"
            zone = "B"
        "#,
        )
        .unwrap();
        let config = load_raw(raw).unwrap();
        assert_eq!(config.zones[1].index, 2);
        assert_eq!(config.zones[1].sink, "/snapsinks/zone2");
        assert_eq!(config.zones[1].tcp_source_port, 4954);
        assert_eq!(config.zones[1].knx.play, None);
        assert_eq!(config.clients[0].zone_index, 2);
    }

    #[test]
    fn knx_client_mode_requires_url() {
        let raw: RawConfig = toml::from_str(
            r#"
            [knx]
            enabled = true
            mode = "client"

            [[zone]]
            name = "A"
            [[client]]
            name = "X"
            mac = "00:00:00:00:00:00"
            zone = "A"
        "#,
        )
        .unwrap();
        assert!(load_raw(raw).unwrap_err().to_string().contains("url"));
    }

    #[test]
    fn knx_device_mode_requires_individual_address() {
        let raw: RawConfig = toml::from_str(
            r#"
            [knx]
            enabled = true
            mode = "device"

            [[zone]]
            name = "A"
            [[client]]
            name = "X"
            mac = "00:00:00:00:00:00"
            zone = "A"
        "#,
        )
        .unwrap();
        assert!(
            load_raw(raw)
                .unwrap_err()
                .to_string()
                .contains("individual_address")
        );
    }

    #[test]
    fn knx_rejects_unknown_mode() {
        let raw: RawConfig = toml::from_str(
            r#"
            [knx]
            enabled = true
            mode = "bogus"

            [[zone]]
            name = "A"
            [[client]]
            name = "X"
            mac = "00:00:00:00:00:00"
            zone = "A"
        "#,
        )
        .unwrap();
        assert!(load_raw(raw).unwrap_err().to_string().contains("bogus"));
    }

    #[test]
    fn knx_disabled_skips_validation() {
        let raw: RawConfig = toml::from_str(
            r#"
            [knx]
            enabled = false

            [[zone]]
            name = "A"
            [[client]]
            name = "X"
            mac = "00:00:00:00:00:00"
            zone = "A"
        "#,
        )
        .unwrap();
        assert!(load_raw(raw).is_ok());
    }
}
