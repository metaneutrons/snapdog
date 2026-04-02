// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Media endpoints: /api/v1/media

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::api::SharedState;
use crate::subsonic::SubsonicClient;

#[derive(Serialize)]
struct PlaylistInfo {
    id: usize,
    name: String,
    song_count: u32,
    duration: u64,
    cover_art: Option<String>,
}

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/playlists", get(get_playlists))
        .route("/playlists/{playlist_index}", get(get_playlist))
        .route("/playlists/{playlist_index}/cover", get(get_playlist_cover))
        .route(
            "/playlists/{playlist_index}/tracks",
            get(get_playlist_tracks),
        )
        .route(
            "/playlists/{playlist_index}/tracks/{track_index}",
            get(get_playlist_track),
        )
        .with_state(state)
}

fn subsonic(state: &SharedState) -> Result<SubsonicClient, StatusCode> {
    state
        .config
        .subsonic
        .as_ref()
        .map(SubsonicClient::new)
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)
}

async fn get_playlists(State(state): State<SharedState>) -> impl IntoResponse {
    let mut result: Vec<PlaylistInfo> = Vec::new();
    let mut idx: usize = 0;

    // Playlist 0: Radio stations (from config)
    if !state.config.radios.is_empty() {
        result.push(PlaylistInfo {
            id: idx,
            name: "Radio".into(),
            song_count: state.config.radios.len() as u32,
            duration: 0,
            cover_art: None,
        });
        idx += 1;
    }

    // Playlist 1+: Subsonic playlists
    if let Ok(sub) = subsonic(&state) {
        if let Ok(playlists) = sub.get_playlists().await {
            for p in playlists {
                result.push(PlaylistInfo {
                    id: idx,
                    name: p.name,
                    song_count: p.song_count,
                    duration: p.duration,
                    cover_art: p.cover_art,
                });
                idx += 1;
            }
        }
    }

    Ok::<_, StatusCode>(Json(result))
}

/// Resolve a unified playlist index to either radio or a Subsonic playlist ID.
/// Returns None for radio (index 0 when radios exist), Some(subsonic_id) for Subsonic.
async fn resolve_subsonic_id(
    state: &SharedState,
    index: usize,
) -> Result<Option<String>, StatusCode> {
    let has_radio = !state.config.radios.is_empty();
    if has_radio && index == 0 {
        return Ok(None); // radio
    }
    let sub_idx = if has_radio { index - 1 } else { index };
    let sub = subsonic(state)?;
    let playlists = sub
        .get_playlists()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    playlists
        .get(sub_idx)
        .map(|p| Some(p.id.clone()))
        .ok_or(StatusCode::NOT_FOUND)
}

async fn get_playlist(
    State(state): State<SharedState>,
    Path(index): Path<usize>,
) -> impl IntoResponse {
    match resolve_subsonic_id(&state, index).await? {
        None => Ok(Json(serde_json::json!({
            "id": index,
            "name": "Radio",
            "tracks": state.config.radios.len(),
        }))),
        Some(id) => {
            let sub = subsonic(&state)?;
            match sub.get_playlist(&id).await {
                Ok(playlist) => Ok(Json(serde_json::json!({
                    "id": index,
                    "name": playlist.name,
                    "tracks": playlist.entry.len(),
                }))),
                Err(e) => {
                    tracing::error!(error = %e, "Failed to fetch playlist");
                    Err(StatusCode::BAD_GATEWAY)
                }
            }
        }
    }
}

async fn get_playlist_cover(
    State(state): State<SharedState>,
    Path(index): Path<usize>,
) -> impl IntoResponse {
    let id = resolve_subsonic_id(&state, index)
        .await?
        .ok_or(StatusCode::NOT_FOUND)?;
    let sub = subsonic(&state)?;
    let playlist = sub
        .get_playlists()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    let cover_id = playlist
        .iter()
        .find(|p| p.id == id)
        .and_then(|p| p.cover_art.clone())
        .ok_or(StatusCode::NOT_FOUND)?;
    match sub.get_cover_art(&cover_id).await {
        Ok(bytes) => {
            let mime = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
                "image/jpeg"
            } else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
                "image/png"
            } else {
                "image/octet-stream"
            };
            Ok((
                [(axum::http::header::CONTENT_TYPE, mime.to_string())],
                bytes,
            ))
        }
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

async fn get_playlist_tracks(
    State(state): State<SharedState>,
    Path(index): Path<usize>,
) -> impl IntoResponse {
    match resolve_subsonic_id(&state, index).await? {
        None => Ok(Json(
            state
                .config
                .radios
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    serde_json::json!({
                        "id": format!("radio_{i}"),
                        "title": r.name,
                        "artist": "Radio",
                        "album": "",
                        "duration": 0,
                        "track": i + 1,
                    })
                })
                .collect::<Vec<_>>(),
        )),
        Some(id) => {
            let sub = subsonic(&state)?;
            match sub.get_playlist(&id).await {
                Ok(playlist) => Ok(Json(
                    playlist
                        .entry
                        .iter()
                        .map(|t| {
                            serde_json::json!({
                                "id": t.id,
                                "title": t.title,
                                "artist": t.artist,
                                "album": t.album,
                                "duration": t.duration,
                                "track": t.track,
                            })
                        })
                        .collect::<Vec<_>>(),
                )),
                Err(e) => {
                    tracing::error!(error = %e, "Failed to fetch tracks");
                    Err(StatusCode::BAD_GATEWAY)
                }
            }
        }
    }
}

async fn get_playlist_track(
    State(state): State<SharedState>,
    Path((index, track_index)): Path<(usize, usize)>,
) -> impl IntoResponse {
    match resolve_subsonic_id(&state, index).await? {
        None => state
            .config
            .radios
            .get(track_index)
            .map(|r| {
                Json(serde_json::json!({
                    "id": format!("radio_{track_index}"),
                    "title": r.name,
                    "artist": "Radio",
                    "album": "",
                    "duration": 0,
                    "track": track_index + 1,
                }))
            })
            .ok_or(StatusCode::NOT_FOUND),
        Some(id) => {
            let sub = subsonic(&state)?;
            match sub.get_playlist(&id).await {
                Ok(playlist) => match playlist.entry.get(track_index) {
                    Some(t) => Ok(Json(serde_json::json!({
                        "id": t.id,
                        "title": t.title,
                        "artist": t.artist,
                        "album": t.album,
                        "duration": t.duration,
                        "track": t.track,
                    }))),
                    None => Err(StatusCode::NOT_FOUND),
                },
                Err(_) => Err(StatusCode::BAD_GATEWAY),
            }
        }
    }
}
