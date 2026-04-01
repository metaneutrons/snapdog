# SnapDog

Multi-zone audio controller with AirPlay, Snapcast, MQTT, and KNX integration.

> Enterprise-grade Rust rewrite of [SnapDog2](https://github.com/metaneutrons/snapdog) (.NET).

## What it does

SnapDog is a single binary that turns a Linux box into a multi-room audio system with smart home integration:

- **AirPlay receiver** — one per zone, stream from iPhone/Mac directly into multi-room audio
- **Snapcast integration** — synchronized playback across rooms, managed as child process
- **Subsonic/Navidrome** — play from your personal music library with playlist navigation and seek
- **Internet radio** — configurable station list with live ICY metadata (current song title)
- **MQTT** — bidirectional smart home integration (commands + status)
- **KNX** — building automation protocol support
- **REST API** — ~90 endpoints, full zone/client/media control
- **WebSocket** — real-time state notifications for WebUI
- **Cover art** — unified proxy endpoint, works with all sources

## Quick Start

```bash
# One-time setup
make setup          # Configure git hooks

# Start dev infrastructure
docker compose -f docker-compose.dev.yml up -d

# Build and run
cargo run -- --config snapdog.dev.toml

# In another terminal: listen via Snapcast
snapclient localhost
```

**Access:**
- API: http://localhost:5555/api/v1/zones
- Health: http://localhost:5555/health
- WebSocket: ws://localhost:5555/ws

## Configuration

One file: `snapdog.toml`. See [snapdog.example.toml](snapdog.example.toml).

KNX addresses, Snapcast sink paths, stream names, and AirPlay names are auto-generated from zone/client definitions. Override only what deviates from convention.

```toml
[[zone]]
name = "Ground Floor"
# → sink: /snapsinks/zone1, stream: Zone1, KNX: 1/x/y
# → AirPlay: "SnapDog Ground Floor"

[[client]]
name = "Living Room"
mac = "02:42:ac:11:00:10"
zone = "Ground Floor"
# → KNX: 3/1/x
```

## Architecture

- **ZonePlayer** — per-zone tokio task with command channel, owns audio pipeline
- **24 commands** — play/pause/stop, next/previous, seek, volume, mute, shuffle, repeat, playlist navigation
- **5 source types** — Radio, Subsonic Playlist, Subsonic Track, URL, AirPlay
- **AirPlay preemption** — AirPlay stops current source, zone goes idle when AirPlay ends
- **Volume via Snapcast** — never PCM amplitude scaling, full dynamic range preserved
- **Resampling** — persistent rubato resampler for 44.1kHz→48kHz (AirPlay) or any mismatch

See [docs/architecture/decisions.md](docs/architecture/decisions.md) for 13 Architecture Decision Records.

## Development

```bash
cargo build         # Build
cargo test          # Run all tests (use --test-threads=1 for integration tests)
cargo run -- --config snapdog.dev.toml
```

## Dev Infrastructure (Docker Compose)

| Service | Purpose |
|---------|---------|
| snapcast-server | Multi-room audio (build from source, FLAC) |
| 3× snapclient | Simulated rooms (Living Room, Kitchen, Bedroom) |
| mqtt | Mosquitto MQTT broker |
| knxd | KNX gateway simulator |
| knx-monitor | Visual KNX bus debugging |
| navidrome | Subsonic-compatible music server |

## License

GPL-3.0-only. See [LICENSE](LICENSE).
