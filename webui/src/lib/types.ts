// ── Enums ─────────────────────────────────────────────────────

export type PlaybackState = "playing" | "paused" | "stopped";

export type SourceType =
  | "idle"
  | "radio"
  | "subsonic_playlist"
  | "subsonic_track"
  | "url"
  | "airplay";

// ── Zone ──────────────────────────────────────────────────────

export interface ZoneInfo {
  index: number;
  name: string;
  icon: string;
  volume: number;
  muted: boolean;
  playback: PlaybackState;
  source: SourceType;
  shuffle: boolean;
  repeat: boolean;
  track_repeat: boolean;
  presence: boolean;
  presence_enabled: boolean;
  presence_timer_active: boolean;
}

export interface TrackMetadata {
  title: string;
  artist: string;
  album: string;
  album_artist: string | null;
  genre: string | null;
  year: number | null;
  track_number: number | null;
  disc_number: number | null;
  duration_ms: number;
  position_ms: number;
  seekable: boolean;
  bitrate_kbps: number | null;
  content_type: string | null;
  sample_rate: number | null;
  source: string;
  cover_url: string | null;
  playlist_index: number | null;
  playlist_track_index: number | null;
  playlist_track_count: number | null;
}

export interface PlaylistState {
  index: number | null;
  name: string | null;
  track_index: number | null;
  track_count: number | null;
}

// ── Client ────────────────────────────────────────────────────

export interface ClientInfo {
  index: number;
  name: string;
  mac: string;
  zone_index: number;
  icon: string;
  volume: number;
  max_volume: number;
  muted: boolean;
  connected: boolean;
  is_snapdog: boolean;
}

// ── Media (Subsonic) ──────────────────────────────────────────

export interface PlaylistInfo {
  id: number;
  name: string;
  song_count: number;
  duration: number;
  cover_art: string | null;
}

export interface TrackInfo {
  id: string;
  title: string;
  artist: string;
  album: string;
  duration: number;
  track: number;
}

// ── System ────────────────────────────────────────────────────

export interface SystemStatus {
  version: string;
  zones: number;
  clients: number;
  radios: number;
}

export interface VersionInfo {
  version: string;
  rust_version: string;
}

export interface HealthResponse {
  status: string;
  zones: number;
  clients: number;
}

// ── WebSocket ─────────────────────────────────────────────────

export interface WsZoneStateChanged {
  type: "zone_state_changed";
  zone: number;
  playback: PlaybackState;
  volume: number;
  muted: boolean;
  source: SourceType;
  shuffle: boolean;
  repeat: boolean;
  track_repeat: boolean;
}

export interface WsZoneTrackChanged {
  type: "zone_track_changed";
  zone: number;
  title: string;
  artist: string;
  album: string;
  duration_ms: number;
  position_ms: number;
  seekable: boolean;
  cover_url: string | null;
}

export interface WsZoneProgress {
  type: "zone_progress";
  zone: number;
  position_ms: number;
  duration_ms: number;
}

export interface WsClientStateChanged {
  type: "client_state_changed";
  client: number;
  volume: number;
  muted: boolean;
  connected: boolean;
  zone: number;
}

export interface WsZonePresenceChanged {
  type: "zone_presence_changed";
  zone: number;
  presence: boolean;
  enabled: boolean;
  timer_active: boolean;
}

export type WsNotification =
  | WsZoneStateChanged
  | WsZoneTrackChanged
  | WsZoneProgress
  | WsClientStateChanged
  | WsZonePresenceChanged;

export interface WsCommand {
  zone: number;
  action: string;
  value?: string | number | boolean;
}

// ── Volume ────────────────────────────────────────────────────

export type VolumeValue = number | string; // absolute (75) or relative ("+5")
