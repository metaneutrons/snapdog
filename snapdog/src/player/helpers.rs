// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! ZonePlayer helper functions for track navigation and completion.

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::commands::ActiveSource;
use super::context::{NotifySender, stop_decode, update_and_notify};
use crate::audio;
use crate::config::AppConfig;
use crate::state::{self, PlaybackState, SourceType, TrackInfo};
use crate::subsonic::SubsonicClient;

pub async fn start_subsonic_track_decode(
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
        if let Err(e) = audio::decode_http_stream(url, tx, ac, None).await {
            tracing::error!(error = %e, "Subsonic decode failed");
        }
    }));
}

pub fn subsonic_track_info(track: &crate::subsonic::Track) -> TrackInfo {
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

fn radio_track_info(name: &str) -> TrackInfo {
    TrackInfo {
        title: name.to_string(),
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
    }
}

async fn start_radio_decode(
    url: &str,
    config: &AppConfig,
    current_decode: &mut Option<JoinHandle<()>>,
    decode_rx: &mut Option<mpsc::Receiver<Vec<u8>>>,
) {
    let (tx, rx) = audio::pcm_channel(64);
    *decode_rx = Some(rx);
    let url = url.to_string();
    let ac = config.audio.clone();
    *current_decode = Some(tokio::spawn(async move {
        if let Err(e) = audio::decode_http_stream(url, tx, ac, None).await {
            tracing::error!(error = %e, "Radio decode failed");
        }
    }));
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_next(
    source: &mut ActiveSource,
    config: &AppConfig,
    subsonic: &Option<SubsonicClient>,
    store: &state::SharedState,
    zone_index: usize,
    current_decode: &mut Option<JoinHandle<()>>,
    decode_rx: &mut Option<mpsc::Receiver<Vec<u8>>>,
    notify: &NotifySender,
) {
    match source.clone() {
        ActiveSource::Radio { index } => {
            let next = (index + 1) % config.radios.len();
            stop_decode(current_decode, decode_rx).await;
            if let Some(radio) = config.radios.get(next) {
                start_radio_decode(&radio.url, config, current_decode, decode_rx).await;
                *source = ActiveSource::Radio { index: next };
                update_and_notify(store, zone_index, notify, |z| {
                    z.radio_index = Some(next);
                    z.track = Some(radio_track_info(&radio.name));
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
                    notify,
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
                        notify,
                    )
                    .await;
                } else {
                    stop_decode(current_decode, decode_rx).await;
                    *source = ActiveSource::Idle;
                    update_and_notify(store, zone_index, notify, |z| {
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

#[allow(clippy::too_many_arguments)]
pub async fn handle_previous(
    source: &mut ActiveSource,
    config: &AppConfig,
    subsonic: &Option<SubsonicClient>,
    store: &state::SharedState,
    zone_index: usize,
    current_decode: &mut Option<JoinHandle<()>>,
    decode_rx: &mut Option<mpsc::Receiver<Vec<u8>>>,
    notify: &NotifySender,
) {
    match source.clone() {
        ActiveSource::Radio { index } => {
            let prev = if index == 0 {
                config.radios.len() - 1
            } else {
                index - 1
            };
            stop_decode(current_decode, decode_rx).await;
            if let Some(radio) = config.radios.get(prev) {
                start_radio_decode(&radio.url, config, current_decode, decode_rx).await;
                *source = ActiveSource::Radio { index: prev };
                update_and_notify(store, zone_index, notify, |z| {
                    z.radio_index = Some(prev);
                    z.track = Some(radio_track_info(&radio.name));
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
                    notify,
                )
                .await;
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn handle_track_complete(
    source: &mut ActiveSource,
    config: &AppConfig,
    subsonic: &Option<SubsonicClient>,
    store: &state::SharedState,
    zone_index: usize,
    current_decode: &mut Option<JoinHandle<()>>,
    decode_rx: &mut Option<mpsc::Receiver<Vec<u8>>>,
    notify: &NotifySender,
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
                    notify,
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
                    notify,
                )
                .await;
            } else {
                handle_next(
                    source,
                    config,
                    subsonic,
                    store,
                    zone_index,
                    current_decode,
                    decode_rx,
                    notify,
                )
                .await;
            }
        }
        ActiveSource::Radio { index } => {
            tracing::warn!(zone = zone_index, "Radio stream ended, restarting");
            if let Some(radio) = config.radios.get(index) {
                start_radio_decode(&radio.url, config, current_decode, decode_rx).await;
            }
        }
        ActiveSource::AirPlay => {
            *source = ActiveSource::Idle;
            update_and_notify(store, zone_index, notify, |z| {
                z.playback = PlaybackState::Stopped;
                z.source = SourceType::Idle;
                z.track = None;
            })
            .await;
            tracing::info!(zone = zone_index, "AirPlay ended, zone idle");
        }
        _ => {
            *source = ActiveSource::Idle;
            update_and_notify(store, zone_index, notify, |z| {
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
    notify: &NotifySender,
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
                update_and_notify(store, zone_index, notify, |z| {
                    z.playlist_track_index = Some(track_index);
                    z.track = Some(subsonic_track_info(track));
                })
                .await;
                tracing::info!(zone = zone_index, track = track_index, "Advanced to track");
            }
        }
    }
}
