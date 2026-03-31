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
  REST API, MQTT, and KNX — no external process orchestration except snapserver
- **Single Config File**: One `snapdog.toml` — everything else (snapserver.conf, KNX
  addresses, sink paths) is derived automatically
- **24/7 Reliability**: Designed for continuous operation in a home environment
- **Smart Home Integration**: Bidirectional MQTT + KNX for dashboards, scenes, physical switches
- **Multi-Zone Audio**: Independent or synchronized playback across zones via Snapcast

## Target Use Cases
- Multi-room synchronized audio (party mode) or independent per-zone streams
- AirPlay receiver that feeds directly into the multi-room system
- Control via REST API, MQTT, KNX, or WebSocket-connected WebUI
- Subsonic/Navidrome integration for personal music library + internet radio

## Author & License
- Author: Fabian Schmieder
- License: GPL-3.0-only
