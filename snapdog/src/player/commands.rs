// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! ZonePlayer command definitions.

/// Commands sent to a ZonePlayer from API, MQTT, or KNX.
#[derive(Debug)]
pub enum ZoneCommand {
    // ── Source selection ───────────────────────────────────────
    PlaySubsonicPlaylist(String, usize), // playlist_id, start_track
    PlaySubsonicTrack(String),           // track_id
    PlayUrl(String),
    SetTrack(usize), // jump to track N in current playlist

    // ── Transport ─────────────────────────────────────────────
    Play, // resume or restart current source
    Pause,
    Stop,
    Next,     // next track (playlist) or next station (radio)
    Previous, // previous track/station

    // ── Playlist navigation ───────────────────────────────────
    NextPlaylist,
    PreviousPlaylist,
    SetPlaylist(usize),

    // ── Seek ──────────────────────────────────────────────────
    Seek(i64),         // absolute position in ms
    SeekProgress(f64), // relative 0.0..1.0

    // ── Zone settings ─────────────────────────────────────────
    SetVolume(i32),
    SetMute(bool),
    ToggleMute,
    SetShuffle(bool),
    ToggleShuffle,
    SetRepeat(bool),
    ToggleRepeat,
    SetTrackRepeat(bool),
    ToggleTrackRepeat,
}

/// What the ZonePlayer is currently doing.
#[derive(Debug, Clone)]
pub enum ActiveSource {
    Idle,
    Radio {
        index: usize,
    },
    SubsonicPlaylist {
        playlist_id: String,
        track_index: usize,
        track_count: usize,
    },
    SubsonicTrack {
        track_id: String,
    },
    Url,
    AirPlay,
}
