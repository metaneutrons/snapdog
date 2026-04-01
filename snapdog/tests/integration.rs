// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Integration tests — requires snapserver installed locally.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::timeout;

use snapdog::config::{self, AppConfig, AudioConfig, RawConfig};
use snapdog::player::{self, ZoneCommand, ZoneCommandSender, ZonePlayerContext};
use snapdog::state;

/// Build a test config with a free TCP source port.
fn test_config(tcp_port: u16) -> Arc<AppConfig> {
    let toml = format!(
        r#"
        [system]
        log_level = "debug"

        [audio]
        sample_rate = 48000
        bit_depth = 16
        channels = 2

        [snapcast]
        address = "localhost"
        streaming_port = 1704
        managed = false

        [[zone]]
        name = "Test Zone"

        [[client]]
        name = "Test Client"
        mac = "00:00:00:00:00:01"
        zone = "Test Zone"

        [[radio]]
        name = "DLF Test"
        url = "https://st01.sslstream.dlf.de/dlf/01/high/aac/stream.aac"

        [[radio]]
        name = "DLF Kultur Test"
        url = "https://st02.sslstream.dlf.de/dlf/02/high/aac/stream.aac"
    "#
    );
    let mut raw: RawConfig = toml::from_str(&toml).unwrap();
    // Override TCP source port to our free port
    let mut config = config::load_raw(raw).unwrap();
    config.zones[0].tcp_source_port = tcp_port;
    Arc::new(config)
}

/// Start a fake TCP listener that accepts connections and reads PCM data.
async fn fake_snapcast_source() -> (TcpListener, u16) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    (listener, port)
}

/// Start zone players with a fake Snapcast command sink.
async fn start_zone_players(
    config: Arc<AppConfig>,
) -> (
    state::SharedState,
    HashMap<usize, ZoneCommandSender>,
    state::cover::SharedCoverCache,
) {
    let store = state::init(&config, None).unwrap();
    let covers = state::cover::new_cache();
    let (notify_tx, _) = tokio::sync::broadcast::channel(64);
    let (snap_cmd_tx, _snap_cmd_rx) = mpsc::channel::<player::SnapcastCmd>(64);

    let zone_commands = player::spawn_zone_players(ZonePlayerContext {
        config: config.clone(),
        store: store.clone(),
        covers: covers.clone(),
        notify: notify_tx,
        snap_tx: snap_cmd_tx,
        client_mac_map: HashMap::new(),
        group_ids: Vec::new(),
        group_clients: HashMap::new(),
    })
    .await
    .unwrap();

    (store, zone_commands, covers)
}

// ── Tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn play_radio_delivers_pcm() {
    let (listener, port) = fake_snapcast_source().await;
    let config = test_config(port);

    // Accept TCP connection first (ZonePlayer connects on startup)
    let accept_handle = tokio::spawn(async move {
        timeout(Duration::from_secs(5), listener.accept())
            .await
            .expect("TCP accept timeout")
            .unwrap()
    });

    let (_store, cmds, _) = start_zone_players(config).await;
    let (mut stream, _) = accept_handle.await.unwrap();

    // Start radio
    cmds[&1].send(ZoneCommand::PlayRadio(0)).await.unwrap();

    // Verify PCM data arrives
    let mut buf = vec![0u8; 4096];
    let n = timeout(Duration::from_secs(10), stream.read(&mut buf))
        .await
        .expect("PCM read timeout")
        .unwrap();
    assert!(n > 0, "Should receive PCM data from radio stream");
}

#[tokio::test]
async fn play_radio_updates_state() {
    let (listener, port) = fake_snapcast_source().await;
    let config = test_config(port);
    let (store, cmds, _) = start_zone_players(config).await;

    // Accept connection
    let _ = timeout(Duration::from_secs(3), listener.accept()).await;

    cmds[&1].send(ZoneCommand::PlayRadio(0)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    let s = store.read().await;
    let zone = s.zones.get(&1).unwrap();
    assert_eq!(zone.playback, state::PlaybackState::Playing);
    assert_eq!(zone.source, state::SourceType::Radio);
    assert_eq!(zone.radio_index, Some(0));
    assert!(zone.track.is_some());
    assert_eq!(zone.track.as_ref().unwrap().title, "DLF Test");
}

#[tokio::test]
async fn stop_clears_playback() {
    let (listener, port) = fake_snapcast_source().await;
    let config = test_config(port);
    let accept_handle = tokio::spawn(async move {
        timeout(Duration::from_secs(5), listener.accept())
            .await
            .ok()
    });
    let (store, cmds, _) = start_zone_players(config).await;
    let _ = accept_handle.await;

    cmds[&1].send(ZoneCommand::PlayRadio(0)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    cmds[&1].send(ZoneCommand::Stop).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let s = store.read().await;
    let zone = s.zones.get(&1).unwrap();
    assert_eq!(zone.playback, state::PlaybackState::Stopped);
    assert_eq!(zone.source, state::SourceType::Idle);
    assert!(zone.track.is_none());
}

#[tokio::test]
async fn next_radio_cycles_stations() {
    let (listener, port) = fake_snapcast_source().await;
    let config = test_config(port);

    // Keep listener alive to accept reconnections on Next
    let accept_handle = tokio::spawn(async move {
        loop {
            let _ = listener.accept().await;
        }
    });

    let (store, cmds, _) = start_zone_players(config).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    cmds[&1].send(ZoneCommand::PlayRadio(0)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    cmds[&1].send(ZoneCommand::Next).await.unwrap();
    tokio::time::sleep(Duration::from_millis(2000)).await;

    let s = store.read().await;
    let zone = s.zones.get(&1).unwrap();
    assert_eq!(
        zone.radio_index,
        Some(1),
        "Should have advanced to station 1"
    );

    accept_handle.abort();
}

#[tokio::test]
async fn volume_set_and_read() {
    let (listener, port) = fake_snapcast_source().await;
    let config = test_config(port);
    let accept_handle = tokio::spawn(async move {
        timeout(Duration::from_secs(5), listener.accept())
            .await
            .ok()
    });
    let (store, cmds, _) = start_zone_players(config).await;
    let _ = accept_handle.await;

    cmds[&1].send(ZoneCommand::SetVolume(80)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let s = store.read().await;
    assert_eq!(s.zones.get(&1).unwrap().volume, 80);
}

#[tokio::test]
async fn mute_toggle() {
    let (listener, port) = fake_snapcast_source().await;
    let config = test_config(port);
    let accept_handle = tokio::spawn(async move {
        timeout(Duration::from_secs(5), listener.accept())
            .await
            .ok()
    });
    let (store, cmds, _) = start_zone_players(config).await;
    let _ = accept_handle.await;

    cmds[&1].send(ZoneCommand::SetMute(true)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(store.read().await.zones.get(&1).unwrap().muted);

    cmds[&1].send(ZoneCommand::ToggleMute).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!store.read().await.zones.get(&1).unwrap().muted);
}

#[tokio::test]
async fn shuffle_repeat_state() {
    let (listener, port) = fake_snapcast_source().await;
    let config = test_config(port);
    let accept_handle = tokio::spawn(async move {
        timeout(Duration::from_secs(5), listener.accept())
            .await
            .ok()
    });
    let (store, cmds, _) = start_zone_players(config).await;
    let _ = accept_handle.await;

    cmds[&1].send(ZoneCommand::SetShuffle(true)).await.unwrap();
    cmds[&1].send(ZoneCommand::SetRepeat(true)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let s = store.read().await;
    let zone = s.zones.get(&1).unwrap();
    assert!(zone.shuffle);
    assert!(zone.repeat);
}

#[tokio::test]
async fn icy_metadata_updates_title() {
    let (listener, port) = fake_snapcast_source().await;
    let config = test_config(port);

    let accept_handle = tokio::spawn(async move {
        timeout(Duration::from_secs(5), listener.accept())
            .await
            .ok()
    });

    let (store, cmds, _) = start_zone_players(config).await;
    let _ = accept_handle.await;

    cmds[&1].send(ZoneCommand::PlayRadio(0)).await.unwrap();

    // Wait for ICY metadata (DLF sends it within a few seconds)
    let mut got_icy = false;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let s = store.read().await;
        let title = s
            .zones
            .get(&1)
            .and_then(|z| z.track.as_ref().map(|t| t.title.clone()));
        if let Some(t) = title {
            if t != "DLF Test" {
                got_icy = true;
                break;
            }
        }
    }
    assert!(got_icy, "ICY metadata should update the track title");
}

// ── Subsonic tests (conditional) ──────────────────────────────

fn subsonic_config() -> Option<config::SubsonicConfig> {
    let _ = dotenvy::from_filename(".env.test");
    let url = std::env::var("SNAPDOG_TEST_SUBSONIC_URL").ok()?;
    let username = std::env::var("SNAPDOG_TEST_SUBSONIC_USERNAME").ok()?;
    let password = std::env::var("SNAPDOG_TEST_SUBSONIC_PASSWORD").ok()?;
    if url.is_empty() || username.is_empty() {
        return None;
    }
    Some(config::SubsonicConfig {
        url,
        username,
        password,
    })
}

#[tokio::test]
async fn subsonic_ping() {
    let Some(cfg) = subsonic_config() else {
        eprintln!("Skipping subsonic_ping — no credentials in .env.test");
        return;
    };
    let client = snapdog::subsonic::SubsonicClient::new(&cfg);
    client.ping().await.expect("Subsonic ping should succeed");
}

#[tokio::test]
async fn subsonic_playlists_not_empty() {
    let Some(cfg) = subsonic_config() else {
        eprintln!("Skipping subsonic_playlists — no credentials in .env.test");
        return;
    };
    let client = snapdog::subsonic::SubsonicClient::new(&cfg);
    let playlists = client
        .get_playlists()
        .await
        .expect("Should fetch playlists");
    assert!(!playlists.is_empty(), "Should have at least one playlist");
}
