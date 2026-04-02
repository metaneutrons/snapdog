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
    id: String,
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

    // Playlist 0: Radio stations (from config)
    if !state.config.radios.is_empty() {
        result.push(PlaylistInfo {
            id: "radio".into(),
            name: "Radio".into(),
            song_count: state.config.radios.len() as u32,
            duration: 0,
            cover_art: None,
        });
    }

    // Playlist 1+: Subsonic playlists
    if let Ok(sub) = subsonic(&state) {
        if let Ok(playlists) = sub.get_playlists().await {
            result.extend(playlists.into_iter().map(|p| PlaylistInfo {
                id: p.id,
                name: p.name,
                song_count: p.song_count,
                duration: p.duration,
                cover_art: p.cover_art,
            }));
        }
    }

    Ok::<_, StatusCode>(Json(result))
}

async fn get_playlist(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if id == "radio" {
        return Ok(Json(serde_json::json!({
            "id": "radio",
            "name": "Radio",
            "tracks": state.config.radios.len(),
        })));
    }
    let sub = subsonic(&state)?;
    match sub.get_playlist(&id).await {
        Ok(playlist) => Ok(Json(serde_json::json!({
            "id": playlist.id,
            "name": playlist.name,
            "tracks": playlist.entry.len(),
        }))),
        Err(e) => {
            tracing::error!(error = %e, "Failed to fetch playlist");
            Err(StatusCode::BAD_GATEWAY)
        }
    }
}

async fn get_playlist_cover(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let sub = subsonic(&state)?;
    match sub.get_cover_art(&id).await {
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
    Path(id): Path<String>,
) -> impl IntoResponse {
    if id == "radio" {
        return Ok(Json(
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
        ));
    }
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

async fn get_playlist_track(
    State(state): State<SharedState>,
    Path((playlist_id, track_index)): Path<(String, usize)>,
) -> impl IntoResponse {
    let sub = subsonic(&state)?;
    match sub.get_playlist(&playlist_id).await {
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
