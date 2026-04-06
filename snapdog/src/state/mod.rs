// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Application state management.
//!
//! In-memory state for zones and clients, persisted to JSON file.
//! Thread-safe via `Arc<RwLock<_>>` for concurrent access from API, MQTT, audio pipeline.

pub mod cover;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::config::AppConfig;

/// Thread-safe shared state handle.
pub type SharedState = Arc<RwLock<Store>>;

/// Create a new state store initialized from config, optionally loading persisted state.
pub fn init(config: &AppConfig, persist_path: Option<&Path>) -> Result<SharedState> {
    let mut store = Store::from_config(config);

    if let Some(path) = persist_path {
        if path.exists() {
            store.load(path)?;
            tracing::info!(path = %path.display(), "Restored persisted state");
        }
    }

    Ok(Arc::new(RwLock::new(store)))
}

/// Central application state.
#[derive(Debug, Serialize, Deserialize)]
pub struct Store {
    pub zones: HashMap<usize, ZoneState>,
    pub clients: HashMap<usize, ClientState>,
    #[serde(skip)]
    persist_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneState {
    pub name: String,
    pub icon: String,
    pub volume: i32,
    pub muted: bool,
    pub playback: PlaybackState,
    pub shuffle: bool,
    pub repeat: bool,
    pub track_repeat: bool,
    pub track: Option<TrackInfo>,
    pub playlist_index: Option<usize>,
    pub playlist_name: Option<String>,
    pub playlist_track_index: Option<usize>,
    pub playlist_track_count: Option<usize>,
    pub source: SourceType,
    pub cover_url: Option<String>,
    pub snapcast_group_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientState {
    pub name: String,
    pub icon: String,
    pub mac: String,
    pub zone_index: usize,
    pub volume: i32,
    pub muted: bool,
    pub latency_ms: i32,
    pub connected: bool,
    pub snapcast_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

impl std::fmt::Display for PlaybackState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped => write!(f, "stopped"),
            Self::Playing => write!(f, "playing"),
            Self::Paused => write!(f, "paused"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    #[default]
    Idle,
    Radio,
    SubsonicPlaylist,
    SubsonicTrack,
    Url,
    AirPlay,
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Radio => write!(f, "radio"),
            Self::SubsonicPlaylist => write!(f, "subsonic_playlist"),
            Self::SubsonicTrack => write!(f, "subsonic_track"),
            Self::Url => write!(f, "url"),
            Self::AirPlay => write!(f, "airplay"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    // Metadata
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u32>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,

    // Playback
    pub duration_ms: i64,
    pub position_ms: i64,
    pub seekable: bool,
    pub source: SourceType,

    // Technical
    pub bitrate_kbps: Option<u32>,
    pub content_type: Option<String>,
    pub sample_rate: Option<u32>,
}

impl Store {
    /// Initialize state from config with default values.
    fn from_config(config: &AppConfig) -> Self {
        let zones = config
            .zones
            .iter()
            .map(|z| {
                (
                    z.index,
                    ZoneState {
                        name: z.name.clone(),
                        icon: z.icon.clone(),
                        volume: 50,
                        muted: false,
                        playback: PlaybackState::Stopped,
                        shuffle: false,
                        repeat: false,
                        track_repeat: false,
                        track: None,
                        playlist_index: None,
                        playlist_name: None,
                        playlist_track_index: None,
                        playlist_track_count: None,
                        source: SourceType::Idle,
                        cover_url: None,
                        snapcast_group_id: None,
                    },
                )
            })
            .collect();

        let clients = config
            .clients
            .iter()
            .map(|c| {
                (
                    c.index,
                    ClientState {
                        name: c.name.clone(),
                        icon: c.icon.clone(),
                        mac: c.mac.clone(),
                        zone_index: c.zone_index,
                        volume: 50,
                        muted: false,
                        latency_ms: 0,
                        connected: false,
                        snapcast_id: None,
                    },
                )
            })
            .collect();

        Self {
            zones,
            clients,
            persist_path: None,
        }
    }

    /// Set the persistence path and enable auto-save.
    pub fn set_persist_path(&mut self, path: PathBuf) {
        self.persist_path = Some(path);
    }

    /// Persist current state to JSON file (atomic write).
    /// Persist state to JSON file. Uses blocking I/O (called infrequently on state changes).
    pub fn persist(&self) -> Result<()> {
        let Some(path) = &self.persist_path else {
            return Ok(());
        };
        let tmp = path.with_extension("tmp");
        let json = serde_json::to_string_pretty(self).context("Failed to serialize state")?;
        std::fs::write(&tmp, &json)
            .with_context(|| format!("Failed to write {}", tmp.display()))?;
        std::fs::rename(&tmp, path)
            .with_context(|| format!("Failed to rename to {}", path.display()))?;
        tracing::debug!(path = %path.display(), "State persisted");
        Ok(())
    }

    /// Load state from JSON file, merging with current config-derived state.
    fn load(&mut self, path: &Path) -> Result<()> {
        let json = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let saved: Store = serde_json::from_str(&json)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        // Merge: only restore runtime state for zones/clients that still exist in config
        for (idx, saved_zone) in saved.zones {
            if let Some(zone) = self.zones.get_mut(&idx) {
                zone.volume = saved_zone.volume;
                zone.muted = saved_zone.muted;
                zone.shuffle = saved_zone.shuffle;
                zone.repeat = saved_zone.repeat;
                zone.track_repeat = saved_zone.track_repeat;
                zone.playlist_index = saved_zone.playlist_index;
                zone.playlist_name = saved_zone.playlist_name;
                // Don't restore playback/track — those are transient
            }
        }

        for (idx, saved_client) in saved.clients {
            if let Some(client) = self.clients.get_mut(&idx) {
                client.volume = saved_client.volume;
                client.muted = saved_client.muted;
                client.latency_ms = saved_client.latency_ms;
                client.zone_index = saved_client.zone_index;
                // Don't restore connected/snapcast_id — those are transient
            }
        }

        self.persist_path = Some(path.to_path_buf());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> AppConfig {
        let raw: crate::config::RawConfig = toml::from_str(
            r#"
            [[zone]]
            name = "Ground Floor"
            [[client]]
            name = "Living Room"
            mac = "00:00:00:00:00:00"
            zone = "Ground Floor"
        "#,
        )
        .unwrap();
        crate::config::load_raw(raw).unwrap()
    }

    #[test]
    fn initializes_from_config() {
        let config = test_config();
        let store = Store::from_config(&config);
        assert_eq!(store.zones.len(), 1);
        assert_eq!(store.clients.len(), 1);
        assert_eq!(store.zones[&1].name, "Ground Floor");
        assert_eq!(store.zones[&1].volume, 50);
        assert_eq!(store.clients[&1].zone_index, 1);
    }

    #[test]
    fn persist_and_load_roundtrip() {
        let config = test_config();
        let mut store = Store::from_config(&config);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        store.set_persist_path(path.clone());

        // Modify state
        store.zones.get_mut(&1).unwrap().volume = 80;
        store.clients.get_mut(&1).unwrap().muted = true;
        store.persist().unwrap();

        // Load into fresh store
        let mut store2 = Store::from_config(&config);
        store2.load(&path).unwrap();
        assert_eq!(store2.zones[&1].volume, 80);
        assert!(store2.clients[&1].muted);
    }

    #[test]
    fn load_ignores_removed_zones() {
        let config = test_config();
        let mut store = Store::from_config(&config);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        store.set_persist_path(path.clone());

        // Add a fake zone that doesn't exist in config
        store.zones.insert(
            99,
            ZoneState {
                name: "Ghost".into(),
                icon: "👻".into(),
                volume: 100,
                muted: false,
                playback: PlaybackState::Stopped,
                shuffle: false,
                repeat: false,
                track_repeat: false,
                track: None,
                playlist_index: None,
                playlist_name: None,
                playlist_track_index: None,
                playlist_track_count: None,
                source: SourceType::Idle,
                cover_url: None,
                snapcast_group_id: None,
            },
        );
        store.persist().unwrap();

        // Load into fresh config-derived store — zone 99 should not appear
        let mut store2 = Store::from_config(&config);
        store2.load(&path).unwrap();
        assert!(!store2.zones.contains_key(&99));
    }

    #[test]
    fn does_not_restore_transient_state() {
        let config = test_config();
        let mut store = Store::from_config(&config);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        store.set_persist_path(path.clone());

        store.zones.get_mut(&1).unwrap().playback = PlaybackState::Playing;
        store.clients.get_mut(&1).unwrap().connected = true;
        store.persist().unwrap();

        let mut store2 = Store::from_config(&config);
        store2.load(&path).unwrap();
        // Transient state should NOT be restored
        assert_eq!(store2.zones[&1].playback, PlaybackState::Stopped);
        assert!(!store2.clients[&1].connected);
    }
}

/// Update client state and broadcast a notification.
pub async fn update_client_and_notify(
    store: &SharedState,
    client_index: usize,
    notify: &tokio::sync::broadcast::Sender<crate::api::ws::Notification>,
    f: impl FnOnce(&mut ClientState),
) {
    let notification = {
        let mut s = store.write().await;
        let Some(client) = s.clients.get_mut(&client_index) else {
            return;
        };
        f(client);
        crate::api::ws::Notification::ClientStateChanged {
            client: client_index,
            volume: client.volume,
            muted: client.muted,
            connected: client.connected,
            zone: client.zone_index,
        }
    };
    let _ = notify.send(notification);
}
