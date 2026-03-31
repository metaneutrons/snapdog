# Project Structure

## Repository Layout
```
SnapDogRust/
├── Cargo.toml                  # Workspace root
├── snapdog/                    # Main binary crate
│   ├── Cargo.toml
│   ├── build.rs                # Compiles vendored libshairplay
│   └── src/
│       ├── main.rs             # Entry point: config loading, service startup, shutdown
│       ├── config/             # TOML parsing, validation, derived config generation
│       ├── audio/              # Symphonia decoding, PCM pipeline, format conversion
│       ├── airplay/            # libshairplay FFI wrapper, safe Rust API
│       ├── snapcast/           # Server lifecycle, JSON-RPC control, TCP source feeding
│       ├── api/                # axum REST endpoints + WebSocket notifications
│       ├── mqtt/               # rumqttc client, topic schema, command/status handling
│       ├── knx/                # knxkit integration, address generation, telegram handling
│       ├── subsonic/           # Subsonic API client (playlists, streaming, cover art)
│       ├── state/              # In-memory state, JSON persistence, zone/client models
│       └── process/            # Child process management for snapserver
├── vendor/
│   └── shairplay/              # Git submodule: juhovh/shairplay
├── docs/
│   └── architecture/           # Design decisions and rationale
├── devcontainer/               # Docker dev environment configs
│   └── music/                  # Test music files for Navidrome
├── .kiro/
│   └── steering/               # Product, structure, and tech steering docs
├── .github/
│   └── workflows/
│       └── ci.yml              # Build, test, clippy, rustfmt
├── docker-compose.dev.yml      # Development test rig
├── snapdog.example.toml        # Example configuration
├── rustfmt.toml                # Formatter config
├── .clippy.toml                # Linter config
├── .editorconfig               # Editor settings
├── .gitignore
├── .gitmodules                 # vendor/shairplay submodule
├── LICENSE                     # GPL-3.0-only
└── README.md
```

## Module Responsibilities

| Module | Responsibility | External Dependencies |
|--------|---------------|----------------------|
| `config` | Load TOML, validate, generate derived config | `toml`, `serde` |
| `audio` | Decode streams to PCM, route to zones | `symphonia` |
| `airplay` | AirPlay 1 receiver (RAOP) | `libshairplay` (vendored C) |
| `snapcast` | Server management + JSON-RPC control | `snapcast-control` |
| `api` | REST + WebSocket server | `axum` |
| `mqtt` | MQTT command/status bridge | `rumqttc` |
| `knx` | KNX/IP telegram bridge | `knxkit` |
| `subsonic` | Music library API client | `reqwest` |
| `state` | Application state + persistence | `serde_json` |
| `process` | Child process lifecycle | `tokio::process` |

## Data Flow
```
AirPlay Client ──RAOP──▶ airplay (libshairplay FFI)
                              │ PCM
Subsonic/Radio ──HTTP──▶ audio (symphonia decode)
                              │ PCM
                              ▼
                         state (zone routing)
                              │
                              ▼ TCP write
                         snapcast (feeds snapserver)
                              │
                         snapserver ──▶ Snapcast Clients
                              │
                         snapcast (JSON-RPC control)
                              │
                    ┌─────────┼─────────┐
                    ▼         ▼         ▼
                   api      mqtt       knx
                (axum)   (rumqttc)  (knxkit)
```
