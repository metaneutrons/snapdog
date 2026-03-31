// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Configuration types: raw TOML structs and resolved application config.

use serde::Deserialize;

// ── Raw TOML structs (what the user writes) ───────────────────

/// Root of the TOML config file. Optional fields use defaults.
#[derive(Debug, Deserialize)]
pub struct RawConfig {
    #[serde(default)]
    pub system: SystemConfig,
    #[serde(default)]
    pub audio: AudioConfig,
    #[serde(default)]
    pub http: HttpConfig,
    #[serde(default)]
    pub snapcast: SnapcastConfig,
    #[serde(default)]
    pub airplay: AirplayConfig,
    #[serde(default)]
    pub subsonic: Option<SubsonicConfig>,
    #[serde(default)]
    pub mqtt: Option<MqttConfig>,
    #[serde(default)]
    pub knx: KnxConfig,
    #[serde(default)]
    pub zone: Vec<RawZoneConfig>,
    #[serde(default)]
    pub client: Vec<RawClientConfig>,
    #[serde(default)]
    pub radio: Vec<RawRadioConfig>,
}

#[derive(Debug, Deserialize)]
pub struct SystemConfig {
    #[serde(default = "default_log_level")]
    pub log_level: String,
    pub log_file: Option<String>,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            log_level: default_log_level(),
            log_file: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AudioConfig {
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    #[serde(default = "default_bit_depth")]
    pub bit_depth: u16,
    #[serde(default = "default_channels")]
    pub channels: u16,
    #[serde(default = "default_codec")]
    pub codec: String,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: default_sample_rate(),
            bit_depth: default_bit_depth(),
            channels: default_channels(),
            codec: default_codec(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct HttpConfig {
    #[serde(default = "default_http_port")]
    pub port: u16,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            port: default_http_port(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SnapcastConfig {
    #[serde(default = "default_snapcast_address")]
    pub address: String,
    #[serde(default = "default_jsonrpc_port")]
    pub jsonrpc_port: u16,
    #[serde(default = "default_streaming_port")]
    pub streaming_port: u16,
    #[serde(default = "default_true")]
    pub managed: bool,
}

impl Default for SnapcastConfig {
    fn default() -> Self {
        Self {
            address: default_snapcast_address(),
            jsonrpc_port: default_jsonrpc_port(),
            streaming_port: default_streaming_port(),
            managed: true,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AirplayConfig {
    #[serde(default = "default_airplay_name")]
    pub name: String,
    pub password: Option<String>,
}

impl Default for AirplayConfig {
    fn default() -> Self {
        Self {
            name: default_airplay_name(),
            password: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SubsonicConfig {
    pub url: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct MqttConfig {
    pub broker: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default = "default_mqtt_base_topic")]
    pub base_topic: String,
}

#[derive(Debug, Deserialize)]
pub struct KnxConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_knx_connection")]
    pub connection: String,
    pub gateway: Option<String>,
    #[serde(default = "default_knx_multicast")]
    pub multicast: String,
}

impl Default for KnxConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            connection: default_knx_connection(),
            gateway: None,
            multicast: default_knx_multicast(),
        }
    }
}

// ── Raw zone/client/radio (user-facing, optional fields) ──────

#[derive(Debug, Deserialize)]
pub struct RawZoneConfig {
    pub name: String,
    #[serde(default = "default_zone_icon")]
    pub icon: String,
    pub sink: Option<String>,
    #[serde(default)]
    pub knx: RawZoneKnxConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawZoneKnxConfig {
    pub play: Option<String>,
    pub pause: Option<String>,
    pub stop: Option<String>,
    pub volume: Option<String>,
    pub volume_status: Option<String>,
    pub volume_dim: Option<String>,
    pub mute: Option<String>,
    pub mute_status: Option<String>,
    pub mute_toggle: Option<String>,
    pub track_next: Option<String>,
    pub track_previous: Option<String>,
    pub playlist: Option<String>,
    pub playlist_status: Option<String>,
    pub shuffle: Option<String>,
    pub shuffle_status: Option<String>,
    pub repeat: Option<String>,
    pub repeat_status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RawClientConfig {
    pub name: String,
    pub mac: String,
    pub zone: String,
    #[serde(default = "default_client_icon")]
    pub icon: String,
    #[serde(default)]
    pub knx: RawClientKnxConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawClientKnxConfig {
    pub volume: Option<String>,
    pub volume_status: Option<String>,
    pub volume_dim: Option<String>,
    pub mute: Option<String>,
    pub mute_status: Option<String>,
    pub mute_toggle: Option<String>,
    pub zone: Option<String>,
    pub zone_status: Option<String>,
    pub connected_status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RawRadioConfig {
    pub name: String,
    pub url: String,
    pub cover: Option<String>,
}

// ── Resolved config (fully populated, no Option) ──────────────

/// Fully resolved application configuration. All conventions applied.
#[derive(Debug)]
pub struct AppConfig {
    pub system: SystemConfig,
    pub audio: AudioConfig,
    pub http: HttpConfig,
    pub snapcast: SnapcastConfig,
    pub airplay: AirplayConfig,
    pub subsonic: Option<SubsonicConfig>,
    pub mqtt: Option<MqttConfig>,
    pub knx: KnxConfig,
    pub zones: Vec<ZoneConfig>,
    pub clients: Vec<ClientConfig>,
    pub radios: Vec<RadioConfig>,
}

#[derive(Debug)]
pub struct ZoneConfig {
    pub index: usize,
    pub name: String,
    pub icon: String,
    pub sink: String,
    pub stream_name: String,
    pub tcp_source_port: u16,
    pub knx: ZoneKnxAddresses,
}

#[derive(Debug)]
pub struct ZoneKnxAddresses {
    pub play: String,
    pub pause: String,
    pub stop: String,
    pub volume: String,
    pub volume_status: String,
    pub volume_dim: String,
    pub mute: String,
    pub mute_status: String,
    pub mute_toggle: String,
    pub track_next: String,
    pub track_previous: String,
    pub playlist: String,
    pub playlist_status: String,
    pub shuffle: String,
    pub shuffle_status: String,
    pub repeat: String,
    pub repeat_status: String,
}

#[derive(Debug)]
pub struct ClientConfig {
    pub index: usize,
    pub name: String,
    pub mac: String,
    pub zone_index: usize,
    pub icon: String,
    pub knx: ClientKnxAddresses,
}

#[derive(Debug)]
pub struct ClientKnxAddresses {
    pub volume: String,
    pub volume_status: String,
    pub volume_dim: String,
    pub mute: String,
    pub mute_status: String,
    pub mute_toggle: String,
    pub zone: String,
    pub zone_status: String,
    pub connected_status: String,
}

#[derive(Debug)]
pub struct RadioConfig {
    pub name: String,
    pub url: String,
    pub cover: Option<String>,
}

impl From<RawRadioConfig> for RadioConfig {
    fn from(raw: RawRadioConfig) -> Self {
        Self {
            name: raw.name,
            url: raw.url,
            cover: raw.cover,
        }
    }
}

// ── Defaults ──────────────────────────────────────────────────

fn default_log_level() -> String {
    "info".into()
}
fn default_sample_rate() -> u32 {
    48000
}
fn default_bit_depth() -> u16 {
    16
}
fn default_channels() -> u16 {
    2
}
fn default_codec() -> String {
    "flac".into()
}
fn default_http_port() -> u16 {
    5555
}
fn default_snapcast_address() -> String {
    "127.0.0.1".into()
}
fn default_jsonrpc_port() -> u16 {
    1780
}
fn default_streaming_port() -> u16 {
    1704
}
fn default_airplay_name() -> String {
    "SnapDog".into()
}
fn default_mqtt_base_topic() -> String {
    "snapdog/".into()
}
fn default_knx_connection() -> String {
    "tunnel".into()
}
fn default_knx_multicast() -> String {
    "224.0.23.12".into()
}
fn default_zone_icon() -> String {
    "🎵".into()
}
fn default_client_icon() -> String {
    "🎵".into()
}
fn default_true() -> bool {
    true
}
