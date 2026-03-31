# SnapDog

Multi-zone audio controller with AirPlay, Snapcast, MQTT, and KNX integration.

> Enterprise-grade Rust rewrite of [SnapDog2](https://github.com/metaneutrons/snapdog) (.NET).

## What it does

SnapDog is a single binary that turns a Linux box into a multi-room audio system with smart home integration:

- **AirPlay receiver** — stream from iPhone/Mac directly into multi-room audio
- **Snapcast integration** — synchronized playback across rooms
- **Subsonic/Navidrome** — play from your personal music library
- **Internet radio** — configurable station list
- **MQTT** — bidirectional smart home integration
- **KNX** — building automation protocol support
- **REST API + WebSocket** — control from any client

## Quick Start

```bash
cp snapdog.example.toml snapdog.toml
# Edit snapdog.toml to match your setup
cargo build --release
./target/release/snapdog --config snapdog.toml
```

## Configuration

One file: `snapdog.toml`. See [snapdog.example.toml](snapdog.example.toml) for a complete example.

KNX addresses and Snapcast sink paths are auto-generated from zone/client definitions.
Override only what deviates from convention.

## Development

```bash
# Start test rig (Snapcast server + clients, MQTT, KNX simulator, Navidrome)
docker compose -f docker-compose.dev.yml up -d

# Build and run
cargo run -- --config snapdog.example.toml

# Run tests
cargo test

# Lint
cargo clippy -- -D warnings
```

## Architecture

See [docs/architecture/decisions.md](docs/architecture/decisions.md) for design rationale.

## License

GPL-3.0-only. See [LICENSE](LICENSE).
