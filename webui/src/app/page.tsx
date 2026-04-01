"use client";

import { useEffect, useCallback, useState } from "react";
import { useAppStore, type ZoneState } from "@/stores/useAppStore";
import { useWebSocket } from "@/hooks/useWebSocket";
import { api, zones } from "@/lib/api";
import type { WsNotification } from "@/lib/types";
import { NowPlaying } from "@/components/NowPlaying";
import { TransportControls } from "@/components/TransportControls";
import { VolumeSlider } from "@/components/VolumeSlider";
import { SeekBar } from "@/components/SeekBar";
import { ShuffleRepeat } from "@/components/ShuffleRepeat";
import { RadioStations } from "@/components/RadioStations";
import { PlaylistBrowser } from "@/components/PlaylistBrowser";
import { ClientList } from "@/components/ClientList";
import { ConnectionStatus } from "@/components/ConnectionStatus";

// ── Zone Rail Item (tablet/desktop sidebar) ───────────────────

function ZoneRailItem({ zone, selected, onSelect }: {
  zone: ZoneState;
  selected: boolean;
  onSelect: () => void;
}) {
  const [imgError, setImgError] = useState(false);
  const isPlaying = zone.playback === "playing";
  const hasCover = zone.track && zone.source !== "idle" && !imgError;
  return (
    <button
      onClick={onSelect}
      className={`w-full flex items-center gap-3 px-3 py-3 rounded-lg text-left transition-colors ${
        selected
          ? "bg-primary/10 text-primary"
          : "hover:bg-muted text-foreground"
      }`}
    >
      {/* Cover thumbnail or zone icon */}
      <div className="relative size-10 rounded-md bg-muted flex items-center justify-center overflow-hidden shrink-0">
        {hasCover ? (
          <img
            src={zones.coverUrl(zone.index)}
            alt=""
            className="size-full object-cover"
            onError={() => setImgError(true)}
          />
        ) : (
          <span className="text-lg">{zone.icon || "🔊"}</span>
        )}
        {isPlaying && (
          <div className="absolute bottom-0.5 right-0.5 size-2 rounded-full bg-primary animate-pulse" />
        )}
      </div>
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium truncate">{zone.name}</div>
        <div className="text-xs text-muted-foreground truncate">
          {zone.track && zone.source !== "idle"
            ? `${zone.track.artist} — ${zone.track.title}`
            : "Idle"}
        </div>
      </div>
      <div className="text-xs text-muted-foreground tabular-nums">{zone.volume}</div>
    </button>
  );
}

// ── Zone Detail — composes all control components ─────────────

function ZoneDetail({ zone, sendCommand }: { zone: ZoneState; sendCommand: (zone: number, action: string, value?: string | number | boolean) => void }) {
  return (
    <div className="flex flex-1 flex-col items-center overflow-y-auto">
      <NowPlaying zone={zone} />
      <div className="w-full max-w-xs space-y-4 px-6 pb-6">
        <SeekBar zone={zone} />
        <TransportControls zone={zone} sendCommand={sendCommand} />
        <ShuffleRepeat zone={zone} />
        <RadioStations zone={zone} />
        <VolumeSlider zone={zone} sendCommand={sendCommand} />
        <ClientList zone={zone} />
        <PlaylistBrowser zone={zone} />
      </div>
    </div>
  );
}

// ── App Shell ─────────────────────────────────────────────────

export default function Home() {
  const {
    zones: zoneMap,
    selectedZone,
    selectZone,
    isLoading,
    isConnected,
    setConnected,
    loadAll,
    updateZone,
    updateZoneTrack,
    updateZoneProgress,
    updateClient,
  } = useAppStore();

  const handleNotification = useCallback(
    (n: WsNotification) => {
      switch (n.type) {
        case "zone_state_changed":
          updateZone(n.zone, {
            playback: n.playback,
            volume: n.volume,
            muted: n.muted,
            source: n.source,
          });
          break;
        case "zone_track_changed":
          updateZoneTrack(n.zone, n);
          // Re-fetch full metadata for complete data
          api.zones.getTrackMetadata(n.zone).then((meta) => {
            const zones = useAppStore.getState().zones;
            const z = zones.get(n.zone);
            if (z) {
              const updated = new Map(zones);
              updated.set(n.zone, { ...z, track: meta });
              useAppStore.setState({ zones: updated });
            }
          }).catch(() => {});
          break;
        case "zone_progress":
          updateZoneProgress(n.zone, n.position_ms, n.duration_ms);
          break;
        case "client_state_changed":
          updateClient(n.client, {
            volume: n.volume,
            muted: n.muted,
            connected: n.connected,
            zone_index: n.zone,
          });
          break;
      }
    },
    [updateZone, updateZoneTrack, updateZoneProgress, updateClient],
  );

  const { isConnected: wsConnected, sendCommand } = useWebSocket(handleNotification);

  useEffect(() => { setConnected(wsConnected); }, [wsConnected, setConnected]);
  useEffect(() => { loadAll(); }, [loadAll]);

  const zoneList = Array.from(zoneMap.values());
  const currentZone = zoneMap.get(selectedZone) ?? zoneList[0];

  if (isLoading) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <div className="text-center space-y-3">
          <div className="size-8 border-2 border-primary border-t-transparent rounded-full animate-spin mx-auto" />
          <p className="text-sm text-muted-foreground">Loading zones…</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-1 h-full">
      <ConnectionStatus />
      {/* ── Sidebar / Rail (hidden on mobile) ──────────────── */}
      <aside className="hidden md:flex flex-col border-r border-border bg-card md:w-60 xl:w-70 shrink-0">
        <div className="px-4 py-4 border-b border-border flex items-center justify-between">
          <h1 className="text-base font-semibold">SnapDog</h1>
          <div className={`size-2 rounded-full ${isConnected ? "bg-green-500" : "bg-destructive"}`} />
        </div>
        <nav className="flex-1 overflow-y-auto p-2 space-y-0.5">
          {zoneList.map((z) => (
            <ZoneRailItem
              key={z.index}
              zone={z}
              selected={z.index === selectedZone}
              onSelect={() => selectZone(z.index)}
            />
          ))}
        </nav>
      </aside>

      {/* ── Main content ───────────────────────────────────── */}
      <main className="flex flex-1 flex-col min-w-0">
        {/* Mobile header (visible only on mobile) */}
        <header className="flex md:hidden items-center justify-between px-4 py-3 border-b border-border">
          <h1 className="text-base font-semibold">SnapDog</h1>
          <div className={`size-2 rounded-full ${isConnected ? "bg-green-500" : "bg-destructive"}`} />
        </header>

        {/* Mobile zone tabs */}
        <div className="flex md:hidden overflow-x-auto border-b border-border px-2 gap-1 scrollbar-none">
          {zoneList.map((z) => (
            <button
              key={z.index}
              onClick={() => selectZone(z.index)}
              className={`shrink-0 px-3 py-2 text-sm rounded-t-md transition-colors ${
                z.index === selectedZone
                  ? "text-primary border-b-2 border-primary font-medium"
                  : "text-muted-foreground"
              }`}
            >
              {z.name}
            </button>
          ))}
        </div>

        {/* Zone detail */}
        {currentZone && <ZoneDetail zone={currentZone} sendCommand={sendCommand} />}
      </main>
    </div>
  );
}
