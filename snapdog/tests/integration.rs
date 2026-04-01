// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder

//! Integration tests — uses real snapserver (must be installed locally).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::mpsc;

use snapdog::config::{self, AppConfig, RawConfig};
use snapdog::player::{self, ZoneCommand, ZoneCommandSender, ZonePlayerContext};
use snapdog::process::SnapserverHandle;
use snapdog::snapcast::Snapcast;
use snapdog::state;

// ── Test Harness ──────────────────────────────────────────────

/// Find a free TCP port.
async fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    l.local_addr().unwrap().port()
}

/// Build a test config with unique free ports for snapserver.
async fn test_config() -> (Arc<AppConfig>, u16, u16, u16) {
    let streaming_port = free_port().await;
    let jsonrpc_port = free_port().await;
    let http_port = free_port().await;
    let tcp_source_port_1 = free_port().await;
    let tcp_source_port_2 = free_port().await;

    let toml = format!(
        r#"
        [system]
        log_level = "info"

        [snapcast]
        address = "127.0.0.1"
        streaming_port = {streaming_port}
        jsonrpc_port = {http_port}
        managed = false

        [[zone]]
        name = "Test Zone 1"

        [[zone]]
        name = "Test Zone 2"

        [[client]]
        name = "Test Client"
        mac = "00:00:00:00:00:01"
        zone = "Test Zone 1"

        [[radio]]
        name = "DLF Test"
        url = "https://st01.sslstream.dlf.de/dlf/01/high/aac/stream.aac"

        [[radio]]
        name = "DLF Kultur Test"
        url = "https://st02.sslstream.dlf.de/dlf/02/high/aac/stream.aac"
    "#
    );

    let mut config = config::load_raw(toml::from_str::<RawConfig>(&toml).unwrap()).unwrap();
    config.zones[0].tcp_source_port = tcp_source_port_1;
    config.zones[1].tcp_source_port = tcp_source_port_2;

    (Arc::new(config), streaming_port, jsonrpc_port, http_port)
}

/// Generate a snapserver.conf for the given config and start snapserver.
async fn start_snapserver(config: &AppConfig) -> SnapserverHandle {
    let handle = SnapserverHandle::start(config).await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    handle
}

/// Start the full system: snapserver + snapcast + zone players.
async fn start_system(
    config: Arc<AppConfig>,
) -> (
    SnapserverHandle,
    state::SharedState,
    HashMap<usize, ZoneCommandSender>,
    state::cover::SharedCoverCache,
) {
    // Build a managed version of the config
    let toml_str = format!(
        r#"
        [system]
        log_level = "info"
        [snapcast]
        address = "127.0.0.1"
        streaming_port = {}
        jsonrpc_port = {}
        managed = true
        [[zone]]
        name = "Test Zone 1"
        [[zone]]
        name = "Test Zone 2"
        [[client]]
        name = "Test Client"
        mac = "00:00:00:00:00:01"
        zone = "Test Zone 1"
        [[radio]]
        name = "DLF Test"
        url = "https://st01.sslstream.dlf.de/dlf/01/high/aac/stream.aac"
        [[radio]]
        name = "DLF Kultur Test"
        url = "https://st02.sslstream.dlf.de/dlf/02/high/aac/stream.aac"
        "#,
        config.snapcast.streaming_port, config.snapcast.jsonrpc_port,
    );
    let mut managed_config =
        config::load_raw(toml::from_str::<RawConfig>(&toml_str).unwrap()).unwrap();
    managed_config.zones[0].tcp_source_port = config.zones[0].tcp_source_port;
    managed_config.zones[1].tcp_source_port = config.zones[1].tcp_source_port;
    managed_config.snapcast.managed = true;

    eprintln!(
        "Config: managed={}, streaming_port={}, tcp_source_ports={},{}",
        managed_config.snapcast.managed,
        managed_config.snapcast.streaming_port,
        managed_config.zones[0].tcp_source_port,
        managed_config.zones[1].tcp_source_port
    );
    let snapserver = SnapserverHandle::start(&managed_config).await.unwrap();
    eprintln!("Snapserver started, waiting...");
    tokio::time::sleep(Duration::from_secs(2)).await;
    eprintln!(
        "Connecting to snapcast on port {}",
        managed_config.snapcast.streaming_port + 1
    );

    let mut snap = Snapcast::from_config(&managed_config).await.unwrap();
    snap.init().await.unwrap();
    let snap_state = snap.state().clone();

    let store = state::init(&managed_config, None).unwrap();
    let covers = state::cover::new_cache();
    let (notify_tx, _) = tokio::sync::broadcast::channel(64);
    let (snap_cmd_tx, _) = mpsc::channel::<player::SnapcastCmd>(64);

    let zone_commands = player::spawn_zone_players(ZonePlayerContext {
        config: Arc::new(managed_config),
        store: store.clone(),
        covers: covers.clone(),
        notify: notify_tx,
        snap_tx: snap_cmd_tx,
        client_mac_map: snap_state
            .clients
            .iter()
            .map(|e| (e.value().host.mac.to_lowercase(), e.key().clone()))
            .collect(),
        group_ids: snap_state.groups.iter().map(|g| g.key().clone()).collect(),
        group_clients: snap_state
            .groups
            .iter()
            .map(|g| (g.key().clone(), g.clients.iter().cloned().collect()))
            .collect(),
    })
    .await
    .unwrap();

    (snapserver, store, zone_commands, covers)
}

// ── Tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn play_radio_with_real_snapserver() {
    let (config, _, _, _) = test_config().await;
    let (mut snapserver, store, cmds, _) = start_system(config).await;

    cmds[&1].send(ZoneCommand::PlayRadio(0)).await.unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;

    let s = store.read().await;
    let zone = s.zones.get(&1).unwrap();
    assert_eq!(zone.playback, state::PlaybackState::Playing);
    assert_eq!(zone.source, state::SourceType::Radio);
    assert!(zone.track.is_some());

    snapserver.stop().await.unwrap();
}

#[tokio::test]
async fn stop_clears_playback() {
    let (config, _, _, _) = test_config().await;
    let (mut snapserver, store, cmds, _) = start_system(config).await;

    cmds[&1].send(ZoneCommand::PlayRadio(0)).await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    cmds[&1].send(ZoneCommand::Stop).await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    let s = store.read().await;
    let zone = s.zones.get(&1).unwrap();
    assert_eq!(zone.playback, state::PlaybackState::Stopped);
    assert_eq!(zone.source, state::SourceType::Idle);
    assert!(zone.track.is_none());

    snapserver.stop().await.unwrap();
}

#[tokio::test]
async fn next_radio_cycles_stations() {
    let (config, _, _, _) = test_config().await;
    let (mut snapserver, store, cmds, _) = start_system(config).await;

    cmds[&1].send(ZoneCommand::PlayRadio(0)).await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    cmds[&1].send(ZoneCommand::Next).await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    let s = store.read().await;
    let zone = s.zones.get(&1).unwrap();
    assert_eq!(zone.radio_index, Some(1), "Should advance to station 1");

    snapserver.stop().await.unwrap();
}

#[tokio::test]
async fn volume_set_and_read() {
    let (config, _, _, _) = test_config().await;
    let (mut snapserver, store, cmds, _) = start_system(config).await;

    cmds[&1].send(ZoneCommand::SetVolume(80)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(store.read().await.zones.get(&1).unwrap().volume, 80);

    snapserver.stop().await.unwrap();
}

#[tokio::test]
async fn mute_toggle() {
    let (config, _, _, _) = test_config().await;
    let (mut snapserver, store, cmds, _) = start_system(config).await;

    cmds[&1].send(ZoneCommand::SetMute(true)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(store.read().await.zones.get(&1).unwrap().muted);

    cmds[&1].send(ZoneCommand::ToggleMute).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!store.read().await.zones.get(&1).unwrap().muted);

    snapserver.stop().await.unwrap();
}

#[tokio::test]
async fn shuffle_repeat_state() {
    let (config, _, _, _) = test_config().await;
    let (mut snapserver, store, cmds, _) = start_system(config).await;

    cmds[&1].send(ZoneCommand::SetShuffle(true)).await.unwrap();
    cmds[&1].send(ZoneCommand::SetRepeat(true)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let s = store.read().await;
    assert!(s.zones.get(&1).unwrap().shuffle);
    assert!(s.zones.get(&1).unwrap().repeat);

    snapserver.stop().await.unwrap();
}

#[tokio::test]
async fn icy_metadata_updates_title() {
    let (config, _, _, _) = test_config().await;
    let (mut snapserver, store, cmds, _) = start_system(config).await;

    cmds[&1].send(ZoneCommand::PlayRadio(0)).await.unwrap();

    // DLF sends ICY metadata within a few seconds
    let mut got_icy = false;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let s = store.read().await;
        if let Some(t) = s
            .zones
            .get(&1)
            .and_then(|z| z.track.as_ref().map(|t| t.title.clone()))
        {
            if t != "DLF Test" {
                got_icy = true;
                break;
            }
        }
    }
    assert!(got_icy, "ICY metadata should update the track title");

    snapserver.stop().await.unwrap();
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
        eprintln!("Skipping — no credentials in .env.test");
        return;
    };
    let client = snapdog::subsonic::SubsonicClient::new(&cfg);
    client.ping().await.expect("Subsonic ping should succeed");
}

#[tokio::test]
async fn subsonic_playlists_not_empty() {
    let Some(cfg) = subsonic_config() else {
        eprintln!("Skipping — no credentials in .env.test");
        return;
    };
    let client = snapdog::subsonic::SubsonicClient::new(&cfg);
    let playlists = client
        .get_playlists()
        .await
        .expect("Should fetch playlists");
    assert!(!playlists.is_empty(), "Should have at least one playlist");
}
