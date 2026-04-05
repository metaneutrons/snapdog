# SnapDog

Multi-zone audio controller with AirPlay, Snapcast, MQTT, and KNX integration.

> Enterprise-grade Rust rewrite of [SnapDog2](https://github.com/metaneutrons/snapdog) (.NET).
> Sound. Everywhere.

## What it does

SnapDog is a single binary that turns a Linux box (or Mac) into a multi-room audio system with smart home integration:

- **AirPlay 1 + 2 receiver** — one per zone, stream from iPhone/Mac directly into multi-room audio
- **Snapcast integration** — synchronized playback across rooms, managed as child process
- **Subsonic/Navidrome** — play from your personal music library with playlist navigation and seek
- **Internet radio** — configurable station list with live ICY metadata (current song title)
- **HLS streaming** — segment-based streaming with retry logic and metadata extraction
- **MQTT** — bidirectional smart home integration (commands + status)
- **KNX** — building automation protocol support (explicit GA configuration)
- **REST API** — ~90 endpoints, full zone/client/media control
- **WebSocket** — real-time state notifications (server → client push)
- **Embedded WebUI** — responsive SPA with zone control, volume sliders, drag-and-drop client management
- **Cover art** — unified endpoint per zone with content-addressed caching
- **Spotify Connect** — receiver support planned (ADR-015, librespot integration)

## Quick Start

```bash
# One-time setup
make setup          # Configure git hooks

# Start dev infrastructure
docker compose -f docker-compose.dev.yml up -d

# Build and run
cargo run -- --config snapdog.dev.toml

# With AirPlay 2 support
cargo run --features ap2 -- --config snapdog.dev.toml
```

**Access:**
- WebUI: http://localhost:5555
- API: http://localhost:5555/api/v1/zones
- Health: http://localhost:5555/health
- WebSocket: ws://localhost:5555/ws

## Configuration

One file: `snapdog.toml`. See [snapdog.example.toml](snapdog.example.toml).

Snapcast sink paths, stream names, and AirPlay names are auto-generated from zone/client definitions. KNX addresses are explicit (no auto-generation — fits into existing KNX installations).

```toml
[audio]
sample_rate = 48000
bit_depth = 16        # 16, 24, or 32
channels = 2

[subsonic]
url = "https://music.example.com"
username = "user"
password = "pass"
format = "flac"       # "flac" (default), "raw", "mp3", "opus"

[[zone]]
name = "Ground Floor"
# → sink: /snapsinks/zone1, stream: Zone1
# → AirPlay: "SnapDog Ground Floor"

[[client]]
name = "Living Room"
mac = "02:42:ac:11:00:10"
zone = "Ground Floor"

[[radio]]
name = "Deutschlandfunk"
url = "https://st01.sslstream.dlf.de/dlf/01/high/aac/stream.aac"
cover = "https://upload.wikimedia.org/wikipedia/commons/thumb/..."
```

### Subsonic Server Notes

When using Navidrome (or other Subsonic-compatible servers), ensure transcoding is configured for the format specified in `format`. For example, with `format = "flac"`:

- Navidrome: Settings → Transcoding → add a FLAC transcoding rule
- Without transcoding, files in non-streamable containers (ALAC/AAC in MP4) will be downloaded fully before playback, causing significant latency

## Architecture

- **ZonePlayer** — per-zone tokio task with command channel, owns audio pipeline
- **ReceiverProvider trait** — pluggable passive audio receivers (AirPlay, future Spotify Connect)
- **24 commands** — play/pause/stop, next/previous, seek, volume, mute, shuffle, repeat, playlist navigation
- **5 source types** — Radio, Subsonic Playlist, Subsonic Track, URL, AirPlay
- **Unified playlist model** — radio stations and Subsonic playlists in single numeric index
- **AirPlay preemption** — AirPlay stops current source, zone goes idle when AirPlay ends
- **Volume via Snapcast** — never PCM amplitude scaling, full dynamic range preserved
- **Resampling** — dynamic resampler creation from actual decoded sample rate (S16LE for active sources, F32 for receivers)
- **Configurable output** — bit depth (16/24/32), sample rate, channels — SSOT with Snapcast
- **Event-driven Snapcast** — own JSON-RPC client, fire-and-forget commands, state from server responses
- **Cover art** — content-addressed caching with CRC32 hash, unified `/zones/{id}/cover` endpoint

### WebUI

- **Desktop** (≥1280px): all zones side-by-side in flex-wrap grid
- **Tablet** (768–1279px): sidebar + single zone with horizontal card layout
- **Mobile** (<768px): tab bar + full-width zone card
- **REST for commands** — all user actions via REST API
- **WebSocket for state** — real-time push notifications, auto-reload on reconnect
- **Unified VolumeSlider** — shared component for zones and clients with debouncing, auto-unmute
- **Drag-and-drop** — drag client chips to sidebar zones to reassign

### Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `ap2` | off | AirPlay 2 support (encrypted transport, HAP pairing) |
| `spotify` | off | Spotify Connect receiver (librespot, WIP) |

See [docs/architecture/decisions.md](docs/architecture/decisions.md) for 18 Architecture Decision Records.

## Development

```bash
cargo build                    # Build (AP1 only)
cargo build --features ap2     # Build with AirPlay 2
cargo test                     # Run all tests
cargo clippy -- -D warnings    # Lint
cargo fmt -- --check           # Format check
docker compose -f docker-compose.dev.yml up  # Start test rig
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
