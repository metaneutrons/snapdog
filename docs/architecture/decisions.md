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

---

## ADR-011: ZonePlayer — Per-Zone Audio Pipeline with Command Channel

**Decision:** Each zone runs an independent ZonePlayer task that owns its audio pipeline,
AirPlay receiver, and Snapcast TCP source connection.

**Architecture:**
```
ZonePlayer (one per zone, runs as tokio task)
├── Command Channel: mpsc::Receiver<ZoneCommand>
├── State: Idle | Playing(Source) | AirPlayActive
├── AirPlay Receiver (own libshairplay instance, own mDNS name)
├── Decode Task (abortable): Radio/Subsonic/URL → PCM
├── PCM Channel: Decode or AirPlay → TCP Writer
├── TCP Writer: PCM → Snapcast Source (connection stays open)
└── Volume: controlled via Snapcast Group (never PCM amplitude)
```

**Complete command set:**
```rust
enum ZoneCommand {
    // Source selection
    PlayRadio(usize),                        // Radio station index from config
    PlaySubsonicPlaylist(String, usize),     // Playlist ID, start track index
    PlaySubsonicTrack(String),               // Single track ID
    PlayUrl(String),                         // Arbitrary HTTP stream URL
    SetTrack(usize),                         // Jump to track N in current playlist

    // Transport
    Play,                                    // Resume or restart current source
    Pause,
    Stop,
    Next,                                    // Next track (playlist) or next station (radio)
    Previous,                                // Previous track/station

    // Playlist navigation
    NextPlaylist,
    PreviousPlaylist,
    SetPlaylist(usize),                      // Switch to playlist by index

    // Seek
    Seek(i64),                               // Absolute position in ms
    SeekProgress(f64),                        // Relative 0.0..1.0

    // Zone settings
    SetVolume(i32),                          // → Snapcast Group Volume
    SetMute(bool),
    ToggleMute,
    SetShuffle(bool),
    ToggleShuffle,
    SetRepeat(bool),                         // Playlist repeat
    ToggleRepeat,
    SetTrackRepeat(bool),                    // Single track repeat
    ToggleTrackRepeat,
}
```

**Volume routing decision:** Volume/Mute commands go through the ZonePlayer even though
they only call Snapcast Group API. Rationale: single entry point for all zone operations,
consistent state updates, and future-proof (e.g. volume fade effects).

**Source types and behavior:**

| Source | Pause | Seek | Next/Previous | Auto-advance | Metadata source |
|--------|-------|------|---------------|-------------|-----------------|
| Radio | No (Stop+Restart) | No | Next/prev station | No | ICY-metadata + config |
| Subsonic Playlist | Yes | Yes | Next/prev track | Yes | Subsonic API (before play) |
| Subsonic Track | Yes | Yes | No | No | Subsonic API (before play) |
| URL | No | No | No | No | ICY-metadata if available |
| AirPlay | External | External | External | External | DMAP callbacks |

**Metadata architecture:**
- Subsonic: full metadata (title, artist, album, cover URL, duration) set in state
  *before* decode starts. Position updated from symphonia packet timestamps during playback.
- Radio: station name from config set initially. ICY-metadata parsed from HTTP stream
  updates title live (e.g. "Artist - Song"). Cover URL optional from config.
- AirPlay: `audio_set_metadata` callback delivers DMAP (title/artist/album),
  `audio_set_coverart` delivers JPEG/PNG bytes, `audio_set_progress` delivers position.
- All metadata written to shared state store. API/MQTT/WebSocket read from there.

**Track completion and auto-advance:**
- Decode task ends → PCM channel closes → `pcm_rx.recv()` returns `None`
- ZonePlayer detects this and applies repeat/advance logic:
  - TrackRepeat=true → restart same track
  - More tracks in playlist → start next track
  - Last track + PlaylistRepeat=true → restart from track 1
  - Last track + PlaylistRepeat=false → zone goes Idle
  - Shuffle=true → random next track (no immediate repeats)

**Preemption rules:**
- AirPlay preempts everything — stops current decode, takes over PCM channel
- When AirPlay session ends, zone returns to Idle (previous source NOT resumed)
- Play(new source) stops current source, starts new one
- Second AirPlay connection to same zone: libshairplay handles this (max_clients=1,
  new connection replaces old)

**AirPlay naming convention:**
- Default: `{airplay.name} {zone.name}` → e.g. "SnapDog Ground Floor"
- Override: `zone.airplay_name` in config

**Error handling:**
- Stream connection failure → log error, zone goes to Idle, state updated
- Decode error mid-stream → skip packet, continue (radio) or advance track (playlist)
- Snapcast TCP write failure → reconnect TCP, resume writing

**Volume architecture:**
- Zone volume = Snapcast Group volume (digital mixing in client, full dynamic range)
- Client volume = Snapcast Client volume (per-speaker within a zone)
- PCM stream is always full-scale — never attenuated in the pipeline

**AirPlay disconnect detection:**
- `audio_destroy` callback is called reliably on disconnect (TEARDOWN, connection drop,
  or cleanup). This is the ZonePlayer's signal to transition from AirPlayActive → Idle.

**AirPlay metadata and cover art (TODO — not yet wired):**
- `audio_set_metadata`: DMAP-encoded metadata (title, artist, album) — must be parsed
- `audio_set_coverart`: JPEG/PNG cover art bytes — store or serve via API
- `audio_set_progress`: start/current/end timestamps — map to position/duration
- `audio_remote_control_id`: DACP ID for remote control — not needed for MVP
- Currently all four callbacks are set to `None` — must be implemented

**AirPlay resampling:**
- libshairplay delivers PCM at 44100 Hz / 16-bit (Apple standard)
- Snapcast TCP source expects the configured sample rate (default 48000 Hz)
- If input rate ≠ output rate, resample before writing to TCP source
- Resampling also needed for any other source that doesn't match the target format

**Extensibility:**
- The PCM channel between decoder and TCP writer serves as the insertion point for
  future audio processing (EQ, crossfade, normalization). No architectural changes needed.

---

## ADR-012: Unified Cover Art via API Proxy

**Decision:** All cover art is served through a single API endpoint per zone.
No external URLs are exposed to clients.

**Endpoint:**
```
GET /api/v1/zones/{id}/cover → image bytes with correct Content-Type
                             → 204 No Content if no cover available
```

**Cover cache (in-memory, not persisted):**
```rust
struct CoverEntry {
    bytes: Vec<u8>,
    mime: String,   // From source, not guessed
}
// HashMap<zone_index, CoverEntry>
```

**MIME-Type sourcing:**
- Subsonic: `Content-Type` header from `getCoverArt` HTTP response
- Radio: `Content-Type` header from cover URL fetch
- AirPlay: Magic bytes fallback (FF D8 FF → image/jpeg, 89 50 4E 47 → image/png)
- Unknown: `image/octet-stream` (browser handles it)

**Supports all formats:** JPEG, PNG, WebP, AVIF, SVG — whatever the source delivers.
No conversion, just proxying.

**Why not expose external URLs directly:**
- AirPlay has no URL, only bytes
- Subsonic URLs contain auth tokens
- CORS issues with external URLs
- Inconsistent behavior per source

**Cache lifecycle:**
- Source change → ZonePlayer fetches/receives new cover → replaces cache entry
- Zone stops → cache entry cleared
- Not persisted to state.json (transient, 50-500KB per image)

---

## ADR-013: Extended TrackInfo and SourceType

**Decision:** TrackInfo extended with genre, track number, and source type.
Cover art removed from TrackInfo (served via ADR-012 instead).

```rust
pub enum SourceType {
    Radio,
    SubsonicPlaylist,
    SubsonicTrack,
    Url,
    AirPlay,
    Idle,
}

pub struct TrackInfo {
    // Metadata
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u32>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,

    // Playback
    pub duration_ms: i64,
    pub position_ms: i64,
    pub source: SourceType,

    // Technical
    pub bitrate_kbps: Option<u32>,       // 128, 320, etc.
    pub content_type: Option<String>,    // "audio/flac", "audio/aac"
    pub sample_rate: Option<u32>,        // 44100, 48000
}
```

**Rationale:** `source` tells the WebUI what controls to show (seek for Subsonic,
next station for Radio, nothing for AirPlay). Cover art is decoupled because it's
binary data with its own caching and serving strategy (ADR-012).
