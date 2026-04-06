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
    pub spotify: Option<SpotifyConfig>,
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

#[derive(Debug, Deserialize, Clone)]
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

#[derive(Debug, Deserialize, Clone)]
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

#[derive(Debug, Deserialize, Clone)]
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

#[derive(Debug, Deserialize, Clone)]
pub struct AirplayConfig {
    pub password: Option<String>,
    /// Path to persist AirPlay pairing keys (required for AP2 reconnects).
    pub pairing_store: Option<std::path::PathBuf>,
    /// Bind to specific addresses (default: all interfaces).
    pub bind: Option<Vec<std::net::IpAddr>>,
}

impl Default for AirplayConfig {
    fn default() -> Self {
        Self {
            password: None,
            pairing_store: None,
            bind: None,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct SpotifyConfig {
    /// Device name shown in Spotify app (e.g., "SnapDog Ground Floor").
    pub name: String,
    /// Audio bitrate: 96, 160, or 320 kbps. Default: 320.
    #[serde(default = "default_spotify_bitrate")]
    pub bitrate: u32,
}

impl SpotifyConfig {
    /// Stable device ID derived from the name (for Zeroconf).
    pub fn device_id(&self) -> String {
        format!("{:x}", md5::compute(self.name.as_bytes()))
    }
}

fn default_spotify_bitrate() -> u32 {
    320
}

#[derive(Debug, Deserialize, Clone)]
pub struct SubsonicConfig {
    pub url: String,
    pub username: String,
    pub password: String,
    /// Stream format: "raw" (original file), "flac", "mp3", "opus".
    /// Default: "flac" (lossless, streamable, no buffering delay).
    #[serde(default = "default_subsonic_format")]
    pub format: String,
}

fn default_subsonic_format() -> String {
    "flac".into()
}

#[derive(Debug, Deserialize, Clone)]
pub struct MqttConfig {
    pub broker: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default = "default_mqtt_base_topic")]
    pub base_topic: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct KnxConfig {
    #[serde(default)]
    pub enabled: bool,
    /// KNX connection URL. Unicast = tunnel, multicast = router.
    /// Examples: `udp://192.168.1.50:3671`, `udp://224.0.23.12:3671`
    pub url: Option<String>,
}

impl Default for KnxConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: None,
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
    pub airplay_name: Option<String>,
    #[serde(default)]
    pub knx: RawZoneKnxConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct RawZoneKnxConfig {
    pub play: Option<String>,
    pub pause: Option<String>,
    pub stop: Option<String>,
    pub track_next: Option<String>,
    pub track_previous: Option<String>,
    pub control_status: Option<String>,
    pub volume: Option<String>,
    pub volume_status: Option<String>,
    pub volume_dim: Option<String>,
    pub mute: Option<String>,
    pub mute_status: Option<String>,
    pub mute_toggle: Option<String>,
    pub track_title_status: Option<String>,
    pub track_artist_status: Option<String>,
    pub track_album_status: Option<String>,
    pub track_progress_status: Option<String>,
    pub track_playing_status: Option<String>,
    pub track_repeat: Option<String>,
    pub track_repeat_status: Option<String>,
    pub track_repeat_toggle: Option<String>,
    pub playlist: Option<String>,
    pub playlist_status: Option<String>,
    pub playlist_next: Option<String>,
    pub playlist_previous: Option<String>,
    pub shuffle: Option<String>,
    pub shuffle_status: Option<String>,
    pub shuffle_toggle: Option<String>,
    pub repeat: Option<String>,
    pub repeat_status: Option<String>,
    pub repeat_toggle: Option<String>,
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
    pub latency: Option<String>,
    pub latency_status: Option<String>,
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
#[derive(Debug, Clone)]
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

/// Result of resolving a unified playlist index.
#[derive(Debug, Clone)]
pub enum ResolvedPlaylist {
    /// Index 0 when radios are configured.
    Radio,
    /// A Subsonic playlist, with its position in the Subsonic list.
    Subsonic(usize),
}

impl AppConfig {
    /// Whether the radio "playlist" occupies index 0.
    pub fn has_radio_playlist(&self) -> bool {
        !self.radios.is_empty()
    }

    /// Total number of unified playlists (radio + subsonic).
    pub fn unified_playlist_count(&self, subsonic_count: usize) -> usize {
        (if self.has_radio_playlist() { 1 } else { 0 }) + subsonic_count
    }

    /// Resolve a unified playlist index to radio or a Subsonic playlist offset.
    pub fn resolve_playlist_index(
        &self,
        index: usize,
        subsonic_count: usize,
    ) -> Option<ResolvedPlaylist> {
        let has_radio = self.has_radio_playlist();
        let total = self.unified_playlist_count(subsonic_count);
        if index >= total {
            return None;
        }
        if has_radio && index == 0 {
            Some(ResolvedPlaylist::Radio)
        } else {
            let sub_idx = if has_radio { index - 1 } else { index };
            Some(ResolvedPlaylist::Subsonic(sub_idx))
        }
    }
}

#[derive(Debug, Clone)]
pub struct ZoneConfig {
    pub index: usize,
    pub name: String,
    pub icon: String,
    pub sink: String,
    pub stream_name: String,
    pub tcp_source_port: u16,
    pub airplay_name: String,
    pub knx: ZoneKnxAddresses,
}

#[derive(Debug, Clone)]
pub struct ZoneKnxAddresses {
    pub play: Option<String>,
    pub pause: Option<String>,
    pub stop: Option<String>,
    pub track_next: Option<String>,
    pub track_previous: Option<String>,
    pub control_status: Option<String>,
    pub volume: Option<String>,
    pub volume_status: Option<String>,
    pub volume_dim: Option<String>,
    pub mute: Option<String>,
    pub mute_status: Option<String>,
    pub mute_toggle: Option<String>,
    pub track_title_status: Option<String>,
    pub track_artist_status: Option<String>,
    pub track_album_status: Option<String>,
    pub track_progress_status: Option<String>,
    pub track_playing_status: Option<String>,
    pub track_repeat: Option<String>,
    pub track_repeat_status: Option<String>,
    pub track_repeat_toggle: Option<String>,
    pub playlist: Option<String>,
    pub playlist_status: Option<String>,
    pub playlist_next: Option<String>,
    pub playlist_previous: Option<String>,
    pub shuffle: Option<String>,
    pub shuffle_status: Option<String>,
    pub shuffle_toggle: Option<String>,
    pub repeat: Option<String>,
    pub repeat_status: Option<String>,
    pub repeat_toggle: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub index: usize,
    pub name: String,
    pub mac: String,
    pub zone_index: usize,
    pub icon: String,
    pub knx: ClientKnxAddresses,
}

#[derive(Debug, Clone)]
pub struct ClientKnxAddresses {
    pub volume: Option<String>,
    pub volume_status: Option<String>,
    pub volume_dim: Option<String>,
    pub mute: Option<String>,
    pub mute_status: Option<String>,
    pub mute_toggle: Option<String>,
    pub latency: Option<String>,
    pub latency_status: Option<String>,
    pub zone: Option<String>,
    pub zone_status: Option<String>,
    pub connected_status: Option<String>,
}

#[derive(Debug, Clone)]
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
fn default_mqtt_base_topic() -> String {
    "snapdog/".into()
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
