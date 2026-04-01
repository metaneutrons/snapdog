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
- AirPlay receiver that feeds directly into the multi-room system
- Control via embedded WebUI, REST API, MQTT, KNX, or WebSocket
- Subsonic/Navidrome integration for personal music library + internet radio

## WebUI Scope
The embedded WebUI is the primary user-facing control surface:
- **Zone control**: play/pause/stop, next/previous, volume, mute, shuffle, repeat
- **Source browsing**: Subsonic playlists + tracks, radio stations, arbitrary URLs
- **Now Playing**: cover art, track metadata, seek bar, progress
- **Client management**: see connected clients per zone, move clients between zones
- **Real-time updates**: WebSocket-driven state sync (no polling)
- **Responsive**: swipe-per-zone on mobile, split view on tablet, sidebar on desktop
- **System theme**: automatic dark/light mode via `prefers-color-scheme`

## Author & License
- Author: Fabian Schmieder
- License: GPL-3.0-only
