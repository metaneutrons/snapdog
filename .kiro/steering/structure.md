# Project Structure

## Repository Layout
```
SnapDogRust/
├── Cargo.toml                  # Workspace root
├── snapdog/                    # Main binary crate
│   ├── Cargo.toml
│   ├── build.rs                # Compiles vendored libshairplay
│   └── src/
│       ├── main.rs             # Entry point, Snapcast command loop, shutdown
│       ├── config/             # TOML parsing, validation, convention-over-config
│       │   ├── mod.rs          # load(), load_raw()
│       │   ├── types.rs        # Raw + resolved config structs, defaults
│       │   └── convention.rs   # Auto-generate KNX addresses, sinks, streams
│       ├── player/             # ZonePlayer — per-zone audio pipeline
│       │   ├── mod.rs          # Re-exports
│       │   ├── commands.rs     # ZoneCommand enum (24 variants), ActiveSource
│       │   ├── context.rs      # ZonePlayerContext, SnapcastCmd, update_and_notify,
│       │   │                     stop_decode, setup_zone_group
│       │   ├── runner.rs       # spawn_zone_players(), run() select! loop
│       │   └── helpers.rs      # DecodeState, handle_next/previous/track_complete,
│       │                         advance_playlist_track, subsonic/radio helpers
│       ├── audio/              # Audio decoding and processing
│       │   ├── mod.rs          # decode_http_stream (symphonia), PCM channel
│       │   ├── resample.rs     # PcmResampler (rubato), Resampling enum
│       │   └── icy.rs          # ICY metadata parsing (Icecast/Shoutcast)
│       ├── airplay/            # AirPlay 1 receiver
│       │   ├── mod.rs          # AirplayReceiver, AirplayEvent, DMAP parser
│       │   └── ffi.rs          # Raw C bindings for libshairplay
│       ├── snapcast/           # Snapcast integration
│       │   └── mod.rs          # Snapcast struct (JSON-RPC), open_audio_source
│       ├── api/                # REST API + WebSocket
│       │   ├── mod.rs          # AppState, serve()
│       │   ├── health.rs       # /health, /health/ready, /health/live
│       │   ├── ws.rs           # WebSocket notifications + incoming commands
│       │   └── routes/
│       │       ├── zones.rs    # ~55 zone endpoints, VolumeValue
│       │       ├── clients.rs  # ~18 client endpoints
│       │       ├── media.rs    # Subsonic playlist/track proxy
│       │       └── system.rs   # Status, version
│       ├── mqtt/               # MQTT bridge
│       │   └── mod.rs          # MqttBridge, command routing via ZonePlayer
│       ├── knx/                # KNX/IP integration
│       │   └── mod.rs          # Address parsing, DPT encoding, remote URL
│       ├── subsonic/           # Subsonic API client
│       │   └── mod.rs          # SubsonicClient, token auth, playlists, streaming
│       ├── state/              # Application state
│       │   ├── mod.rs          # Store, ZoneState, ClientState, TrackInfo, persistence
│       │   └── cover.rs        # CoverCache, MIME detection, fetch helper
│       └── process/            # Child process management
│           └── mod.rs          # SnapserverHandle, config generation
├── vendor/
│   └── shairplay/              # Git submodule: juhovh/shairplay
├── devcontainer/               # Docker dev environment
│   ├── snapcast-server/        # Build-from-source Dockerfile (FLAC only)
│   ├── snapcast-client/        # Alpine package Dockerfile
│   ├── knxd/                   # KNX gateway simulator
│   ├── mosquitto/              # MQTT broker config
│   ├── music/                  # Test music for Navidrome
│   └── knx-groupaddresses.csv  # KNX group address database
├── docs/
│   └── architecture/
│       └── decisions.md        # 13 ADRs
├── .kiro/
│   └── steering/               # product.md, structure.md, tech.md
├── .github/workflows/ci.yml
├── .githooks/                  # pre-commit (fmt), pre-push (clippy)
├── docker-compose.dev.yml      # 8-service dev rig
├── snapdog.example.toml
├── snapdog.dev.toml
├── Makefile
├── rustfmt.toml
├── .clippy.toml
├── .editorconfig
├── .gitignore
├── LICENSE                     # GPL-3.0-only
└── README.md
```

## Module Responsibilities

| Module | Lines | Responsibility |
|--------|-------|---------------|
| `config` | ~350 | TOML parsing, convention-over-config, validation |
| `player` | ~1020 | ZonePlayer: commands, context, select loop, track navigation |
| `audio` | ~350 | HTTP stream decode (symphonia), resampling (rubato), ICY metadata |
| `airplay` | ~300 | AirPlay 1 FFI, DMAP parsing, all callbacks |
| `snapcast` | ~160 | JSON-RPC control, TCP audio source, group volume |
| `api` | ~700 | REST (~90 endpoints), WebSocket, health |
| `mqtt` | ~230 | Bidirectional MQTT bridge, command routing |
| `knx` | ~120 | Address parsing, DPT encoding |
| `subsonic` | ~200 | Subsonic API client, token auth |
| `state` | ~350 | In-memory store, JSON persistence, cover cache |
| `process` | ~120 | Snapserver lifecycle, config generation |

## Data Flow
```
AirPlay Client ──RAOP──▶ airplay (FFI) ──PCM──▶ player (resampler) ──▶ TCP ──▶ snapserver
Subsonic/Radio ──HTTP──▶ audio (symphonia) ──PCM──▶ player (resampler) ──▶ TCP ──▶ snapserver

API/MQTT/WS/KNX ──ZoneCommand──▶ player ──state update──▶ state store
                                    │                         │
                                    ├──SnapcastCmd──▶ main loop ──▶ snapcast (JSON-RPC)
                                    └──Notification──▶ WebSocket broadcast
```
