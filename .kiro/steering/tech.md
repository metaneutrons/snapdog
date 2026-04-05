# Technology & Conventions

## Design Principles

### SSOT — Single Source of Truth
- `snapdog.toml` is the only user-facing configuration file
- All derived config (snapserver.conf, KNX addresses, sink paths) is generated at startup
- Application state lives in one place: in-memory, persisted to a single JSON file

### DRY — Don't Repeat Yourself
- No copy-paste configuration. KNX addresses are auto-generated from zone/client indices
- Shared types and traits — never duplicate a struct definition
- If a pattern appears twice, extract it

### Convention over Configuration
- Zone 1 → sink `/snapsinks/zone1`, KNX addresses `1/x/y`, stream name `Zone1`
- Client 1 → KNX addresses `3/1/x`
- Overrides only for what deviates from convention

### Fail Fast, Recover Gracefully
- Validate config completely at startup — don't discover errors at runtime
- Use `Result<T>` everywhere — no panics in library code, no `.unwrap()` outside tests
- Reconnect automatically to external services (Snapcast, MQTT, KNX)

### Minimal Surface Area
- One binary, one config file, one state file
- No Redis, no database, no message queue
- External dependencies only when they provide clear value over a simple implementation

## Tech Stack

### Core
- **Rust** (latest stable, edition 2024)
- **Tokio** — async runtime
- **axum** — REST API + WebSocket
- **serde** + **toml** — configuration
- **tracing** — structured logging (console + file, daily rotation)

### Audio
- **symphonia** — pure Rust decoding (AAC, MP3, FLAC, ALAC)
- **shairplay** — pure Rust AirPlay 1 + 2 receiver (crates.io, `ap2` feature)
- **rubato** — sample rate conversion (resampling)
- **Snapcast** — multi-room streaming (external binary, managed as child process)
- **snapcast-control** — JSON-RPC control API client

### Smart Home
- **rumqttc** — MQTT client (async, bidirectional)
- **knxkit** — KNX/IP tunneling and routing

### HTTP
- **reqwest** — Subsonic API client, radio stream fetching, cover art proxy

### Error Handling
- **thiserror** — typed errors for library-style modules
- **anyhow** — top-level error propagation in main/CLI

### WebUI (embedded static SPA)
- **Next.js** (latest stable) — App Router, `output: 'export'` (static SPA)
- **React 19** — transitions, optimistic updates
- **Tailwind CSS v4** — CSS-first configuration
- **shadcn/ui** — accessible component primitives (Radix UI)
- **Framer Motion** — swipe gestures, page transitions, spring animations
- **Zustand** — lightweight reactive state management
- **rust-embed** — embeds `webui/out/` into the Rust binary at compile time

## Code Conventions

### File Header
Every `.rs` file starts with:
```rust
// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2025 Fabian Schmieder
```

### Logging
Use `tracing` with structured fields. Prefer `#[tracing::instrument]` on async functions:
```rust
#[tracing::instrument(skip(self), fields(zone = %zone_id))]
async fn set_zone_volume(&self, zone_id: u32, volume: u8) -> Result<()> {
    tracing::info!(volume, "Setting zone volume");
    // ...
}
```

Log levels:
- `error` — unrecoverable failures, service down
- `warn` — recoverable issues, retries, degraded operation
- `info` — significant state changes (zone playing, client connected, config loaded)
- `debug` — detailed operational flow
- `trace` — protocol-level detail (JSON-RPC messages, PCM buffer sizes)

### Error Handling
- Define domain errors with `thiserror` per module
- Return `Result<T>` from all fallible functions
- No `.unwrap()` or `.expect()` outside of tests
- No `panic!()` in library code

### Naming
- Modules: `snake_case` (Rust convention)
- Types: `PascalCase`
- Functions/methods: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- Crate name: `snapdog` (lowercase)

### Formatting & Linting
- `rustfmt` with project config (see `rustfmt.toml`)
- `clippy` with strict settings (see `.clippy.toml`)
- CI enforces both — no merge without clean fmt + clippy

### Testing
- Unit tests: `#[cfg(test)] mod tests` in the same file
- Integration tests: `tests/` directory
- Mock external services with `mockall` (traits) and `wiremock` (HTTP)
- `#[tokio::test]` for async tests
- No tests for external binaries (snapserver) — test the Rust wrapper

### Dependencies
- Prefer pure Rust crates — no FFI bindings
- Pin workspace dependency versions in `Cargo.toml`
- Minimize dependency count — every crate must justify its existence

### Git Workflow

**Conventional Commits** — every commit message follows the format:
```
<type>(<scope>): <description>

[optional body]
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `ci`, `chore`, `perf`, `style`
Scopes: `config`, `audio`, `airplay`, `snapcast`, `api`, `mqtt`, `knx`, `subsonic`, `state`, `process`, `webui`

Examples:
```
feat(config): add TOML parsing with convention-over-config
fix(snapcast): handle reconnect on connection loss
refactor(audio): extract PCM pipeline into separate module
feat(webui): add responsive zone grid layout
docs: update architecture decisions
ci: add clippy to pre-push hook
```

**Git Hooks (enforced):**
- **pre-commit**: `cargo fmt -- --check` — no commit without clean formatting
- **pre-push**: `cargo clippy -- -D warnings` — no push without clean lints
- Hooks live in `.githooks/`, activated via `make setup` (runs `git config core.hooksPath .githooks`)

## Development Workflow
```bash
cargo build                    # Build
cargo test                     # Run all tests
cargo clippy -- -D warnings    # Lint (deny all warnings)
cargo fmt -- --check           # Format check
docker compose -f docker-compose.dev.yml up  # Start test rig
```

**IMPORTANT: Never push to remote without explicit user approval.** Commits are local only until the user says "push".
