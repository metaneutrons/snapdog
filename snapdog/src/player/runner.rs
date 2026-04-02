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
use super::context::*;
use super::helpers::*;
use super::helpers::{DecodeState, PlaybackCtx};
use crate::audio;
use crate::snapcast;
use crate::state::{self, PlaybackState, SourceType, TrackInfo};
use crate::subsonic::SubsonicClient;

/// Spawn a ZonePlayer task for each configured zone. Returns command senders.
pub async fn spawn_zone_players(
    ctx: ZonePlayerContext,
) -> Result<HashMap<usize, ZoneCommandSender>> {
    let mut senders = HashMap::new();
    let ctx = Arc::new(ctx);

    for zone in &ctx.config.zones {
        let (cmd_tx, cmd_rx) = mpsc::channel(32); // zone command backlog
        senders.insert(zone.index, cmd_tx);

        let ctx = ctx.clone();
        let zone_index = zone.index;

        tokio::spawn(async move {
            if let Err(e) = run(zone_index, cmd_rx, ctx).await {
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
    ctx: Arc<ZonePlayerContext>,
) -> Result<()> {
    let config = &ctx.config;
    let store = &ctx.store;
    let covers = &ctx.covers;
    let notify = &ctx.notify;
    let zone_config = &config.zones[zone_index - 1];
    let audio_config = config.audio.clone(); // Cloned once, moved into decode tasks

    // Connect TCP to Snapcast source
    let mut tcp = snapcast::open_audio_source(zone_config.tcp_source_port).await?;

    // Zone grouping: assign clients to group, set stream
    let group_id = setup_zone_group(zone_index, &ctx).await;
    {
        let mut s = store.write().await;
        if let Some(zone) = s.zones.get_mut(&zone_index) {
            zone.snapcast_group_id = group_id.clone();
        }
    }

    // Subsonic client (if configured)
    let subsonic = config.subsonic.as_ref().map(SubsonicClient::new);

    // AirPlay: PCM channel + event channel + receiver instance
    let (airplay_pcm_tx, mut airplay_rx) = audio::pcm_channel(128);
    let (airplay_event_tx, mut airplay_event_rx) =
        mpsc::channel::<crate::airplay::AirplayEvent>(32);
    let _airplay_receiver = {
        let ap_config = crate::config::AirplayConfig {
            name: zone_config.airplay_name.clone(),
            password: config.airplay.password.clone(),
        };
        match crate::airplay::AirplayReceiver::start(
            &ap_config,
            zone_index,
            airplay_pcm_tx,
            airplay_event_tx,
        )
        .await
        {
            Ok(r) => {
                tracing::info!(zone = zone_index, name = %ap_config.name, "AirPlay receiver active");
                Some(r)
            }
            Err(e) => {
                tracing::warn!(zone = zone_index, error = %e, "AirPlay receiver failed to start");
                None
            }
        }
    };

    // Decode task state
    let mut current_decode: Option<JoinHandle<()>> = None;
    let mut decode_rx: Option<mpsc::Receiver<Vec<u8>>> = None;
    let mut source = ActiveSource::Idle;
    let mut dacp_client: Option<shairplay::dacp::DacpClient> = None;
    let mut resampler = audio::resample::Resampling::new(
        config.audio.sample_rate,
        config.audio.sample_rate,
        config.audio.channels,
    );
    let mut airplay_resampler =
        audio::resample::Resampling::new(44100, config.audio.sample_rate, config.audio.channels);

    loop {
        tokio::select! {
            Some(cmd) = commands.recv() => {
                match cmd {
                    ZoneCommand::PlaySubsonicPlaylist(playlist_id, track_idx) => {
                        stop_decode(&mut current_decode, &mut decode_rx).await;
                        if let Some(sub) = &subsonic {
                            if let Ok(playlist) = sub.get_playlist(&playlist_id).await {
                                let track_count = playlist.entry.len();
                                if let Some(track) = playlist.entry.get(track_idx) {
                                    start_subsonic_track_decode(sub, track, config, &mut current_decode, &mut decode_rx).await;
                                    source = ActiveSource::SubsonicPlaylist { playlist_id, track_index: track_idx, track_count };
                                    update_and_notify(store, zone_index, notify, |z| {
                                        z.playback = PlaybackState::Playing;
                                        z.source = SourceType::SubsonicPlaylist;
                                        z.playlist_index = Some(track_idx);
                                        z.playlist_name = Some(playlist.name.clone());
                                        z.playlist_track_index = Some(track_idx);
                                        z.playlist_track_count = Some(track_count);
                                        z.track = Some(subsonic_track_info(track));
                                    }).await;
                                    // Fetch cover art
                                    if let Some(ref cover_id) = track.cover_art {
                                        let covers = covers.clone();
                                        let url = sub.cover_art_url(cover_id);
                                        tokio::spawn(async move {
                                            if let Some((bytes, mime)) = state::cover::fetch_cover(&url).await {
                                                covers.write().await.set(zone_index, bytes, mime);
                                            }
                                        });
                                    }
                                }
                            }
                        }
                    }
                    ZoneCommand::PlaySubsonicTrack(track_id) => {
                        stop_decode(&mut current_decode, &mut decode_rx).await;
                        if let Some(sub) = &subsonic {
                            let url = sub.stream_url(&track_id);
                            let (tx, rx) = audio::pcm_channel(64);
                            decode_rx = Some(rx);
                            let ac = audio_config.clone();
                            current_decode = Some(tokio::spawn(async move {
                                if let Err(e) = audio::decode_http_stream(url, tx, ac, None).await {
                                    tracing::error!(error = %e, "Subsonic track decode failed");
                                }
                            }));
                            source = ActiveSource::SubsonicTrack { track_id };
                            update_and_notify(store, zone_index, notify, |z| { z.playback = PlaybackState::Playing; z.source = SourceType::SubsonicTrack; }).await;
                        }
                    }
                    ZoneCommand::PlayUrl(url) => {
                        stop_decode(&mut current_decode, &mut decode_rx).await;
                        let (tx, rx) = audio::pcm_channel(64);
                        decode_rx = Some(rx);
                        let ac = audio_config.clone();
                        let u = url.clone();
                        current_decode = Some(tokio::spawn(async move {
                            if let Err(e) = audio::decode_http_stream(u, tx, ac, None).await { tracing::error!(error = %e, "URL decode failed"); }
                        }));
                        source = ActiveSource::Url;
                        update_and_notify(store, zone_index, notify, |z| { z.playback = PlaybackState::Playing; z.source = SourceType::Url; }).await;
                    }
                    ZoneCommand::SetTrack(track_idx) => {
                        if let ActiveSource::Radio { .. } = source {
                            if track_idx < config.radios.len() {
                                if let Some(radio) = config.radios.get(track_idx) {
                                    stop_decode(&mut current_decode, &mut decode_rx).await;
                                    start_radio_decode(&radio.url, config, &mut current_decode, &mut decode_rx, store, zone_index, notify).await;
                                    source = ActiveSource::Radio { index: track_idx };
                                    update_and_notify(store, zone_index, notify, |z| {
                                        z.radio_index = Some(track_idx);
                                        z.playlist_index = Some(0);
                                        z.playlist_track_index = Some(track_idx);
                                        z.track = Some(radio_track_info(&radio.name));
                                    }).await;
                                    tracing::info!(zone = zone_index, radio = %radio.name, "Set radio station");
                                    if let Some(cover_url) = &radio.cover {
                                        if let Some((bytes, mime)) = state::cover::fetch_cover(cover_url).await {
                                            covers.write().await.set(zone_index, bytes, mime);
                                        } else {
                                            covers.write().await.clear(zone_index);
                                        }
                                    } else {
                                        covers.write().await.clear(zone_index);
                                    }
                                }
                            }
                        } else if let ActiveSource::SubsonicPlaylist { ref playlist_id, track_count, .. } = source {
                            if track_idx < track_count {
                                let pid = playlist_id.clone();
                                stop_decode(&mut current_decode, &mut decode_rx).await;
                                if let Some(sub) = &subsonic {
                                    if let Ok(playlist) = sub.get_playlist(&pid).await {
                                        if let Some(track) = playlist.entry.get(track_idx) {
                                            start_subsonic_track_decode(sub, track, config, &mut current_decode, &mut decode_rx).await;
                                            source = ActiveSource::SubsonicPlaylist { playlist_id: pid, track_index: track_idx, track_count };
                                            update_and_notify(store, zone_index, notify, |z| { z.playlist_track_index = Some(track_idx); z.track = Some(subsonic_track_info(track)); }).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ZoneCommand::Play => {
                        if matches!(source, ActiveSource::AirPlay) {
                            if let Some(ref dacp) = dacp_client {
                                if let Err(e) = dacp.play_pause().await { tracing::warn!(error = %e, "DACP play_pause failed"); }
                            } else { tracing::debug!("No DACP client available for AirPlay control"); }
                        } else if matches!(source, ActiveSource::Idle) {
                            let radio_idx = store.read().await.zones.get(&zone_index).and_then(|z| z.radio_index).unwrap_or(0);
                            if let Some(radio) = config.radios.get(radio_idx) {
                                stop_decode(&mut current_decode, &mut decode_rx).await;
                                let (tx, rx) = audio::pcm_channel(64);
                                decode_rx = Some(rx);
                                let url = radio.url.clone();
                                let ac = audio_config.clone();
                                current_decode = Some(tokio::spawn(async move {
                                    if let Err(e) = audio::decode_http_stream(url, tx, ac, None).await { tracing::error!(error = %e, "Radio decode failed"); }
                                }));
                                source = ActiveSource::Radio { index: radio_idx };
                                update_and_notify(store, zone_index, notify, |z| { z.playback = PlaybackState::Playing; z.source = SourceType::Radio; }).await;
                            }
                        }
                    }
                    ZoneCommand::Pause => {
                        if matches!(source, ActiveSource::AirPlay) {
                            if let Some(ref dacp) = dacp_client {
                                if let Err(e) = dacp.play_pause().await { tracing::warn!(error = %e, "DACP play_pause failed"); }
                            } else { tracing::debug!("No DACP client available for AirPlay control"); }
                        } else {
                            stop_decode(&mut current_decode, &mut decode_rx).await;
                            update_and_notify(store, zone_index, notify, |z| { z.playback = PlaybackState::Paused; }).await;
                        }
                    }
                    ZoneCommand::Stop => {
                        stop_decode(&mut current_decode, &mut decode_rx).await;
                        source = ActiveSource::Idle;
                        update_and_notify(store, zone_index, notify, |z| { z.playback = PlaybackState::Stopped; z.source = SourceType::Idle; z.track = None; }).await;
                    }
                    ZoneCommand::Next => {
                        if matches!(source, ActiveSource::AirPlay) {
                            if let Some(ref dacp) = dacp_client { let _ = dacp.next().await; }
                        } else {
                            handle_next(&mut DecodeState { current_decode: &mut current_decode, decode_rx: &mut decode_rx, source: &mut source }, &PlaybackCtx { config, subsonic: &subsonic, store, zone_index, notify, covers }).await;
                        }
                    }
                    ZoneCommand::Previous => {
                        if matches!(source, ActiveSource::AirPlay) {
                            if let Some(ref dacp) = dacp_client { let _ = dacp.prev().await; }
                        } else {
                            handle_previous(&mut DecodeState { current_decode: &mut current_decode, decode_rx: &mut decode_rx, source: &mut source }, &PlaybackCtx { config, subsonic: &subsonic, store, zone_index, notify, covers }).await;
                        }
                    }
                    ZoneCommand::NextPlaylist | ZoneCommand::PreviousPlaylist | ZoneCommand::SetPlaylist(_) => {
                        // Unified playlist model: index 0 = radio (from config), index 1+ = Subsonic playlists
                        let subsonic_playlists = if let Some(sub) = &subsonic {
                            sub.get_playlists().await.unwrap_or_default()
                        } else {
                            vec![]
                        };
                        let total_count = config.unified_playlist_count(subsonic_playlists.len());
                        if total_count == 0 {
                            tracing::warn!(zone = zone_index, "No playlists available");
                            continue;
                        }

                        // Determine current unified index
                        let has_radio = config.has_radio_playlist();
                        let current_unified = match &source {
                            ActiveSource::Radio { .. } => 0,
                            ActiveSource::SubsonicPlaylist { playlist_id, .. } => {
                                let sub_idx = subsonic_playlists.iter().position(|p| p.id == *playlist_id).unwrap_or(0);
                                if has_radio { sub_idx + 1 } else { sub_idx }
                            }
                            _ => 0,
                        };

                        let target_unified = match cmd {
                            ZoneCommand::NextPlaylist => (current_unified + 1) % total_count,
                            ZoneCommand::PreviousPlaylist => if current_unified == 0 { total_count - 1 } else { current_unified - 1 },
                            ZoneCommand::SetPlaylist(i) => i.min(total_count - 1),
                            _ => continue,
                        };

                        match config.resolve_playlist_index(target_unified, subsonic_playlists.len()) {
                            Some(crate::config::ResolvedPlaylist::Radio) => {
                            stop_decode(&mut current_decode, &mut decode_rx).await;
                            if let Some(radio) = config.radios.first() {
                                start_radio_decode(&radio.url, config, &mut current_decode, &mut decode_rx, store, zone_index, notify).await;
                                source = ActiveSource::Radio { index: 0 };
                                update_and_notify(store, zone_index, notify, |z| {
                                    z.playback = PlaybackState::Playing;
                                    z.source = SourceType::Radio;
                                    z.radio_index = Some(0);
                                    z.playlist_index = Some(0);
                                    z.playlist_name = Some("Radio".into());
                                    z.playlist_track_index = Some(0);
                                    z.playlist_track_count = Some(config.radios.len());
                                    z.track = Some(radio_track_info(&radio.name));
                                }).await;
                                tracing::info!(zone = zone_index, radio = %radio.name, "Playing radio via playlist 0");
                                // Fetch cover synchronously to avoid race with a following SetTrack command
                                if let Some(cover_url) = &radio.cover {
                                    if let Some((bytes, mime)) = state::cover::fetch_cover(cover_url).await {
                                        covers.write().await.set(zone_index, bytes, mime);
                                    } else {
                                        covers.write().await.clear(zone_index);
                                    }
                                } else {
                                    covers.write().await.clear(zone_index);
                                }
                            }
                        }
                            Some(crate::config::ResolvedPlaylist::Subsonic(sub_idx)) => {
                            if let Some(sub) = &subsonic {
                                if let Some(pl) = subsonic_playlists.get(sub_idx) {
                                    tracing::info!(zone = zone_index, playlist = %pl.name, "Switching playlist");
                                    stop_decode(&mut current_decode, &mut decode_rx).await;
                                    if let Ok(playlist) = sub.get_playlist(&pl.id).await {
                                        if let Some(track) = playlist.entry.first() {
                                            start_subsonic_track_decode(sub, track, config, &mut current_decode, &mut decode_rx).await;
                                            source = ActiveSource::SubsonicPlaylist {
                                                playlist_id: pl.id.clone(),
                                                track_index: 0,
                                                track_count: playlist.entry.len(),
                                            };
                                            update_and_notify(store, zone_index, notify, |z| {
                                                z.playback = PlaybackState::Playing;
                                                z.source = SourceType::SubsonicPlaylist;
                                                z.playlist_index = Some(target_unified);
                                                z.playlist_name = Some(playlist.name.clone());
                                                z.playlist_track_index = Some(0);
                                                z.playlist_track_count = Some(playlist.entry.len());
                                                z.track = Some(subsonic_track_info(track));
                                            }).await;
                                        }
                                    }
                                }
                            }
                        }
                            _ => {}
                        }
                    }
                    ZoneCommand::Seek(pos_ms) => {
                        if let Some(sub) = &subsonic {
                            let track_id = match &source {
                                ActiveSource::SubsonicTrack { track_id } => Some(track_id.clone()),
                                ActiveSource::SubsonicPlaylist { playlist_id, track_index, .. } => {
                                    sub.get_playlist(playlist_id).await.ok()
                                        .and_then(|p| p.entry.get(*track_index).map(|t| t.id.clone()))
                                }
                                _ => None,
                            };
                            if let Some(tid) = track_id {
                                stop_decode(&mut current_decode, &mut decode_rx).await;
                                let offset_secs = (pos_ms / 1000).max(0) as u64;
                                let url = sub.stream_url_with_offset(&tid, offset_secs);
                                let (tx, rx) = audio::pcm_channel(64);
                                decode_rx = Some(rx);
                                let ac = audio_config.clone();
                                current_decode = Some(tokio::spawn(async move {
                                    if let Err(e) = audio::decode_http_stream(url, tx, ac, None).await {
                                        tracing::error!(error = %e, "Seek decode failed");
                                    }
                                }));
                                update_and_notify(store, zone_index, notify, |z| {
                                    if let Some(ref mut t) = z.track { t.position_ms = pos_ms; }
                                }).await;
                                tracing::info!(zone = zone_index, position_ms = pos_ms, "Seeked");
                            }
                        }
                    }
                    ZoneCommand::SeekProgress(progress) => {
                        let duration = store.read().await.zones.get(&zone_index)
                            .and_then(|z| z.track.as_ref().map(|t| t.duration_ms)).unwrap_or(0);
                        if duration > 0 {
                            let pos_ms = (progress.clamp(0.0, 1.0) * duration as f64) as i64;
                            let _ = commands.try_recv(); // drain
                            // Re-dispatch as absolute seek — will be handled next iteration
                            // For simplicity, inline the same logic:
                            if let Some(sub) = &subsonic {
                                let track_id = match &source {
                                    ActiveSource::SubsonicTrack { track_id } => Some(track_id.clone()),
                                    ActiveSource::SubsonicPlaylist { playlist_id, track_index, .. } => {
                                        sub.get_playlist(playlist_id).await.ok()
                                            .and_then(|p| p.entry.get(*track_index).map(|t| t.id.clone()))
                                    }
                                    _ => None,
                                };
                                if let Some(tid) = track_id {
                                    stop_decode(&mut current_decode, &mut decode_rx).await;
                                    let url = sub.stream_url_with_offset(&tid, (pos_ms / 1000).max(0) as u64);
                                    let (tx, rx) = audio::pcm_channel(64);
                                    decode_rx = Some(rx);
                                    let ac = audio_config.clone();
                                    current_decode = Some(tokio::spawn(async move {
                                        if let Err(e) = audio::decode_http_stream(url, tx, ac, None).await {
                                            tracing::error!(error = %e, "Seek decode failed");
                                        }
                                    }));
                                    update_and_notify(store, zone_index, notify, |z| {
                                        if let Some(ref mut t) = z.track { t.position_ms = pos_ms; }
                                    }).await;
                                }
                            }
                        }
                    }
                    ZoneCommand::SetVolume(v) => {
                        update_and_notify(store, zone_index, notify, |z| z.volume = v.clamp(0, 100)).await;
                        if let Some(ref gid) = group_id {
                            let _ = ctx.snap_tx.send(SnapcastCmd { group_id: gid.clone(), action: SnapcastAction::Volume(v) }).await;
                        }
                    }
                    ZoneCommand::SetMute(m) => {
                        update_and_notify(store, zone_index, notify, |z| z.muted = m).await;
                        if let Some(ref gid) = group_id {
                            let _ = ctx.snap_tx.send(SnapcastCmd { group_id: gid.clone(), action: SnapcastAction::Mute(m) }).await;
                        }
                    }
                    ZoneCommand::ToggleMute => {
                        let muted = { store.read().await.zones.get(&zone_index).is_some_and(|z| !z.muted) };
                        update_and_notify(store, zone_index, notify, |z| z.muted = muted).await;
                        if let Some(ref gid) = group_id {
                            let _ = ctx.snap_tx.send(SnapcastCmd { group_id: gid.clone(), action: SnapcastAction::Mute(muted) }).await;
                        }
                    }
                    ZoneCommand::SetShuffle(v) => { update_and_notify(store, zone_index, notify, |z| z.shuffle = v).await; }
                    ZoneCommand::ToggleShuffle => { update_and_notify(store, zone_index, notify, |z| z.shuffle = !z.shuffle).await; }
                    ZoneCommand::SetRepeat(v) => { update_and_notify(store, zone_index, notify, |z| z.repeat = v).await; }
                    ZoneCommand::ToggleRepeat => { update_and_notify(store, zone_index, notify, |z| z.repeat = !z.repeat).await; }
                    ZoneCommand::SetTrackRepeat(v) => { update_and_notify(store, zone_index, notify, |z| z.track_repeat = v).await; }
                    ZoneCommand::ToggleTrackRepeat => { update_and_notify(store, zone_index, notify, |z| z.track_repeat = !z.track_repeat).await; }
                }
            }
            pcm = async { match &mut decode_rx { Some(rx) => rx.recv().await, None => std::future::pending().await } } => {
                match pcm {
                    Some(data) => {
                        let data = resampler.process(&data).unwrap_or(data);
                        if data.is_empty() { continue; }
                        if let Err(e) = tcp.write_all(&data).await {
                            tracing::error!(zone = zone_index, error = %e, "TCP write failed");
                            if let Ok(new_tcp) = snapcast::open_audio_source(zone_config.tcp_source_port).await { tcp = new_tcp; }
                        }
                    }
                    None => {
                        current_decode = None;
                        decode_rx = None;
                        handle_track_complete(&mut DecodeState { current_decode: &mut current_decode, decode_rx: &mut decode_rx, source: &mut source }, &PlaybackCtx { config, subsonic: &subsonic, store, zone_index, notify, covers }).await;
                    }
                }
            }
            Some(pcm) = airplay_rx.recv() => {
                if !matches!(source, ActiveSource::AirPlay) {
                    stop_decode(&mut current_decode, &mut decode_rx).await;
                    source = ActiveSource::AirPlay;
                    update_and_notify(store, zone_index, notify, |z| { z.playback = PlaybackState::Playing; z.source = SourceType::AirPlay; }).await;
                }
                let pcm = airplay_resampler.process(&pcm).unwrap_or(pcm);
                if pcm.is_empty() { continue; }
                if let Err(e) = tcp.write_all(&pcm).await { tracing::error!(zone = zone_index, error = %e, "TCP write failed (AirPlay)"); }
            }
            Some(event) = airplay_event_rx.recv() => {
                use crate::airplay::AirplayEvent;
                match event {
                    AirplayEvent::Metadata { title, artist, album } => {
                        update_and_notify(store, zone_index, notify, |z| {
                            z.track = Some(TrackInfo { title, artist, album, album_artist: None, genre: None, year: None, track_number: None, disc_number: None, duration_ms: 0, position_ms: 0, source: SourceType::AirPlay, bitrate_kbps: None, content_type: None, sample_rate: Some(44100) });
                        }).await;
                    }
                    AirplayEvent::CoverArt { bytes } => { covers.write().await.set_auto_mime(zone_index, bytes); }
                    AirplayEvent::Progress { position_ms, duration_ms } => {
                        update_and_notify(store, zone_index, notify, |z| { if let Some(ref mut t) = z.track { t.position_ms = position_ms as i64; t.duration_ms = duration_ms as i64; } }).await;
                    }
                    AirplayEvent::Volume { percent } => {
                        update_and_notify(store, zone_index, notify, |z| z.volume = percent).await;
                        if let Some(ref gid) = group_id {
                            let _ = ctx.snap_tx.send(SnapcastCmd { group_id: gid.clone(), action: SnapcastAction::Volume(percent) }).await;
                        }
                    }
                    AirplayEvent::RemoteAvailable { client } => {
                        dacp_client = Some(client);
                    }
                    AirplayEvent::SessionEnded => {
                        source = ActiveSource::Idle;
                        dacp_client = None;
                        covers.write().await.clear(zone_index);
                        update_and_notify(store, zone_index, notify, |z| { z.playback = PlaybackState::Stopped; z.source = SourceType::Idle; z.track = None; }).await;
                    }
                }
            }
        }
    }
}
