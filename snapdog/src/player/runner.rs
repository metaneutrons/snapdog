// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! ZonePlayer runner — the per-zone tokio task.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::commands::{ActiveSource, ZoneCommand};
use crate::audio;
use crate::config::AppConfig;
use crate::snapcast;
use crate::state::{self, PlaybackState, SourceType, TrackInfo};
use crate::subsonic::SubsonicClient;

/// Command sender handle — one per zone, shared with API/MQTT/KNX.
pub type ZoneCommandSender = mpsc::Sender<ZoneCommand>;

/// Spawn a ZonePlayer task for each configured zone. Returns command senders.
pub async fn spawn_zone_players(
    config: Arc<AppConfig>,
    store: state::SharedState,
) -> Result<HashMap<usize, ZoneCommandSender>> {
    let mut senders = HashMap::new();

    for zone in &config.zones {
        let (cmd_tx, cmd_rx) = mpsc::channel(32);
        senders.insert(zone.index, cmd_tx);

        let config = config.clone();
        let store = store.clone();
        let zone_index = zone.index;

        tokio::spawn(async move {
            if let Err(e) = run(zone_index, cmd_rx, config, store).await {
                tracing::error!(zone = zone_index, error = %e, "ZonePlayer crashed");
            }
        });

        tracing::info!(zone = zone.index, name = %zone.name, "ZonePlayer started");
    }

    Ok(senders)
}

/// Main ZonePlayer loop.
async fn run(
    zone_index: usize,
    mut commands: mpsc::Receiver<ZoneCommand>,
    config: Arc<AppConfig>,
    store: state::SharedState,
) -> Result<()> {
    let zone_config = &config.zones[zone_index - 1];

    // Connect TCP to Snapcast source
    let mut tcp = snapcast::open_audio_source(zone_config.tcp_source_port).await?;

    // Subsonic client (if configured)
    let subsonic = config.subsonic.as_ref().map(SubsonicClient::new);

    // AirPlay PCM channel (always listening)
    let (_airplay_tx, mut airplay_rx) = audio::pcm_channel(128);
    // TODO: start AirplayReceiver with _airplay_tx when implemented

    // Decode task state
    let mut current_decode: Option<JoinHandle<()>> = None;
    let mut decode_rx: Option<mpsc::Receiver<Vec<u8>>> = None;
    let mut source = ActiveSource::Idle;
    let source_rate = config.audio.sample_rate; // Updated when source starts
    let mut resampler = audio::resample::Resampling::new(
        source_rate,
        config.audio.sample_rate,
        config.audio.channels,
    );
    let mut airplay_resampler =
        audio::resample::Resampling::new(44100, config.audio.sample_rate, config.audio.channels);

    loop {
        tokio::select! {
            // ── Commands ──────────────────────────────────────
            Some(cmd) = commands.recv() => {
                match cmd {
                    // Source selection
                    ZoneCommand::PlayRadio(idx) => {
                        stop_decode(&mut current_decode, &mut decode_rx).await;
                        if let Some(radio) = config.radios.get(idx) {
                            let (tx, rx) = audio::pcm_channel(64);
                            decode_rx = Some(rx);
                            let url = radio.url.clone();
                            let ac = config.audio.clone();
                            current_decode = Some(tokio::spawn(async move {
                                if let Err(e) = audio::decode_http_stream(url, tx, ac).await {
                                    tracing::error!(error = %e, "Radio decode failed");
                                }
                            }));
                            source = ActiveSource::Radio { index: idx };
                            update_zone_state(&store, zone_index, |z| {
                                z.playback = PlaybackState::Playing;
                                z.source = SourceType::Radio;
                                z.radio_index = Some(idx);
                                z.track = Some(TrackInfo {
                                    title: radio.name.clone(),
                                    artist: "Radio".into(),
                                    album: String::new(),
                                    album_artist: None,
                                    genre: None,
                                    year: None,
                                    track_number: None,
                                    disc_number: None,
                                    duration_ms: 0,
                                    position_ms: 0,
                                    source: SourceType::Radio,
                                    bitrate_kbps: None,
                                    content_type: None,
                                    sample_rate: None,
                                });
                            }).await;
                            tracing::info!(zone = zone_index, radio = %radio.name, "Playing radio");
                        }
                    }

                    ZoneCommand::PlaySubsonicPlaylist(playlist_id, track_idx) => {
                        stop_decode(&mut current_decode, &mut decode_rx).await;
                        if let Some(sub) = &subsonic {
                            match sub.get_playlist(&playlist_id).await {
                                Ok(playlist) => {
                                    let track_count = playlist.entry.len();
                                    if let Some(track) = playlist.entry.get(track_idx) {
                                        start_subsonic_track_decode(
                                            sub, track, &config, &mut current_decode, &mut decode_rx,
                                        ).await;
                                        source = ActiveSource::SubsonicPlaylist {
                                            playlist_id: playlist_id.clone(),
                                            track_index: track_idx,
                                            track_count,
                                        };
                                        update_zone_state(&store, zone_index, |z| {
                                            z.playback = PlaybackState::Playing;
                                            z.source = SourceType::SubsonicPlaylist;
                                            z.playlist_index = Some(track_idx);
                                            z.playlist_name = Some(playlist.name.clone());
                                            z.playlist_track_index = Some(track_idx);
                                            z.playlist_track_count = Some(track_count);
                                            z.track = Some(subsonic_track_info(track));
                                        }).await;
                                        tracing::info!(zone = zone_index, playlist = %playlist.name, track = track_idx, "Playing subsonic playlist");
                                    }
                                }
                                Err(e) => tracing::error!(error = %e, "Failed to load playlist"),
                            }
                        }
                    }

                    ZoneCommand::PlaySubsonicTrack(track_id) => {
                        stop_decode(&mut current_decode, &mut decode_rx).await;
                        if let Some(sub) = &subsonic {
                            let url = sub.stream_url(&track_id);
                            let (tx, rx) = audio::pcm_channel(64);
                            decode_rx = Some(rx);
                            let ac = config.audio.clone();
                            current_decode = Some(tokio::spawn(async move {
                                if let Err(e) = audio::decode_http_stream(url, tx, ac).await {
                                    tracing::error!(error = %e, "Subsonic track decode failed");
                                }
                            }));
                            source = ActiveSource::SubsonicTrack { track_id };
                            update_zone_state(&store, zone_index, |z| {
                                z.playback = PlaybackState::Playing;
                                z.source = SourceType::SubsonicTrack;
                            }).await;
                        }
                    }

                    ZoneCommand::PlayUrl(url) => {
                        stop_decode(&mut current_decode, &mut decode_rx).await;
                        let (tx, rx) = audio::pcm_channel(64);
                        decode_rx = Some(rx);
                        let ac = config.audio.clone();
                        let url_clone = url.clone();
                        current_decode = Some(tokio::spawn(async move {
                            if let Err(e) = audio::decode_http_stream(url_clone, tx, ac).await {
                                tracing::error!(error = %e, "URL decode failed");
                            }
                        }));
                        source = ActiveSource::Url { url };
                        update_zone_state(&store, zone_index, |z| {
                            z.playback = PlaybackState::Playing;
                            z.source = SourceType::Url;
                        }).await;
                    }

                    ZoneCommand::SetTrack(track_idx) => {
                        if let ActiveSource::SubsonicPlaylist { ref playlist_id, track_count, .. } = source {
                            if track_idx < track_count {
                                let pid = playlist_id.clone();
                                // Re-send as PlaySubsonicPlaylist to reuse that logic
                                let _ = commands.try_recv(); // drain
                                // Inline the logic instead of re-sending
                                stop_decode(&mut current_decode, &mut decode_rx).await;
                                if let Some(sub) = &subsonic {
                                    if let Ok(playlist) = sub.get_playlist(&pid).await {
                                        if let Some(track) = playlist.entry.get(track_idx) {
                                            start_subsonic_track_decode(sub, track, &config, &mut current_decode, &mut decode_rx).await;
                                            source = ActiveSource::SubsonicPlaylist { playlist_id: pid, track_index: track_idx, track_count };
                                            update_zone_state(&store, zone_index, |z| {
                                                z.playlist_track_index = Some(track_idx);
                                                z.track = Some(subsonic_track_info(track));
                                            }).await;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Transport
                    ZoneCommand::Play => {
                        match &source {
                            ActiveSource::Idle => {
                                // Restart last radio if available
                                let radio_idx = store.read().await.zones.get(&zone_index).and_then(|z| z.radio_index).unwrap_or(0);
                                if !config.radios.is_empty() {
                                    // Will be handled next iteration
                                    let _ = commands.try_recv();
                                    // Directly start radio
                                    if let Some(radio) = config.radios.get(radio_idx) {
                                        stop_decode(&mut current_decode, &mut decode_rx).await;
                                        let (tx, rx) = audio::pcm_channel(64);
                                        decode_rx = Some(rx);
                                        let url = radio.url.clone();
                                        let ac = config.audio.clone();
                                        current_decode = Some(tokio::spawn(async move {
                                            if let Err(e) = audio::decode_http_stream(url, tx, ac).await {
                                                tracing::error!(error = %e, "Radio decode failed");
                                            }
                                        }));
                                        source = ActiveSource::Radio { index: radio_idx };
                                        update_zone_state(&store, zone_index, |z| {
                                            z.playback = PlaybackState::Playing;
                                            z.source = SourceType::Radio;
                                        }).await;
                                    }
                                }
                            }
                            _ => {
                                // Already playing — no-op
                            }
                        }
                    }

                    ZoneCommand::Pause => {
                        stop_decode(&mut current_decode, &mut decode_rx).await;
                        // Keep source info so Play can resume
                        update_zone_state(&store, zone_index, |z| {
                            z.playback = PlaybackState::Paused;
                        }).await;
                        tracing::info!(zone = zone_index, "Paused");
                    }

                    ZoneCommand::Stop => {
                        stop_decode(&mut current_decode, &mut decode_rx).await;
                        source = ActiveSource::Idle;
                        update_zone_state(&store, zone_index, |z| {
                            z.playback = PlaybackState::Stopped;
                            z.source = SourceType::Idle;
                            z.track = None;
                        }).await;
                        tracing::info!(zone = zone_index, "Stopped");
                    }

                    ZoneCommand::Next => {
                        handle_next(&mut source, &config, &subsonic, &store, zone_index, &mut current_decode, &mut decode_rx).await;
                    }

                    ZoneCommand::Previous => {
                        handle_previous(&mut source, &config, &subsonic, &store, zone_index, &mut current_decode, &mut decode_rx).await;
                    }

                    // Playlist navigation
                    ZoneCommand::NextPlaylist | ZoneCommand::PreviousPlaylist | ZoneCommand::SetPlaylist(_) => {
                        // TODO: requires playlist list from Subsonic
                        tracing::debug!(zone = zone_index, "Playlist navigation not yet implemented");
                    }

                    // Seek
                    ZoneCommand::Seek(_) | ZoneCommand::SeekProgress(_) => {
                        // TODO: requires restarting decode at offset
                        tracing::debug!(zone = zone_index, "Seek not yet implemented");
                    }

                    // Settings
                    ZoneCommand::SetVolume(v) => {
                        update_zone_state(&store, zone_index, |z| z.volume = v.clamp(0, 100)).await;
                        // TODO: snapcast.set_group_volume()
                    }
                    ZoneCommand::SetMute(m) => {
                        update_zone_state(&store, zone_index, |z| z.muted = m).await;
                    }
                    ZoneCommand::ToggleMute => {
                        update_zone_state(&store, zone_index, |z| z.muted = !z.muted).await;
                    }
                    ZoneCommand::SetShuffle(v) => {
                        update_zone_state(&store, zone_index, |z| z.shuffle = v).await;
                    }
                    ZoneCommand::ToggleShuffle => {
                        update_zone_state(&store, zone_index, |z| z.shuffle = !z.shuffle).await;
                    }
                    ZoneCommand::SetRepeat(v) => {
                        update_zone_state(&store, zone_index, |z| z.repeat = v).await;
                    }
                    ZoneCommand::ToggleRepeat => {
                        update_zone_state(&store, zone_index, |z| z.repeat = !z.repeat).await;
                    }
                    ZoneCommand::SetTrackRepeat(v) => {
                        update_zone_state(&store, zone_index, |z| z.track_repeat = v).await;
                    }
                    ZoneCommand::ToggleTrackRepeat => {
                        update_zone_state(&store, zone_index, |z| z.track_repeat = !z.track_repeat).await;
                    }
                }
            }

            // ── PCM from decode task ──────────────────────────
            pcm = async { match &mut decode_rx { Some(rx) => rx.recv().await, None => std::future::pending().await } } => {
                match pcm {
                    Some(data) => {
                        let data = resampler.process(&data);
                        if data.is_empty() { continue; } // Resampler buffering
                        if let Err(e) = tcp.write_all(&data).await {
                            tracing::error!(zone = zone_index, error = %e, "TCP write failed");
                            // Reconnect
                            if let Ok(new_tcp) = snapcast::open_audio_source(zone_config.tcp_source_port).await {
                                tcp = new_tcp;
                            }
                        }
                    }
                    None => {
                        // Decode task ended — handle track completion
                        tracing::debug!(zone = zone_index, "Decode task ended");
                        current_decode = None;
                        decode_rx = None;
                        handle_track_complete(&mut source, &config, &subsonic, &store, zone_index, &mut current_decode, &mut decode_rx).await;
                    }
                }
            }

            // ── PCM from AirPlay ──────────────────────────────
            Some(pcm) = airplay_rx.recv() => {
                // AirPlay preempts current source
                if !matches!(source, ActiveSource::AirPlay) {
                    stop_decode(&mut current_decode, &mut decode_rx).await;
                    source = ActiveSource::AirPlay;
                    update_zone_state(&store, zone_index, |z| {
                        z.playback = PlaybackState::Playing;
                        z.source = SourceType::AirPlay;
                    }).await;
                    tracing::info!(zone = zone_index, "AirPlay preempted current source");
                }
                let pcm = airplay_resampler.process(&pcm);
                if pcm.is_empty() { continue; }
                if let Err(e) = tcp.write_all(&pcm).await {
                    tracing::error!(zone = zone_index, error = %e, "TCP write failed (AirPlay)");
                }
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────

async fn stop_decode(
    current: &mut Option<JoinHandle<()>>,
    rx: &mut Option<mpsc::Receiver<Vec<u8>>>,
) {
    if let Some(handle) = current.take() {
        handle.abort();
    }
    *rx = None;
}

async fn update_zone_state(
    store: &state::SharedState,
    zone_index: usize,
    f: impl FnOnce(&mut state::ZoneState),
) {
    let mut s = store.write().await;
    if let Some(zone) = s.zones.get_mut(&zone_index) {
        f(zone);
    }
}

async fn start_subsonic_track_decode(
    sub: &SubsonicClient,
    track: &crate::subsonic::Track,
    config: &AppConfig,
    current_decode: &mut Option<JoinHandle<()>>,
    decode_rx: &mut Option<mpsc::Receiver<Vec<u8>>>,
) {
    let url = sub.stream_url(&track.id);
    let (tx, rx) = audio::pcm_channel(64);
    *decode_rx = Some(rx);
    let ac = config.audio.clone();
    *current_decode = Some(tokio::spawn(async move {
        if let Err(e) = audio::decode_http_stream(url, tx, ac).await {
            tracing::error!(error = %e, "Subsonic decode failed");
        }
    }));
}

fn subsonic_track_info(track: &crate::subsonic::Track) -> TrackInfo {
    TrackInfo {
        title: track.title.clone(),
        artist: track.artist.clone().unwrap_or_default(),
        album: track.album.clone().unwrap_or_default(),
        album_artist: None,
        genre: None,
        year: None,
        track_number: track.track,
        disc_number: None,
        duration_ms: (track.duration * 1000) as i64,
        position_ms: 0,
        source: SourceType::SubsonicPlaylist,
        bitrate_kbps: None,
        content_type: None,
        sample_rate: None,
    }
}

async fn handle_next(
    source: &mut ActiveSource,
    config: &AppConfig,
    subsonic: &Option<SubsonicClient>,
    store: &state::SharedState,
    zone_index: usize,
    current_decode: &mut Option<JoinHandle<()>>,
    decode_rx: &mut Option<mpsc::Receiver<Vec<u8>>>,
) {
    match source.clone() {
        ActiveSource::Radio { index } => {
            let next = (index + 1) % config.radios.len();
            stop_decode(current_decode, decode_rx).await;
            if let Some(radio) = config.radios.get(next) {
                let (tx, rx) = audio::pcm_channel(64);
                *decode_rx = Some(rx);
                let url = radio.url.clone();
                let ac = config.audio.clone();
                *current_decode = Some(tokio::spawn(async move {
                    if let Err(e) = audio::decode_http_stream(url, tx, ac).await {
                        tracing::error!(error = %e, "Radio decode failed");
                    }
                }));
                *source = ActiveSource::Radio { index: next };
                update_zone_state(store, zone_index, |z| {
                    z.radio_index = Some(next);
                    z.track = Some(TrackInfo {
                        title: radio.name.clone(),
                        artist: "Radio".into(),
                        album: String::new(),
                        album_artist: None,
                        genre: None,
                        year: None,
                        track_number: None,
                        disc_number: None,
                        duration_ms: 0,
                        position_ms: 0,
                        source: SourceType::Radio,
                        bitrate_kbps: None,
                        content_type: None,
                        sample_rate: None,
                    });
                })
                .await;
                tracing::info!(zone = zone_index, radio = %radio.name, "Next radio station");
            }
        }
        ActiveSource::SubsonicPlaylist {
            playlist_id,
            track_index,
            track_count,
        } => {
            let next = track_index + 1;
            if next < track_count {
                advance_playlist_track(
                    source,
                    &playlist_id,
                    next,
                    track_count,
                    config,
                    subsonic,
                    store,
                    zone_index,
                    current_decode,
                    decode_rx,
                )
                .await;
            } else {
                let repeat = store
                    .read()
                    .await
                    .zones
                    .get(&zone_index)
                    .is_some_and(|z| z.repeat);
                if repeat {
                    advance_playlist_track(
                        source,
                        &playlist_id,
                        0,
                        track_count,
                        config,
                        subsonic,
                        store,
                        zone_index,
                        current_decode,
                        decode_rx,
                    )
                    .await;
                } else {
                    stop_decode(current_decode, decode_rx).await;
                    *source = ActiveSource::Idle;
                    update_zone_state(store, zone_index, |z| {
                        z.playback = PlaybackState::Stopped;
                        z.source = SourceType::Idle;
                    })
                    .await;
                }
            }
        }
        _ => {}
    }
}

async fn handle_previous(
    source: &mut ActiveSource,
    config: &AppConfig,
    subsonic: &Option<SubsonicClient>,
    store: &state::SharedState,
    zone_index: usize,
    current_decode: &mut Option<JoinHandle<()>>,
    decode_rx: &mut Option<mpsc::Receiver<Vec<u8>>>,
) {
    match source.clone() {
        ActiveSource::Radio { index } => {
            let prev = if index == 0 {
                config.radios.len() - 1
            } else {
                index - 1
            };
            // Reuse next logic with different index
            stop_decode(current_decode, decode_rx).await;
            if let Some(radio) = config.radios.get(prev) {
                let (tx, rx) = audio::pcm_channel(64);
                *decode_rx = Some(rx);
                let url = radio.url.clone();
                let ac = config.audio.clone();
                *current_decode = Some(tokio::spawn(async move {
                    if let Err(e) = audio::decode_http_stream(url, tx, ac).await {
                        tracing::error!(error = %e, "Radio decode failed");
                    }
                }));
                *source = ActiveSource::Radio { index: prev };
                update_zone_state(store, zone_index, |z| {
                    z.radio_index = Some(prev);
                    z.track = Some(TrackInfo {
                        title: radio.name.clone(),
                        artist: "Radio".into(),
                        album: String::new(),
                        album_artist: None,
                        genre: None,
                        year: None,
                        track_number: None,
                        disc_number: None,
                        duration_ms: 0,
                        position_ms: 0,
                        source: SourceType::Radio,
                        bitrate_kbps: None,
                        content_type: None,
                        sample_rate: None,
                    });
                })
                .await;
                tracing::info!(zone = zone_index, radio = %radio.name, "Previous radio station");
            }
        }
        ActiveSource::SubsonicPlaylist {
            playlist_id,
            track_index,
            track_count,
        } => {
            if track_index > 0 {
                advance_playlist_track(
                    source,
                    &playlist_id,
                    track_index - 1,
                    track_count,
                    config,
                    subsonic,
                    store,
                    zone_index,
                    current_decode,
                    decode_rx,
                )
                .await;
            }
        }
        _ => {}
    }
}

async fn handle_track_complete(
    source: &mut ActiveSource,
    config: &AppConfig,
    subsonic: &Option<SubsonicClient>,
    store: &state::SharedState,
    zone_index: usize,
    current_decode: &mut Option<JoinHandle<()>>,
    decode_rx: &mut Option<mpsc::Receiver<Vec<u8>>>,
) {
    let track_repeat = store
        .read()
        .await
        .zones
        .get(&zone_index)
        .is_some_and(|z| z.track_repeat);
    let shuffle = store
        .read()
        .await
        .zones
        .get(&zone_index)
        .is_some_and(|z| z.shuffle);

    match source.clone() {
        ActiveSource::SubsonicPlaylist {
            playlist_id,
            track_index,
            track_count,
        } => {
            if track_repeat {
                // Replay same track
                advance_playlist_track(
                    source,
                    &playlist_id,
                    track_index,
                    track_count,
                    config,
                    subsonic,
                    store,
                    zone_index,
                    current_decode,
                    decode_rx,
                )
                .await;
            } else if shuffle {
                let next = fastrand::usize(..track_count);
                advance_playlist_track(
                    source,
                    &playlist_id,
                    next,
                    track_count,
                    config,
                    subsonic,
                    store,
                    zone_index,
                    current_decode,
                    decode_rx,
                )
                .await;
            } else {
                // Auto-advance to next track
                handle_next(
                    source,
                    config,
                    subsonic,
                    store,
                    zone_index,
                    current_decode,
                    decode_rx,
                )
                .await;
            }
        }
        ActiveSource::Radio { .. } => {
            // Radio stream ended unexpectedly — restart
            tracing::warn!(zone = zone_index, "Radio stream ended, restarting");
            if let ActiveSource::Radio { index } = source.clone() {
                if let Some(radio) = config.radios.get(index) {
                    let (tx, rx) = audio::pcm_channel(64);
                    *decode_rx = Some(rx);
                    let url = radio.url.clone();
                    let ac = config.audio.clone();
                    *current_decode = Some(tokio::spawn(async move {
                        if let Err(e) = audio::decode_http_stream(url, tx, ac).await {
                            tracing::error!(error = %e, "Radio restart failed");
                        }
                    }));
                }
            }
        }
        ActiveSource::AirPlay => {
            // AirPlay ended — go idle
            *source = ActiveSource::Idle;
            update_zone_state(store, zone_index, |z| {
                z.playback = PlaybackState::Stopped;
                z.source = SourceType::Idle;
                z.track = None;
            })
            .await;
            tracing::info!(zone = zone_index, "AirPlay ended, zone idle");
        }
        _ => {
            // URL or single track ended — go idle
            *source = ActiveSource::Idle;
            update_zone_state(store, zone_index, |z| {
                z.playback = PlaybackState::Stopped;
                z.source = SourceType::Idle;
            })
            .await;
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn advance_playlist_track(
    source: &mut ActiveSource,
    playlist_id: &str,
    track_index: usize,
    track_count: usize,
    config: &AppConfig,
    subsonic: &Option<SubsonicClient>,
    store: &state::SharedState,
    zone_index: usize,
    current_decode: &mut Option<JoinHandle<()>>,
    decode_rx: &mut Option<mpsc::Receiver<Vec<u8>>>,
) {
    stop_decode(current_decode, decode_rx).await;
    if let Some(sub) = subsonic {
        if let Ok(playlist) = sub.get_playlist(playlist_id).await {
            if let Some(track) = playlist.entry.get(track_index) {
                start_subsonic_track_decode(sub, track, config, current_decode, decode_rx).await;
                *source = ActiveSource::SubsonicPlaylist {
                    playlist_id: playlist_id.to_string(),
                    track_index,
                    track_count,
                };
                update_zone_state(store, zone_index, |z| {
                    z.playlist_track_index = Some(track_index);
                    z.track = Some(subsonic_track_info(track));
                })
                .await;
                tracing::info!(zone = zone_index, track = track_index, "Advanced to track");
            }
        }
    }
}
