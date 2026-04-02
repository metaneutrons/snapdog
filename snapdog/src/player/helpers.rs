// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! ZonePlayer helper functions for track navigation and completion.

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::commands::ActiveSource;
use super::context::{NotifySender, stop_decode, update_and_notify};
use crate::audio;
use crate::config::AppConfig;
use crate::state::cover::SharedCoverCache;
use crate::state::{self, PlaybackState, SourceType, TrackInfo};
use crate::subsonic::SubsonicClient;

/// Mutable decode state passed to navigation helpers.
pub struct DecodeState<'a> {
    pub current_decode: &'a mut Option<JoinHandle<()>>,
    pub decode_rx: &'a mut Option<mpsc::Receiver<Vec<u8>>>,
    pub source: &'a mut ActiveSource,
}

/// Read-only playback context shared by all navigation helpers.
pub struct PlaybackCtx<'a> {
    pub config: &'a AppConfig,
    pub subsonic: &'a Option<SubsonicClient>,
    pub store: &'a state::SharedState,
    pub zone_index: usize,
    pub notify: &'a NotifySender,
    pub covers: &'a SharedCoverCache,
}
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

pub fn radio_track_info(name: &str) -> TrackInfo {
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

pub async fn start_radio_decode(
    url: &str,
    config: &AppConfig,
    current_decode: &mut Option<JoinHandle<()>>,
    decode_rx: &mut Option<mpsc::Receiver<Vec<u8>>>,
    store: &state::SharedState,
    zone_index: usize,
    notify: &NotifySender,
) {
    let (tx, rx) = audio::pcm_channel(64);
    *decode_rx = Some(rx);
    let (icy_tx, mut icy_rx) = mpsc::channel::<audio::icy::IcyMetadata>(4);
    let icy_store = store.clone();
    let icy_notify = notify.clone();
    tokio::spawn(async move {
        while let Some(meta) = icy_rx.recv().await {
            if let Some(title) = meta.title {
                tracing::info!(zone = zone_index, title = %title, "ICY title update");
                update_and_notify(&icy_store, zone_index, &icy_notify, |z| {
                    if let Some(ref mut track) = z.track {
                        track.artist = title.clone();
                    }
                })
                .await;
            }
        }
    });
    let url = url.to_string();
    let ac = config.audio.clone();
    *current_decode = Some(tokio::spawn(async move {
        if let Err(e) = audio::decode_http_stream(url, tx, ac, Some(icy_tx)).await {
            tracing::error!(error = %e, "Radio decode failed");
        }
    }));
}

pub async fn handle_next(ds: &mut DecodeState<'_>, ctx: &PlaybackCtx<'_>) {
    match ds.source.clone() {
        ActiveSource::Radio { index } => {
            let next = (index + 1) % ctx.config.radios.len();
            stop_decode(ds.current_decode, ds.decode_rx).await;
            if let Some(radio) = ctx.config.radios.get(next) {
                start_radio_decode(
                    &radio.url,
                    ctx.config,
                    ds.current_decode,
                    ds.decode_rx,
                    ctx.store,
                    ctx.zone_index,
                    ctx.notify,
                )
                .await;
                *ds.source = ActiveSource::Radio { index: next };
                update_and_notify(ctx.store, ctx.zone_index, ctx.notify, |z| {
                    z.radio_index = Some(next);
                    z.playlist_index = Some(0);
                    z.playlist_track_index = Some(next);
                    z.track = Some(radio_track_info(&radio.name));
                })
                .await;
                tracing::info!(zone = ctx.zone_index, radio = %radio.name, "Next radio station");
            }
        }
        ActiveSource::SubsonicPlaylist {
            playlist_id,
            track_index,
            track_count,
        } => {
            let next = track_index + 1;
            if next < track_count {
                advance_playlist_track(ds, &playlist_id, next, track_count, ctx).await;
            } else {
                let repeat = ctx
                    .store
                    .read()
                    .await
                    .zones
                    .get(&ctx.zone_index)
                    .is_some_and(|z| z.repeat);
                if repeat {
                    advance_playlist_track(ds, &playlist_id, 0, track_count, ctx).await;
                } else {
                    stop_decode(ds.current_decode, ds.decode_rx).await;
                    *ds.source = ActiveSource::Idle;
                    update_and_notify(ctx.store, ctx.zone_index, ctx.notify, |z| {
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

pub async fn handle_previous(ds: &mut DecodeState<'_>, ctx: &PlaybackCtx<'_>) {
    match ds.source.clone() {
        ActiveSource::Radio { index } => {
            let prev = if index == 0 {
                ctx.config.radios.len() - 1
            } else {
                index - 1
            };
            stop_decode(ds.current_decode, ds.decode_rx).await;
            if let Some(radio) = ctx.config.radios.get(prev) {
                start_radio_decode(
                    &radio.url,
                    ctx.config,
                    ds.current_decode,
                    ds.decode_rx,
                    ctx.store,
                    ctx.zone_index,
                    ctx.notify,
                )
                .await;
                *ds.source = ActiveSource::Radio { index: prev };
                update_and_notify(ctx.store, ctx.zone_index, ctx.notify, |z| {
                    z.radio_index = Some(prev);
                    z.playlist_index = Some(0);
                    z.playlist_track_index = Some(prev);
                    z.track = Some(radio_track_info(&radio.name));
                })
                .await;
                tracing::info!(zone = ctx.zone_index, radio = %radio.name, "Previous radio station");
            }
        }
        ActiveSource::SubsonicPlaylist {
            playlist_id,
            track_index,
            track_count,
        } => {
            if track_index > 0 {
                advance_playlist_track(ds, &playlist_id, track_index - 1, track_count, ctx).await;
            }
        }
        _ => {}
    }
}

pub async fn handle_track_complete(ds: &mut DecodeState<'_>, ctx: &PlaybackCtx<'_>) {
    let track_repeat = ctx
        .store
        .read()
        .await
        .zones
        .get(&ctx.zone_index)
        .is_some_and(|z| z.track_repeat);
    let shuffle = ctx
        .store
        .read()
        .await
        .zones
        .get(&ctx.zone_index)
        .is_some_and(|z| z.shuffle);

    match ds.source.clone() {
        ActiveSource::SubsonicPlaylist {
            playlist_id,
            track_index,
            track_count,
        } => {
            if track_repeat {
                advance_playlist_track(ds, &playlist_id, track_index, track_count, ctx).await;
            } else if shuffle {
                let next = fastrand::usize(..track_count);
                advance_playlist_track(ds, &playlist_id, next, track_count, ctx).await;
            } else {
                handle_next(ds, ctx).await;
            }
        }
        ActiveSource::Radio { index } => {
            tracing::warn!(zone = ctx.zone_index, "Radio stream ended, restarting");
            if let Some(radio) = ctx.config.radios.get(index) {
                start_radio_decode(
                    &radio.url,
                    ctx.config,
                    ds.current_decode,
                    ds.decode_rx,
                    ctx.store,
                    ctx.zone_index,
                    ctx.notify,
                )
                .await;
            }
        }
        ActiveSource::AirPlay => {
            *ds.source = ActiveSource::Idle;
            update_and_notify(ctx.store, ctx.zone_index, ctx.notify, |z| {
                z.playback = PlaybackState::Stopped;
                z.source = SourceType::Idle;
                z.track = None;
            })
            .await;
            tracing::info!(zone = ctx.zone_index, "AirPlay ended, zone idle");
        }
        _ => {
            *ds.source = ActiveSource::Idle;
            update_and_notify(ctx.store, ctx.zone_index, ctx.notify, |z| {
                z.playback = PlaybackState::Stopped;
                z.source = SourceType::Idle;
            })
            .await;
        }
    }
}

async fn advance_playlist_track(
    ds: &mut DecodeState<'_>,
    playlist_id: &str,
    track_index: usize,
    track_count: usize,
    ctx: &PlaybackCtx<'_>,
) {
    stop_decode(ds.current_decode, ds.decode_rx).await;
    if let Some(sub) = &ctx.subsonic {
        if let Ok(playlist) = sub.get_playlist(playlist_id).await {
            if let Some(track) = playlist.entry.get(track_index) {
                start_subsonic_track_decode(
                    sub,
                    track,
                    ctx.config,
                    ds.current_decode,
                    ds.decode_rx,
                )
                .await;
                *ds.source = ActiveSource::SubsonicPlaylist {
                    playlist_id: playlist_id.to_string(),
                    track_index,
                    track_count,
                };
                update_and_notify(ctx.store, ctx.zone_index, ctx.notify, |z| {
                    z.playlist_track_index = Some(track_index);
                    z.track = Some(subsonic_track_info(track));
                })
                .await;
                // Fetch cover art
                if let Some(ref cover_id) = track.cover_art {
                    let covers = ctx.covers.clone();
                    let url = sub.cover_art_url(cover_id);
                    let zi = ctx.zone_index;
                    tokio::spawn(async move {
                        if let Some((bytes, mime)) = state::cover::fetch_cover(&url).await {
                            covers.write().await.set(zi, bytes, mime);
                        }
                    });
                }
                tracing::info!(
                    zone = ctx.zone_index,
                    track = track_index,
                    "Advanced to track"
                );
            }
        }
    }
}
