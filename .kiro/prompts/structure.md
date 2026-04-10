# Project Structure

## Repository Layout
```
SnapDogRust/
├── Cargo.toml                  # Workspace root
├── snapdog/                    # Main binary crate
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs             # Entry point, Snapcast command loop, shutdown
│       ├── config/             # TOML parsing, validation, convention-over-config
│       │   ├── mod.rs          # load(), load_raw()
│       │   ├── types.rs        # Raw + resolved config structs, defaults,
│       │   │                     ResolvedPlaylist, unified playlist resolution
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
│       │   ├── mod.rs          # decode_http_stream (symphonia), decode_hls_stream,
│       │   │                     resolve_playlist_url, PCM channel, retry logic
│       │   ├── resample.rs     # PcmResampler (rubato), Resampling enum
│       │   └── icy.rs          # ICY metadata parsing (Icecast/Shoutcast)
│       ├── airplay/            # AirPlay 1 + 2 receiver (pure Rust)
│       │   └── mod.rs          # AirplayReceiver, AirplayEvent, BridgeHandler,
│       │                         FilePairingStore, DMAP parser, F32→S16LE
│       ├── snapcast/           # Snapcast integration
│       │   └── mod.rs          # Snapcast struct (JSON-RPC), open_audio_source
│       ├── api/                # REST API + WebSocket
│       │   ├── mod.rs          # AppState, serve()
│       │   ├── health.rs       # /health, /health/ready, /health/live
│       │   ├── ws.rs           # WebSocket notifications + incoming commands
│       │   └── routes/
│       │       ├── zones.rs    # ~55 zone endpoints, VolumeValue
│       │       ├── clients.rs  # ~18 client endpoints
│       │       ├── media.rs    # Unified playlist/track/cover proxy
│       │       └── system.rs   # Status, version
│       ├── mqtt/               # MQTT bridge
│       │   └── mod.rs          # MqttBridge, command routing via ZonePlayer
│       ├── knx/                # KNX/IP integration
│       │   └── mod.rs          # Address parsing, DPT encoding, remote URL
│       ├── subsonic/           # Subsonic API client
│       │   └── mod.rs          # SubsonicClient, token auth, playlists, streaming
│       ├── state/              # Application state
│       │   ├── mod.rs          # Store, ZoneState, ClientState, TrackInfo, persistence
│       │   └── cover.rs        # CoverCache (AirPlay only), fetch_cover, MIME detection
│       └── process/            # Child process management
│           └── mod.rs          # SnapserverHandle, config generation
├── webui/                      # Next.js WebUI (static export, embedded in binary)
│   ├── package.json
│   ├── next.config.ts
│   ├── src/
│   │   ├── app/                # App Router — page.tsx (responsive zone grid)
│   │   ├── components/         # NowPlaying, PlaylistBrowser, TransportControls,
│   │   │   │                     SeekBar, ShuffleRepeat, ClientList, Marquee
│   │   │   └── ui/             # shadcn/ui primitives
│   │   ├── hooks/              # useWebSocket
│   │   ├── lib/                # API client, types, utils
│   │   └── stores/             # useAppStore (Zustand)
│   └── out/                    # Static export output (gitignored)
├── devcontainer/               # Docker dev environment
│   ├── snapcast-server/        # Build-from-source Dockerfile (FLAC only)
│   ├── snapcast-client/        # Alpine package Dockerfile
│   ├── knxd/                   # KNX gateway simulator
│   ├── mosquitto/              # MQTT broker config
│   ├── music/                  # Test music for Navidrome
│   └── knx-groupaddresses.csv  # KNX group address database
├── docs/
│   └── architecture/
│       └── decisions.md        # 17 ADRs
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
| `config` | ~400 | TOML parsing, convention-over-config, validation, ResolvedPlaylist |
| `player` | ~1020 | ZonePlayer: commands, context, select loop, track navigation |
| `audio` | ~500 | HTTP/HLS stream decode (symphonia), resampling (rubato), ICY metadata, retry |
| `airplay` | ~250 | Pure Rust AirPlay 1+2 bridge (shairplay crate), F32→S16LE, DMAP, pairing |
| `snapcast` | ~160 | JSON-RPC control, TCP audio source, group volume |
| `api` | ~700 | REST (~90 endpoints), WebSocket, health |
| `mqtt` | ~230 | Bidirectional MQTT bridge, command routing |
| `knx` | ~120 | Address parsing, DPT encoding |
| `subsonic` | ~200 | Subsonic API client, token auth |
| `state` | ~350 | In-memory store, JSON persistence, cover cache (AirPlay only) |
| `process` | ~120 | Snapserver lifecycle, config generation |

## Data Flow
```
┌─────────────────────────────────────────────────────────────────────┐
│ Sources                                                             │
│                                                                     │
│  iPhone ──AirPlay 1+2──▶ shairplay crate (pure Rust)              │
│                              ├── F32 PCM → S16LE → resampler      │
│                              ├── DMAP metadata ──▶ state.track     │
│                              ├── Cover art ──▶ cover cache (AirPlay)│
│                              ├── Progress ──▶ state.track.position │
│                              └── RemoteControl ──▶ transport cmds  │
│                                                                     │
│  Subsonic ──HTTP/JSON──▶ subsonic client                           │
│                              ├── stream URL ──▶ audio (symphonia)  │
│                              │                    └── PCM ──▶ resampler
│                              └── metadata ──▶ state.track          │
│                                                                     │
│  Radio ──HTTP──▶ audio (symphonia + ICY parser)                    │
│                    ├── PCM ──▶ resampler                           │
│                    └── ICY StreamTitle ──▶ state.track.artist      │
│                                                                     │
│  Cover art: deterministic endpoint per playlist/track index        │
│    Radio → config URL, Subsonic → getCoverArt API                  │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│ ZonePlayer (one per zone)                                           │
│                                                                     │
│  ZoneCommand ◄── API (REST)                                        │
│              ◄── MQTT (rumqttc)                                    │
│              ◄── WebSocket (incoming)                              │
│              ◄── KNX (knxkit)                                      │
│                                                                     │
│  PCM ──▶ resampler ──▶ TCP write ──▶ snapserver ──▶ Snapcast Clients
│                                         ▲                          │
│  SnapcastCmd ──▶ main loop ──▶ snapcast (JSON-RPC)                 │
│    (SetGroupVolume, SetGroupStream, SetGroupClients, ...)          │
│                                                                     │
│  State update ──▶ state store ──▶ WebSocket broadcast              │
│                                ──▶ MQTT publish (retained)         │
│                                ──▶ KNX group write                 │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│ Outputs                                                             │
│                                                                     │
│  REST API ◄── state store (read-only for API)                      │
│    GET /zones/1/track/metadata ──▶ JSON (includes cover_url)       │
│    GET /media/playlists/{pi}/tracks/{ti}/cover ──▶ image bytes     │
│                                                                     │
│  WebSocket ◄── broadcast channel ◄── every state change            │
│    {"type":"zone_state_changed", "zone":1, "playback":"playing"}   │
│                                                                     │
│  MQTT ──▶ retained status topics                                   │
│    snapdog/zones/1/track/title = "Moonlight Sonata"                │
│                                                                     │
│  KNX ──▶ group value writes                                       │
│    1/3/10 (DPT 16.001) = "Moonlight Sonat"                        │
│                                                                     │
│  Snapcast Clients ◄── snapserver ◄── TCP PCM                      │
│    Living Room (02:42:ac:11:00:10)                                 │
│    Kitchen     (02:42:ac:11:00:11)                                 │
│    Bedroom     (02:42:ac:11:00:12)                                 │
└─────────────────────────────────────────────────────────────────────┘
```
