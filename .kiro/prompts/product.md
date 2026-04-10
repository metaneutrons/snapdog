# Product: SnapDog

## Vision
SnapDog is an enterprise-grade, open-source multi-zone audio controller for smart homes.
It replaces the fragile process-orchestration approach of its .NET predecessor (SnapDog2)
with a single Rust binary that tightly integrates AirPlay reception, audio decoding,
Snapcast feeding, and smart home protocol bridges (MQTT, KNX).

## Predecessor
- Repository: `~/Source/snapdog` (SnapDog2, .NET 9.0)
- The .NET version's architecture (loose coupling of Snapcast, shairplay, VLC as separate
  processes) proved fragile. This Rust rewrite addresses that by internalizing the audio
  pipeline while keeping Snapcast as a managed child process.

## Core Objectives
- **Single Binary**: One Rust binary manages AirPlay, audio decoding, Snapcast lifecycle,
  REST API, MQTT, KNX, and embedded WebUI — no external process orchestration except snapserver
- **Single Config File**: One `snapdog.toml` — everything else (snapserver.conf, KNX
  addresses, sink paths) is derived automatically
- **24/7 Reliability**: Designed for continuous operation in a home environment
- **Smart Home Integration**: Bidirectional MQTT + KNX for dashboards, scenes, physical switches
- **Multi-Zone Audio**: Independent or synchronized playback across zones via Snapcast
- **Embedded WebUI**: Apple-style, mobile-first control interface served from the binary itself

## Target Use Cases
- Multi-room synchronized audio (party mode) or independent per-zone streams
- AirPlay 1 + 2 receiver that feeds directly into the multi-room system
- Spotify Connect receiver (planned, via librespot)
- Control via embedded WebUI, REST API, MQTT, KNX, or WebSocket
- Subsonic/Navidrome integration for personal music library + internet radio

## Unified Playlist Model
Radio stations and Subsonic playlists are unified into a single numeric index system:
- Index 0 = Radio (stations from config)
- Index 1+ = Subsonic playlists (or other providers, order configurable)
- `SetPlaylist(index, start_track)` — single atomic command for all sources
- Cover art: deterministic endpoint per playlist/track index

## WebUI Scope
The embedded WebUI is the primary user-facing control surface:
- **Zone control**: play/pause/stop, next/previous, volume, mute, shuffle, repeat
- **Source browsing**: unified playlist browser (radio + Subsonic), arbitrary URLs
- **Now Playing**: cover art (deterministic, no cache), track metadata (3-line marquee),
  seek bar (disabled with elapsed counter for radio), progress
- **Client management**: see connected clients per zone, drag-and-drop between zones
- **Real-time updates**: WebSocket-driven state sync (no polling)
- **Responsive layout**:
  - Desktop: all zones side-by-side in flex-wrap grid (no sidebar)
  - Tablet: sidebar + single zone with horizontal card layout
  - Mobile: tab bar + full-width zone card
- **System theme**: automatic dark/light mode via `prefers-color-scheme`

## Future Paths (ADR-documented)
- **Spotify Connect** (ADR-015): passive receiver via librespot, optional active client
- **Configurable provider ordering** (ADR-016): user controls playlist index assignment
- **Per-zone DSP** (ADR-017): built-in parametric EQ + compressor/limiter, pure Rust

## Author & License
- Author: Fabian Schmieder
- License: GPL-3.0-only
