// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Configuration types: raw TOML structs and resolved application config.

use serde::Deserialize;

// ── Raw TOML structs (what the user writes) ───────────────────

/// Root of the TOML config file. Optional fields use defaults.
#[derive(Debug, Deserialize)]
pub struct RawConfig {
    /// System settings (log level, log file).
    #[serde(default)]
    pub system: SystemConfig,
    /// Audio output format (sample rate, bit depth, channels, codec).
    #[serde(default)]
    pub audio: AudioConfig,
    /// HTTP server settings (port, API keys).
    #[serde(default)]
    pub http: HttpConfig,
    /// Snapcast connection and management settings.
    #[serde(default)]
    pub snapcast: SnapcastConfig,
    /// AirPlay receiver settings.
    #[serde(default)]
    pub airplay: AirplayConfig,
    /// Subsonic/Navidrome server connection.
    #[serde(default)]
    pub subsonic: Option<SubsonicConfig>,
    /// Spotify Connect receiver settings.
    #[serde(default)]
    pub spotify: Option<SpotifyConfig>,
    /// MQTT bridge settings.
    #[serde(default)]
    pub mqtt: Option<MqttConfig>,
    /// KNX/IP integration settings.
    #[serde(default)]
    pub knx: KnxConfig,
    /// Zone definitions.
    #[serde(default)]
    pub zone: Vec<RawZoneConfig>,
    /// Client (speaker) definitions.
    #[serde(default)]
    pub client: Vec<RawClientConfig>,
    /// Radio station definitions.
    #[serde(default)]
    pub radio: Vec<RawRadioConfig>,
}

/// System-level settings.
#[derive(Debug, Deserialize, Clone)]
pub struct SystemConfig {
    /// Tracing log level: trace, debug, info, warn, error.
    #[serde(default = "default_log_level")]
    pub log_level: String,
    /// Optional log file path (daily rotation).
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

/// Audio output format — SSOT with Snapcast stream configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct AudioConfig {
    /// Output sample rate in Hz (e.g., 44100, 48000, 96000).
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    /// Output bit depth: 16, 24, or 32.
    #[serde(default = "default_bit_depth")]
    pub bit_depth: u16,
    /// Number of audio channels (typically 2 for stereo).
    #[serde(default = "default_channels")]
    pub channels: u16,
    /// Snapcast codec: "flac", "pcm", "f32lz4", "f32lz4e", "opus", "ogg".
    #[serde(default = "default_codec")]
    pub codec: String,
    /// Pre-shared key for f32lz4e encryption (default: built-in key).
    #[serde(default)]
    pub encryption_psk: Option<String>,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: default_sample_rate(),
            bit_depth: default_bit_depth(),
            channels: default_channels(),
            codec: default_codec(),
            encryption_psk: None,
        }
    }
}

impl AudioConfig {
    /// Snapcast sample format string (e.g., "48000:16:2").
    pub fn sample_format(&self) -> String {
        format!("{}:{}:{}", self.sample_rate, self.bit_depth, self.channels)
    }
}

/// HTTP server configuration.
#[derive(Debug, Deserialize, Clone)]
pub struct HttpConfig {
    /// Port for the REST API, WebSocket, and embedded WebUI.
    #[serde(default = "default_http_port")]
    pub port: u16,
    /// Optional API keys. If set, all API endpoints require `Authorization: Bearer <key>`.
    #[serde(default)]
    pub api_keys: Vec<String>,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            port: default_http_port(),
            api_keys: vec![],
        }
    }
}

/// Snapcast server connection and management.
#[derive(Debug, Deserialize, Clone)]
pub struct SnapcastConfig {
    /// Snapcast server address (hostname or IP).
    #[serde(default = "default_snapcast_address")]
    pub address: String,
    /// TCP JSON-RPC control port (default: 1705).
    #[serde(default = "default_jsonrpc_port")]
    pub jsonrpc_port: u16,
    /// Audio streaming port (default: 1704).
    #[serde(default = "default_streaming_port")]
    pub streaming_port: u16,
    /// Start snapserver as a managed child process.
    #[serde(default = "default_true")]
    pub managed: bool,
    /// Show snapserver console output.
    #[serde(default)]
    pub verbose: bool,
}

impl Default for SnapcastConfig {
    fn default() -> Self {
        Self {
            address: default_snapcast_address(),
            jsonrpc_port: default_jsonrpc_port(),
            streaming_port: default_streaming_port(),
            managed: true,
            verbose: false,
        }
    }
}

/// AirPlay receiver settings (shared across all zones).
#[derive(Debug, Deserialize, Clone, Default)]
pub struct AirplayConfig {
    /// Optional password for AirPlay connections.
    pub password: Option<String>,
    /// Path to persist AirPlay pairing keys (required for AP2 reconnects).
    pub pairing_store: Option<std::path::PathBuf>,
    /// Bind to specific addresses (default: all interfaces).
    pub bind: Option<Vec<std::net::IpAddr>>,
}

/// Spotify Connect receiver settings.
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

/// Subsonic/Navidrome server connection.
#[derive(Debug, Deserialize, Clone)]
pub struct SubsonicConfig {
    /// Server base URL (e.g., `https://music.example.com`).
    pub url: String,
    /// Authentication username.
    pub username: String,
    /// Authentication password.
    pub password: String,
    /// Stream format: "raw" (original file), "flac", "mp3", "opus".
    /// Default: "flac" (lossless, streamable, no buffering delay).
    #[serde(default = "default_subsonic_format")]
    pub format: String,
    /// Skip TLS certificate verification (for self-signed certs).
    #[serde(default)]
    pub tls_skip_verify: bool,
}

fn default_subsonic_format() -> String {
    "flac".into()
}

/// MQTT bridge configuration.
#[derive(Debug, Deserialize, Clone)]
pub struct MqttConfig {
    /// Broker address (host:port).
    pub broker: String,
    /// MQTT username (empty for anonymous).
    #[serde(default)]
    pub username: String,
    /// MQTT password.
    #[serde(default)]
    pub password: String,
    /// Base topic prefix (e.g., "snapdog/").
    #[serde(default = "default_mqtt_base_topic")]
    pub base_topic: String,
}

/// KNX/IP integration settings.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct KnxConfig {
    /// Enable KNX integration.
    #[serde(default)]
    pub enabled: bool,
    /// KNX connection URL. Unicast = tunnel, multicast = router.
    /// Examples: `udp://192.168.1.50:3671`, `udp://224.0.23.12:3671`
    pub url: Option<String>,
}

// ── Raw zone/client/radio (user-facing, optional fields) ──────

/// Zone definition as written in TOML.
#[derive(Debug, Deserialize)]
pub struct RawZoneConfig {
    /// Human-readable zone name (also used as AirPlay name).
    pub name: String,
    /// Emoji icon for the zone.
    #[serde(default = "default_zone_icon")]
    pub icon: String,
    /// Override Snapcast sink path (default: auto-generated).
    pub sink: Option<String>,
    /// Override AirPlay receiver name (default: zone name).
    pub airplay_name: Option<String>,
    /// KNX group addresses for this zone.
    #[serde(default)]
    pub knx: RawZoneKnxConfig,
}

/// KNX group addresses for zone control (all optional, explicit config only).
///
/// Each field is a KNX group address string (e.g. "1/2/3") mapped to a zone function.
#[derive(Debug, Default, Deserialize)]
pub struct RawZoneKnxConfig {
    /// Play command.
    pub play: Option<String>,
    /// Pause command.
    pub pause: Option<String>,
    /// Stop command.
    pub stop: Option<String>,
    /// Next track command.
    pub track_next: Option<String>,
    /// Previous track command.
    pub track_previous: Option<String>,
    /// Playback status feedback.
    pub control_status: Option<String>,
    /// Volume set (DPT 5.001 scaling).
    pub volume: Option<String>,
    /// Volume status feedback.
    pub volume_status: Option<String>,
    /// Relative volume dimming (DPT 3.007).
    pub volume_dim: Option<String>,
    /// Mute command.
    pub mute: Option<String>,
    /// Mute status feedback.
    pub mute_status: Option<String>,
    /// Mute toggle command.
    pub mute_toggle: Option<String>,
    /// Track title status feedback (DPT 16.001).
    pub track_title_status: Option<String>,
    /// Track artist status feedback (DPT 16.001).
    pub track_artist_status: Option<String>,
    /// Track album status feedback (DPT 16.001).
    pub track_album_status: Option<String>,
    /// Track progress status feedback (percentage).
    pub track_progress_status: Option<String>,
    /// Track playing status feedback (boolean).
    pub track_playing_status: Option<String>,
    /// Single-track repeat command.
    pub track_repeat: Option<String>,
    /// Single-track repeat status feedback.
    pub track_repeat_status: Option<String>,
    /// Single-track repeat toggle command.
    pub track_repeat_toggle: Option<String>,
    /// Playlist selection command (index).
    pub playlist: Option<String>,
    /// Playlist selection status feedback.
    pub playlist_status: Option<String>,
    /// Next playlist command.
    pub playlist_next: Option<String>,
    /// Previous playlist command.
    pub playlist_previous: Option<String>,
    /// Shuffle command.
    pub shuffle: Option<String>,
    /// Shuffle status feedback.
    pub shuffle_status: Option<String>,
    /// Shuffle toggle command.
    pub shuffle_toggle: Option<String>,
    /// Playlist repeat command.
    pub repeat: Option<String>,
    /// Playlist repeat status feedback.
    pub repeat_status: Option<String>,
    /// Playlist repeat toggle command.
    pub repeat_toggle: Option<String>,
}

/// Client (speaker) definition as written in TOML.
#[derive(Debug, Deserialize)]
pub struct RawClientConfig {
    /// Human-readable client name.
    pub name: String,
    /// Snapcast client MAC address (used to match `host.mac`).
    pub mac: String,
    /// Zone name this client belongs to.
    pub zone: String,
    /// Emoji icon for the client.
    #[serde(default = "default_client_icon")]
    pub icon: String,
    /// KNX group addresses for this client.
    #[serde(default)]
    pub knx: RawClientKnxConfig,
}

/// KNX group addresses for client control (all optional, explicit config only).
///
/// Each field is a KNX group address string (e.g. "1/2/3") mapped to a client function.
#[derive(Debug, Default, Deserialize)]
pub struct RawClientKnxConfig {
    /// Volume set (DPT 5.001 scaling).
    pub volume: Option<String>,
    /// Volume status feedback.
    pub volume_status: Option<String>,
    /// Relative volume dimming (DPT 3.007).
    pub volume_dim: Option<String>,
    /// Mute command.
    pub mute: Option<String>,
    /// Mute status feedback.
    pub mute_status: Option<String>,
    /// Mute toggle command.
    pub mute_toggle: Option<String>,
    /// Latency set (milliseconds).
    pub latency: Option<String>,
    /// Latency status feedback.
    pub latency_status: Option<String>,
    /// Zone assignment command.
    pub zone: Option<String>,
    /// Zone assignment status feedback.
    pub zone_status: Option<String>,
    /// Connection status feedback.
    pub connected_status: Option<String>,
}

/// Radio station definition.
#[derive(Debug, Deserialize)]
pub struct RawRadioConfig {
    /// Station name.
    pub name: String,
    /// Stream URL (direct, M3U, PLS, or HLS).
    pub url: String,
    /// Cover art URL.
    pub cover: Option<String>,
}

// ── Resolved config (fully populated, no Option) ──────────────

/// Fully resolved application configuration. All conventions applied.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// System settings.
    pub system: SystemConfig,
    /// Audio output format.
    pub audio: AudioConfig,
    /// HTTP server settings.
    pub http: HttpConfig,
    /// Snapcast connection settings.
    pub snapcast: SnapcastConfig,
    /// AirPlay receiver settings.
    pub airplay: AirplayConfig,
    /// Subsonic connection (if configured).
    pub subsonic: Option<SubsonicConfig>,
    /// MQTT bridge (if configured).
    pub mqtt: Option<MqttConfig>,
    /// KNX settings.
    pub knx: KnxConfig,
    /// Resolved zone configurations (1-indexed).
    pub zones: Vec<ZoneConfig>,
    /// Resolved client configurations (1-indexed).
    pub clients: Vec<ClientConfig>,
    /// Radio station list.
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

/// Resolved zone configuration with all conventions applied.
#[derive(Debug, Clone)]
pub struct ZoneConfig {
    /// Zone index (1-based).
    pub index: usize,
    /// Zone name.
    pub name: String,
    /// Emoji icon.
    pub icon: String,
    /// Snapcast sink path (e.g., `/snapsinks/zone1`).
    pub sink: String,
    /// Snapcast stream name (e.g., `Zone1`).
    pub stream_name: String,
    /// TCP source port for audio data to snapserver.
    pub tcp_source_port: u16,
    /// AirPlay receiver name (shown in AirPlay picker).
    pub airplay_name: String,
    /// KNX group addresses.
    pub knx: ZoneKnxAddresses,
}

/// Resolved KNX group addresses for a zone (all optional, explicit config only).
///
/// Same fields as [`RawZoneKnxConfig`] after address validation.
#[derive(Debug, Clone)]
pub struct ZoneKnxAddresses {
    /// Play command.
    pub play: Option<String>,
    /// Pause command.
    pub pause: Option<String>,
    /// Stop command.
    pub stop: Option<String>,
    /// Next track command.
    pub track_next: Option<String>,
    /// Previous track command.
    pub track_previous: Option<String>,
    /// Playback status feedback.
    pub control_status: Option<String>,
    /// Volume set (DPT 5.001 scaling).
    pub volume: Option<String>,
    /// Volume status feedback.
    pub volume_status: Option<String>,
    /// Relative volume dimming (DPT 3.007).
    pub volume_dim: Option<String>,
    /// Mute command.
    pub mute: Option<String>,
    /// Mute status feedback.
    pub mute_status: Option<String>,
    /// Mute toggle command.
    pub mute_toggle: Option<String>,
    /// Track title status feedback (DPT 16.001).
    pub track_title_status: Option<String>,
    /// Track artist status feedback (DPT 16.001).
    pub track_artist_status: Option<String>,
    /// Track album status feedback (DPT 16.001).
    pub track_album_status: Option<String>,
    /// Track progress status feedback (percentage).
    pub track_progress_status: Option<String>,
    /// Track playing status feedback (boolean).
    pub track_playing_status: Option<String>,
    /// Single-track repeat command.
    pub track_repeat: Option<String>,
    /// Single-track repeat status feedback.
    pub track_repeat_status: Option<String>,
    /// Single-track repeat toggle command.
    pub track_repeat_toggle: Option<String>,
    /// Playlist selection command (index).
    pub playlist: Option<String>,
    /// Playlist selection status feedback.
    pub playlist_status: Option<String>,
    /// Next playlist command.
    pub playlist_next: Option<String>,
    /// Previous playlist command.
    pub playlist_previous: Option<String>,
    /// Shuffle command.
    pub shuffle: Option<String>,
    /// Shuffle status feedback.
    pub shuffle_status: Option<String>,
    /// Shuffle toggle command.
    pub shuffle_toggle: Option<String>,
    /// Playlist repeat command.
    pub repeat: Option<String>,
    /// Playlist repeat status feedback.
    pub repeat_status: Option<String>,
    /// Playlist repeat toggle command.
    pub repeat_toggle: Option<String>,
}

/// Resolved client configuration.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Client index (1-based).
    pub index: usize,
    /// Client name.
    pub name: String,
    /// Snapcast client MAC address.
    pub mac: String,
    /// Zone index this client belongs to.
    pub zone_index: usize,
    /// Emoji icon.
    pub icon: String,
    /// KNX group addresses.
    pub knx: ClientKnxAddresses,
}

/// Resolved KNX group addresses for a client (all optional, explicit config only).
///
/// Same fields as [`RawClientKnxConfig`] after address validation.
#[derive(Debug, Clone)]
pub struct ClientKnxAddresses {
    /// Volume set (DPT 5.001 scaling).
    pub volume: Option<String>,
    /// Volume status feedback.
    pub volume_status: Option<String>,
    /// Relative volume dimming (DPT 3.007).
    pub volume_dim: Option<String>,
    /// Mute command.
    pub mute: Option<String>,
    /// Mute status feedback.
    pub mute_status: Option<String>,
    /// Mute toggle command.
    pub mute_toggle: Option<String>,
    /// Latency set (milliseconds).
    pub latency: Option<String>,
    /// Latency status feedback.
    pub latency_status: Option<String>,
    /// Zone assignment command.
    pub zone: Option<String>,
    /// Zone assignment status feedback.
    pub zone_status: Option<String>,
    /// Connection status feedback.
    pub connected_status: Option<String>,
}

/// Resolved radio station configuration.
#[derive(Debug, Clone)]
pub struct RadioConfig {
    /// Station name.
    pub name: String,
    /// Stream URL.
    pub url: String,
    /// Cover art URL.
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
    1705
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
