# Architecture Decision Record

## Context

SnapDog2 (.NET 9.0) orchestrates multiple external processes (Snapcast server, shairplay,
LibVLC) via network protocols and pipes. This loose coupling is fragile — each process
boundary is a failure point, and configuration must be kept in sync across all components.

This document records the key architecture decisions for the Rust rewrite.

---

## ADR-001: Single Rust Binary with Managed Snapserver

**Decision:** One Rust binary handles all logic (AirPlay, audio decoding, API, MQTT, KNX).
Snapserver runs as a managed child process, started and supervised by the Rust binary.

**Rationale:**
- Eliminates process orchestration fragility
- Snapserver has no library API (C++ standalone binary) — FFI is not feasible
- The Rust binary generates `snapserver.conf` from `snapdog.toml`, ensuring consistency
- Snapserver listens on loopback only (127.0.0.1) — only the Rust binary can talk to it

**Consequences:**
- Docker image must include both `snapdog` binary and `snapserver`
- Snapserver updates are decoupled (apt upgrade)

---

## ADR-002: libshairplay via Vendored C Source + FFI

**Decision:** AirPlay 1 (RAOP) support via `juhovh/shairplay` as a git submodule,
compiled from source using the `cc` crate in `build.rs`.

**Rationale:**
- No pure Rust RAOP implementation exists
- libshairplay has a tiny, stable C API (~15 functions)
- Callback-based design delivers raw PCM + metadata — perfect for our pipeline
- AirPlay 1 is sufficient (no need for AirPlay 2's proprietary HAP pairing)
- Vendoring ensures reproducible builds without system-level dependencies

**API surface:**
- `raop_init` / `raop_start` / `raop_stop` / `raop_destroy`
- `dnssd_init` / `dnssd_register_raop` / `dnssd_destroy` (mDNS included)
- Callbacks: `audio_init`, `audio_process`, `audio_destroy`, `audio_flush`,
  `audio_set_volume`, `audio_set_metadata`, `audio_set_coverart`, `audio_set_progress`

---

## ADR-003: Symphonia for Audio Decoding (No LibVLC)

**Decision:** Use `symphonia` (pure Rust) for all audio decoding. LibVLC is eliminated.

**Rationale:**
- Supports all required codecs: AAC, MP3, FLAC, ALAC
- Pure Rust — no FFI, no external process, no system dependency
- Gapless playback for MP3, FLAC, ALAC
- LibVLC was the primary source of fragility in SnapDog2

---

## ADR-004: TOML Configuration with Convention over Configuration

**Decision:** Single `snapdog.toml` file. KNX addresses, sink paths, and snapserver
config are auto-generated from zone/client definitions.

**Rationale:**
- SnapDog2's `.env` file was ~500 lines, mostly repetitive KNX addresses following
  a clear pattern (zone N → `N/x/y`, client N → `3/N/x`)
- Auto-generation reduces config from ~500 lines to ~70 lines
- Overrides are possible for non-standard setups
- TOML is the Rust ecosystem standard for configuration

**Convention rules:**
- Zone N: sink = `/snapsinks/zoneN`, stream = `ZoneN`, KNX = `N/{1-4}/x`
- Client N: KNX = `3/N/x`
- Snapserver TCP source ports: `4952 + zone_index`

---

## ADR-005: JSON File for State Persistence (No Redis)

**Decision:** Persist application state to a single JSON file instead of Redis.

**Rationale:**
- SnapDog2 stored ~6 keys in Redis (2 zone states, 3 client states, 1 config fingerprint)
- A full Redis server for 6 keys is massive overhead (container, connection management,
  resilience policies, health checks)
- Single-writer model (only the Rust binary writes) — no concurrency concerns
- Atomic write (write to temp file, then rename) is crash-safe
- Eliminates one container from the deployment

---

## ADR-006: Plain WebSocket Instead of SignalR

**Decision:** Use axum's built-in WebSocket support with JSON messages instead of SignalR.

**Rationale:**
- SignalR is a Microsoft-specific protocol with complex negotiation
- No Rust SignalR implementation exists
- Plain WebSocket with typed JSON messages is simpler, universal, and sufficient
- The WebUI (React) replaces `@microsoft/signalr` with native `WebSocket` — less code

**Message format:**
```json
{"type": "zone.playback_changed", "zone": 1, "state": "playing", "track": {...}}
```

---

## ADR-007: tracing for Logging (No OpenTelemetry at Launch)

**Decision:** Use `tracing` + `tracing-subscriber` for structured logging.
OpenTelemetry deferred to a later phase.

**Rationale:**
- `tracing` is the Rust ecosystem standard, used by tokio/axum/hyper natively
- Structured logging with spans provides context propagation through async calls
- File rotation via `tracing-appender`
- OpenTelemetry Rust SDK is less mature than .NET's, and the overhead of OTEL Collector +
  Grafana is not justified for a home automation system
- `tracing` is forward-compatible: adding `tracing-opentelemetry` later requires zero
  changes to existing instrumentation

---

## ADR-008: Snapcast Control via `snapcast-control` Crate

**Decision:** Use the `snapcast-control` crate for all Snapcast JSON-RPC operations.

**Rationale:**
- Covers 100% of the Snapcast JSON-RPC API (v0.28+)
- Tokio-based async client with auto-reconnect
- Replaces SnapDog2's custom `SnapcastJsonRpcClient` (339 lines of C#)
- Actively maintained (v0.4.0, Jan 2026)

---

## ADR-009: KNX via knxkit Crate

**Decision:** Use `knxkit` for KNX/IP communication. If routing (multicast) support
is missing, extend or supplement with direct UDP implementation.

**Rationale:**
- Supports tunneling (confirmed) and likely routing
- High-level API for group value read/write
- DPT (Datapoint Type) support included
- Fallback: KNX/IP routing is simple UDP multicast — ~200 lines to add if needed

---

## ADR-010: Development Test Rig

**Decision:** Provide a `docker-compose.dev.yml` with simulated Snapcast clients,
MQTT broker, KNX simulator, and Navidrome — but no Redis, Grafana, or OTEL Collector.

**Rationale:**
- SnapDog2's dev environment had 13 containers. The Rust version needs ~6
- Simulated Snapcast clients with fixed MAC addresses for realistic testing
- Mosquitto for MQTT, knxd for KNX simulation, Navidrome for Subsonic API
- No observability stack needed — `tracing` logs to console/file
