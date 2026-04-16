<div align="center">

<!-- Logo placeholder — replace with actual logo -->
<img src="https://raw.githubusercontent.com/metaneutrons/snapdog/main/assets/snapdog-logo.svg" alt="SnapDog" width="200">

**Multi-room audio system with smart home integration**

One binary. AirPlay + Snapcast + MQTT + KNX.

[![CI](https://github.com/metaneutrons/snapdog/actions/workflows/ci.yml/badge.svg)](https://github.com/metaneutrons/snapdog/actions/workflows/ci.yml)
[![Release](https://github.com/metaneutrons/snapdog/actions/workflows/release.yml/badge.svg)](https://github.com/metaneutrons/snapdog/actions/workflows/release.yml)
[![GitHub Release](https://img.shields.io/github/v/release/metaneutrons/snapdog)](https://github.com/metaneutrons/snapdog/releases/latest)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)
[![Docker](https://img.shields.io/badge/ghcr.io-snapdog-blue?logo=docker)](https://ghcr.io/metaneutrons/snapdog)

</div>

---

SnapDog turns a Linux box (or Mac) into a synchronized multi-room audio system with deep smart home integration. It embeds a [Snapcast](https://github.com/badaix/snapcast) server, runs AirPlay receivers per zone, streams from Navidrome/Subsonic, plays internet radio — and bridges everything to MQTT and KNX.

## Features

| | |
|---|---|
| 🎵 **AirPlay 1 + 2** | Per-zone receivers, stream from iPhone/Mac |
| 🎧 **Spotify Connect** | Per-zone receivers via librespot |
| 🔊 **Snapcast** | Synchronized playback, embedded server or external process |
| 📚 **Subsonic/Navidrome** | Personal music library with playlist navigation and seek |
| 📻 **Internet Radio** | Station list with live ICY metadata |
| 🏠 **MQTT** | Bidirectional smart home integration |
| 🏢 **KNX** | Building automation (tunnel + router, typed DPT encoding) |
| 🎛️ **Parametric EQ** | Per-zone and per-client, real-time via custom protocol |
| 🌐 **REST API** | ~90 endpoints, full zone/client/media control |
| 📡 **WebSocket** | Real-time state push notifications |
| 🖥️ **WebUI** | Responsive SPA with drag-and-drop client management |
| 🎨 **Cover Art** | Content-addressed caching, unified per-zone endpoint |

## Quick Start

### Docker

```bash
docker run -d --name snapdog \
  -v ./snapdog.toml:/etc/snapdog/snapdog.toml \
  -p 5555:5555 -p 1704:1704 \
  ghcr.io/metaneutrons/snapdog:latest
```

### Binary

Download from [Releases](https://github.com/metaneutrons/snapdog/releases/latest), then:

```bash
snapdog --config snapdog.toml
```

### From Source

```bash
cargo build --release
./target/release/snapdog --config snapdog.toml
```

**Access:**

| | |
|---|---|
| WebUI | http://localhost:5555 |
| API | http://localhost:5555/api/v1/zones |
| Health | http://localhost:5555/health |
| WebSocket | ws://localhost:5555/ws |

## Configuration

Single file: [`snapdog.example.toml`](snapdog.example.toml)

```toml
[http]
port = 5555

[audio]
sample_rate = 48000
bit_depth = 16
channels = 2

[subsonic]
url = "https://music.example.com"
username = "user"
password = "pass"

[knx]
enabled = true
url = "udp://192.168.1.50:3671"

[[zone]]
name = "Living Room"

[[client]]
name = "Kitchen Speaker"
mac = "02:42:ac:11:00:10"
zone = "Living Room"

[[radio]]
name = "Deutschlandfunk"
url = "https://st01.sslstream.dlf.de/dlf/01/high/aac/stream.aac"
```

Snapcast sink paths, stream names, and AirPlay names are auto-generated from zone/client definitions. KNX addresses are explicit (fits into existing installations).

<details>
<summary><strong>API Authentication</strong></summary>

If `api_keys` is set in `[http]`, all `/api/v1/*` and `/ws` endpoints require authentication:
- REST: `Authorization: Bearer <key>` header
- WebSocket: `ws://host:port/ws?token=<key>` query parameter
- Health endpoints and the WebUI are always accessible

</details>

<details>
<summary><strong>Subsonic Server Notes</strong></summary>

When using Navidrome, ensure transcoding is configured for the format specified in `format`. Without transcoding, files in non-streamable containers (ALAC/AAC in MP4) will be downloaded fully before playback, causing significant latency.

</details>

## Ecosystem

SnapDog builds on a family of Rust crates:

| Crate | Description |
|-------|-------------|
| [snapcast-server](https://github.com/metaneutrons/snapcast-rs) | Embeddable Snapcast server with per-stream codecs, custom protocol, encryption |
| [shairplay-rust](https://github.com/metaneutrons/shairplay-rust) | AirPlay 1 + 2 receiver library (RAOP/AirTunes) |
| [knxkit](https://github.com/metaneutrons/knxkit) | KNX/IP tunnel + router with typed DPT encoding |

### snapdog-client

A specialized Snapcast client that understands SnapDog's custom protocol extensions:

- **F32+LZ4 codec** — lossless 32-bit float audio with LZ4 compression (not supported by stock snapclients)
- **Per-client parametric EQ** — receives EQ curves via custom protocol, applies biquad filters before output
- **Encryption** — PSK-based chunk encryption matching the embedded server

Available as binary and Docker image (`ghcr.io/metaneutrons/snapdog-client`).

## Architecture

``` plain
┌─────────────────────────────────────────────────────┐
│                     SnapDog                         │
│                                                     │
│  ┌───────────┐  ┌───────────┐  ┌───────────┐        │
│  │ ZonePlayer│  │ ZonePlayer│  │ ZonePlayer│  ...   │
│  │ (tokio)   │  │ (tokio)   │  │ (tokio)   │        │
│  └────┬──────┘  └────┬──────┘  └────┬──────┘        │
│       │              │              │               │
│  ┌────┴──────────────┴──────────────┴─────┐         │
│  │        Embedded Snapcast Server        │         │
│  │      (per-zone streams + encoders)     │         │
│  └───────────────────┬────────────────────┘         │
│                      │                              │
│  ┌─────────┐  ┌──────┴────┐  ┌──────────┐           │
│  │ AirPlay │  │  REST API │  │   MQTT   │           │
│  │receivers│  │  + WebUI  │  │  bridge  │           │
│  └─────────┘  └───────────┘  └──────────┘           │
│                                    ┌──────────┐     │
│                                    │   KNX    │     │
│                                    │  bridge  │     │
│                                    └──────────┘     │
└─────────────────────────────────────────────────────┘
         │                   
    ┌────┴──────┐        
    │snapclients│        
    │(per room) │        
    └───────────┘        
```

- **ZonePlayer** — per-zone tokio task, owns audio pipeline (decode → resample → encode → Snapcast)
- **Dual Snapcast backend** — embedded server (default) or external process via JSON-RPC
- **Volume via Snapcast** — never PCM amplitude scaling, full dynamic range preserved
- **MAC-based client matching** — clients auto-assigned to zones from config

### Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `snapcast-embedded` | ✅ | In-process Snapcast server ([snapcast-server](https://crates.io/crates/snapcast-server)) |
| `snapcast-process` | — | External snapserver binary + JSON-RPC |
| `ap2` | — | AirPlay 2 (encrypted transport, HAP pairing) |
| `spotify` | ✅ | Spotify Connect receiver ([librespot](https://github.com/librespot-org/librespot)) |

See [Architecture Decision Records](docs/architecture/decisions.md) for design rationale.

## Development

```bash
make setup                                    # Git hooks
docker compose -f docker-compose.dev.yml up -d  # Dev infrastructure
cargo run -- --config snapdog.dev.toml        # Run
cargo test                                    # Test
cargo clippy -- -D warnings                   # Lint
```

<details>
<summary><strong>Dev Infrastructure (Docker Compose)</strong></summary>

| Service | Purpose |
|---------|---------|
| 3× snapclient | Simulated rooms (Living Room, Kitchen, Bedroom) |
| mqtt | Mosquitto MQTT broker |
| knxd | KNX gateway simulator |
| knx-monitor | Visual KNX bus debugging |
| navidrome | Subsonic-compatible music server |

</details>

## License

[GPL-3.0-only](LICENSE)
