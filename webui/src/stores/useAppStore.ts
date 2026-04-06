import { create } from "zustand";
import type {
  ZoneInfo,
  TrackMetadata,
  ClientInfo,
} from "@/lib/types";
import { api } from "@/lib/api";

// ── Zone with track metadata merged ───────────────────────────

export interface ZoneState extends ZoneInfo {
  track: TrackMetadata | null;
}

// ── Store shape ───────────────────────────────────────────────

interface AppState {
  zones: Map<number, ZoneState>;
  clients: Map<number, ClientInfo>;
  selectedZone: number;
  isConnected: boolean;
  isLoading: boolean;

  // Init
  loadAll: () => Promise<void>;

  // Zone updates
  setZones: (zones: ZoneInfo[]) => void;
  updateZone: (
    id: number,
    patch: Partial<Pick<ZoneState, "playback" | "volume" | "muted" | "source" | "shuffle" | "repeat" | "track_repeat">>,
  ) => void;
  updateZoneTrack: (
    id: number,
    track: Pick<TrackMetadata, "title" | "artist" | "album" | "duration_ms" | "position_ms" | "seekable" | "cover_url">,
  ) => void;
  updateZoneProgress: (id: number, position_ms: number, duration_ms: number) => void;

  // Client updates
  setClients: (clients: ClientInfo[]) => void;
  updateClient: (
    id: number,
    patch: Partial<Pick<ClientInfo, "volume" | "muted" | "connected" | "zone_index">>,
  ) => void;

  // UI
  selectZone: (id: number) => void;
  setConnected: (v: boolean) => void;
}

export const useAppStore = create<AppState>((set, get) => ({
  zones: new Map(),
  clients: new Map(),
  selectedZone: 1,
  isConnected: false,
  isLoading: true,

  loadAll: async () => {
    set({ isLoading: true });
    try {
      const [zoneList, clientList] = await Promise.all([
        api.zones.list(),
        api.clients.list(),
      ]);

      const zones = new Map<number, ZoneState>();
      for (const z of zoneList) {
        zones.set(z.index, { ...z, track: null });
      }

      // Fetch track metadata for each zone in parallel
      const metaResults = await Promise.allSettled(
        zoneList.map((z) => api.zones.getTrackMetadata(z.index)),
      );
      for (let i = 0; i < zoneList.length; i++) {
        const r = metaResults[i];
        if (r.status === "fulfilled") {
          const zone = zones.get(zoneList[i].index);
          if (zone) zone.track = r.value;
        }
      }

      const clients = new Map<number, ClientInfo>();
      for (const c of clientList) clients.set(c.index, c);

      const stored = typeof window !== "undefined" ? Number(sessionStorage.getItem("selectedZone")) : 0;
      const initial = stored && zones.has(stored) ? stored : (zoneList[0]?.index ?? 1);
      set({ zones, clients, selectedZone: initial, isLoading: false });
    } catch {
      set({ isLoading: false });
    }
  },

  setZones: (list) => {
    const zones = new Map<number, ZoneState>();
    for (const z of list) {
      const existing = get().zones.get(z.index);
      zones.set(z.index, {
        ...z,
        track: existing?.track ?? null,
      });
    }
    set({ zones });
  },

  updateZone: (id, patch) => {
    const zones = new Map(get().zones);
    const z = zones.get(id);
    if (z) zones.set(id, { ...z, ...patch });
    set({ zones });
  },

  updateZoneTrack: (id, track) => {
    const zones = new Map(get().zones);
    const z = zones.get(id);
    if (z) {
      zones.set(id, {
        ...z,
        track: z.track ? { ...z.track, ...track } : null,
      });
    }
    set({ zones });
  },

  updateZoneProgress: (id, position_ms, duration_ms) => {
    const zones = new Map(get().zones);
    const z = zones.get(id);
    if (z?.track) {
      zones.set(id, { ...z, track: { ...z.track, position_ms, duration_ms } });
    }
    set({ zones });
  },

  setClients: (list) => {
    const clients = new Map<number, ClientInfo>();
    for (const c of list) clients.set(c.index, c);
    set({ clients });
  },

  updateClient: (id, patch) => {
    const clients = new Map(get().clients);
    const c = clients.get(id);
    if (c) clients.set(id, { ...c, ...patch });
    set({ clients });
  },

  selectZone: (id) => {
    sessionStorage.setItem("selectedZone", String(id));
    set({ selectedZone: id });
  },
  setConnected: (v) => {
    const was = get().isConnected;
    set({ isConnected: v });
    // On reconnect, re-fetch all state
    if (v && !was) {
      get().loadAll();
    }
  },
}));
