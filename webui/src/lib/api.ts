import type {
  ZoneInfo,
  TrackMetadata,
  ClientInfo,
  PlaylistInfo,
  PlaylistState,
  TrackInfo,
  SystemStatus,
  VersionInfo,
  HealthResponse,
  VolumeValue,
} from "./types";

const BASE = "";

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

async function get<T>(path: string): Promise<T> {
  const res = await fetch(`${BASE}${path}`);
  if (!res.ok) throw new ApiError(res.status, `GET ${path}: ${res.status}`);
  return res.json();
}

async function put<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new ApiError(res.status, `PUT ${path}: ${res.status}`);
  const text = await res.text();
  return text ? JSON.parse(text) : (undefined as T);
}

async function post<T = void>(path: string, body?: unknown): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    method: "POST",
    headers: body !== undefined ? { "Content-Type": "application/json" } : {},
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  if (!res.ok) throw new ApiError(res.status, `POST ${path}: ${res.status}`);
  const text = await res.text();
  return text ? JSON.parse(text) : (undefined as T);
}

// ── Zones ─────────────────────────────────────────────────────

const Z = "/api/v1/zones";

export const zones = {
  list: () => get<ZoneInfo[]>(Z),
  get: (id: number) => get<ZoneInfo>(`${Z}/${id}`),
  count: () => get<number>(`${Z}/count`),

  // Volume
  getVolume: (id: number) => get<number>(`${Z}/${id}/volume`),
  setVolume: (id: number, v: VolumeValue) => put<number>(`${Z}/${id}/volume`, v),

  // Mute
  getMute: (id: number) => get<boolean>(`${Z}/${id}/mute`),
  setMute: (id: number, v: boolean) => put<void>(`${Z}/${id}/mute`, v),
  toggleMute: (id: number) => post(`${Z}/${id}/mute/toggle`),

  // Transport
  play: (id: number) => post(`${Z}/${id}/play`),
  pause: (id: number) => post(`${Z}/${id}/pause`),
  stop: (id: number) => post(`${Z}/${id}/stop`),
  next: (id: number) => post(`${Z}/${id}/next`),
  previous: (id: number) => post(`${Z}/${id}/previous`),

  // Shuffle
  getShuffle: (id: number) => get<boolean>(`${Z}/${id}/shuffle`),
  setShuffle: (id: number, v: boolean) => put<void>(`${Z}/${id}/shuffle`, v),
  toggleShuffle: (id: number) => post(`${Z}/${id}/shuffle/toggle`),

  // Repeat (playlist)
  getRepeat: (id: number) => get<boolean>(`${Z}/${id}/repeat`),
  setRepeat: (id: number, v: boolean) => put<void>(`${Z}/${id}/repeat`, v),
  toggleRepeat: (id: number) => post(`${Z}/${id}/repeat/toggle`),

  // Track repeat
  getTrackRepeat: (id: number) => get<boolean>(`${Z}/${id}/track/repeat`),
  setTrackRepeat: (id: number, v: boolean) => put<void>(`${Z}/${id}/track/repeat`, v),
  toggleTrackRepeat: (id: number) => post(`${Z}/${id}/track/repeat/toggle`),

  // Track info
  getTrackMetadata: (id: number) => get<TrackMetadata>(`${Z}/${id}/track/metadata`),
  getTrackPosition: (id: number) => get<number>(`${Z}/${id}/track/position`),
  seekPosition: (id: number, ms: number) => put<void>(`${Z}/${id}/track/position`, ms),
  getTrackProgress: (id: number) => get<number>(`${Z}/${id}/track/progress`),
  seekProgress: (id: number, v: number) => put<void>(`${Z}/${id}/track/progress`, v),

  // Play specific content
  playTrack: (id: number, track: number) => post(`${Z}/${id}/play/track`, track),
  playUrl: (id: number, url: string) => post(`${Z}/${id}/play/url`, url),
  playRadio: (id: number, station: number) => post(`${Z}/${id}/play/radio/${station}`),
  playPlaylist: (id: number, playlistId: string, track?: number) =>
    post(`${Z}/${id}/play/playlist`, { id: playlistId, track: track ?? 0 }),
  playPlaylistTrack: (zoneId: number, playlistId: number, track: number) =>
    post(`${Z}/${zoneId}/play/playlist/${playlistId}/track`, track),

  // Playlist navigation
  getPlaylist: (id: number) => get<number>(`${Z}/${id}/playlist`),
  setPlaylist: (id: number, v: number) => put<void>(`${Z}/${id}/playlist`, v),
  nextPlaylist: (id: number) => post(`${Z}/${id}/next/playlist`),
  previousPlaylist: (id: number) => post(`${Z}/${id}/previous/playlist`),
  getPlaylistInfo: (id: number) => get<PlaylistState>(`${Z}/${id}/playlist/info`),

  // Zone info
  getName: (id: number) => get<string>(`${Z}/${id}/name`),
  getIcon: (id: number) => get<string>(`${Z}/${id}/icon`),
  getPlayback: (id: number) => get<string>(`${Z}/${id}/playback`),
  getClients: (id: number) => get<number[]>(`${Z}/${id}/clients`),

  // Cover art URL (not a fetch — returns the URL string for <img>)
  coverUrl: (id: number) => `${BASE}${Z}/${id}/cover`,
};

// ── Clients ───────────────────────────────────────────────────

const C = "/api/v1/clients";

export const clients = {
  list: () => get<ClientInfo[]>(C),
  get: (id: number) => get<ClientInfo>(`${C}/${id}`),
  count: () => get<number>(`${C}/count`),

  getVolume: (id: number) => get<number>(`${C}/${id}/volume`),
  setVolume: (id: number, v: VolumeValue) => put<number>(`${C}/${id}/volume`, v),

  getMute: (id: number) => get<boolean>(`${C}/${id}/mute`),
  setMute: (id: number, v: boolean) => put<void>(`${C}/${id}/mute`, v),
  toggleMute: (id: number) => post(`${C}/${id}/mute/toggle`),

  getLatency: (id: number) => get<number>(`${C}/${id}/latency`),
  setLatency: (id: number, v: number) => put<void>(`${C}/${id}/latency`, v),

  getZone: (id: number) => get<number>(`${C}/${id}/zone`),
  setZone: (id: number, zoneId: number) => put<void>(`${C}/${id}/zone`, zoneId),

  getName: (id: number) => get<string>(`${C}/${id}/name`),
  setName: (id: number, name: string) => put<void>(`${C}/${id}/name`, name),

  getIcon: (id: number) => get<string>(`${C}/${id}/icon`),
  getConnected: (id: number) => get<boolean>(`${C}/${id}/connected`),
};

// ── Media ─────────────────────────────────────────────────────

const M = "/api/v1/media";

export const media = {
  playlists: () => get<PlaylistInfo[]>(`${M}/playlists`),
  playlist: (id: string) => get<{ id: string; name: string; tracks: number }>(`${M}/playlists/${id}`),
  tracks: (playlistId: string) => get<TrackInfo[]>(`${M}/playlists/${playlistId}/tracks`),
  track: (playlistId: string, idx: number) => get<TrackInfo>(`${M}/playlists/${playlistId}/tracks/${idx}`),
};

// ── System ────────────────────────────────────────────────────

export const system = {
  status: () => get<SystemStatus>("/api/v1/system/status"),
  version: () => get<VersionInfo>("/api/v1/system/version"),
};

// ── Health ────────────────────────────────────────────────────

export const health = {
  check: () => get<HealthResponse>("/health"),
  ready: () => get<string>("/health/ready"),
  live: () => get<string>("/health/live"),
};

export const api = { zones, clients, media, system, health };
