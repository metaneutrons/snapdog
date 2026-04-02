"use client";

import { useEffect, useCallback, useState, Component, type ReactNode, type DragEvent } from "react";
import { useAppStore, type ZoneState } from "@/stores/useAppStore";
import { useWebSocket } from "@/hooks/useWebSocket";
import { api, zones } from "@/lib/api";
import type { WsNotification } from "@/lib/types";
import { Skeleton } from "@/components/ui/skeleton";
import { NowPlaying } from "@/components/NowPlaying";
import { TransportControls } from "@/components/TransportControls";
import { VolumeSlider } from "@/components/VolumeSlider";
import { SeekBar } from "@/components/SeekBar";
import { ShuffleRepeat } from "@/components/ShuffleRepeat";
import { RadioStations } from "@/components/RadioStations";
import { PlaylistBrowser } from "@/components/PlaylistBrowser";
import { ClientList } from "@/components/ClientList";
import { ConnectionStatus } from "@/components/ConnectionStatus";

// ── Error Boundary ────────────────────────────────────────────

class ZoneErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };
  static getDerivedStateFromError(error: Error) { return { error }; }
  render() {
    if (this.state.error) {
      return (
        <div className="flex flex-1 items-center justify-center p-6 text-center">
          <div className="space-y-2">
            <p className="text-sm font-medium text-destructive">Something went wrong</p>
            <p className="text-xs text-muted-foreground">{this.state.error.message}</p>
            <button
              onClick={() => this.setState({ error: null })}
              className="text-xs text-primary hover:underline"
            >
              Try again
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}

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

const SOURCE_LABELS: Record<string, string> = {
  radio: "Radio",
  subsonic_playlist: "Subsonic",
  subsonic_track: "Subsonic",
  airplay: "AirPlay",
  url: "URL",
};

function ZoneHeader({ zone }: { zone: ZoneState }) {
  const sourceLabel = SOURCE_LABELS[zone.source];
  return (
    <div className="flex items-center justify-between gap-2">
      <h2 className="text-sm font-semibold truncate">{zone.name}</h2>
      {sourceLabel ? (
        <span className="shrink-0 text-[10px] font-medium uppercase tracking-wider px-1.5 py-0.5 rounded-full bg-primary/10 text-primary">
          {sourceLabel}
        </span>
      ) : (
        <span className="shrink-0 text-[10px] text-muted-foreground">Idle</span>
      )}
    </div>
  );
}

function ZoneDropTarget({ zoneIndex, children }: { zoneIndex: number; children: ReactNode }) {
  const [dragOver, setDragOver] = useState(false);

  const handleDragOver = (e: DragEvent) => {
    if (e.dataTransfer.types.includes("application/x-snapdog-client")) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
      setDragOver(true);
    }
  };

  const handleDrop = (e: DragEvent) => {
    e.preventDefault();
    setDragOver(false);
    const clientIndex = Number(e.dataTransfer.getData("application/x-snapdog-client"));
    if (!isNaN(clientIndex)) {
      api.clients.setZone(clientIndex, zoneIndex).catch(() => {});
    }
  };

  return (
    <div
      className={`border rounded-xl bg-card overflow-hidden transition-colors ${dragOver ? "border-primary border-2" : "border-border"}`}
      onDragOver={handleDragOver}
      onDragLeave={() => setDragOver(false)}
      onDrop={handleDrop}
    >
      {children}
    </div>
  );
}

function TrackInfo({ zone }: { zone: ZoneState }) {
  const track = zone.track;
  const isIdle = zone.source === "idle" || !track;

  if (isIdle) {
    return (
      <div className="text-center xl:text-left py-1">
        <p className="text-sm text-muted-foreground">No audio playing</p>
      </div>
    );
  }

  return (
    <div className="text-center xl:text-left space-y-0.5 w-full">
      <h3 className="text-base font-bold leading-snug">{track.title || "Unknown"}</h3>
      <p className="text-sm text-muted-foreground truncate">{track.artist || "Unknown Artist"}</p>
      {track.album && (
        <p className="text-xs text-muted-foreground/70 truncate">{track.album}</p>
      )}
    </div>
  );
}

function ZoneDetail({ zone, sendCommand }: { zone: ZoneState; sendCommand: (zone: number, action: string, value?: string | number | boolean) => void }) {
  return (
    <div className="flex flex-1 flex-col overflow-y-auto">
      <div className="w-full max-w-xs mx-auto xl:max-w-none space-y-3 px-4 py-4 xl:px-5 xl:py-4">
        <ZoneHeader zone={zone} />
        {/* Desktop: horizontal layout for cover + controls */}
        <div className="xl:flex xl:gap-5 xl:items-stretch">
          <div className="xl:w-56 xl:shrink-0">
            <NowPlaying zone={zone} />
          </div>
          <div className="space-y-3 xl:flex-1 xl:min-w-0">
            <TrackInfo zone={zone} />
            <SeekBar zone={zone} />
            <TransportControls zone={zone} sendCommand={sendCommand} />
            <ShuffleRepeat zone={zone} />
            <RadioStations zone={zone} />
            <VolumeSlider zone={zone} sendCommand={sendCommand} />
            <ClientList zone={zone} />
          </div>
        </div>
        {/* Full-width below the horizontal row */}
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
            shuffle: n.shuffle,
            repeat: n.repeat,
            track_repeat: n.track_repeat,
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
      <div className="flex flex-1 h-full">
        {/* Skeleton sidebar */}
        <aside className="hidden md:flex flex-col border-r border-border bg-card md:w-60 xl:w-70 shrink-0">
          <div className="px-4 py-4 border-b border-border">
            <Skeleton className="h-5 w-24" />
          </div>
          <div className="p-2 space-y-2">
            {[1, 2, 3].map((i) => (
              <div key={i} className="flex items-center gap-3 px-3 py-3">
                <Skeleton className="size-10 rounded-md" />
                <div className="flex-1 space-y-1.5">
                  <Skeleton className="h-3.5 w-24" />
                  <Skeleton className="h-3 w-32" />
                </div>
              </div>
            ))}
          </div>
        </aside>
        {/* Skeleton main */}
        <main className="flex flex-1 flex-col items-center justify-center gap-5 p-6">
          <Skeleton className="w-full max-w-xs aspect-square rounded-2xl" />
          <Skeleton className="h-5 w-40" />
          <Skeleton className="h-4 w-28" />
          <Skeleton className="h-10 w-48 rounded-full" />
        </main>
      </div>
    );
  }

  return (
    <div className="flex flex-1 h-full">
      <ConnectionStatus />
      {/* ── Sidebar / Rail (tablet only) ──────────────────── */}
      <aside className="hidden md:flex xl:hidden flex-col border-r border-border bg-card md:w-60 shrink-0">
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
        {/* Mobile header */}
        <header className="flex md:hidden items-center justify-between px-4 py-3 border-b border-border">
          <h1 className="text-base font-semibold">SnapDog</h1>
          <div className={`size-2 rounded-full ${isConnected ? "bg-green-500" : "bg-destructive"}`} />
        </header>

        {/* Desktop header (xl+) */}
        <header className="hidden xl:flex items-center justify-between px-6 py-3 border-b border-border">
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

        {/* Desktop: all zones in responsive grid (xl+) */}
        <div className="hidden xl:grid xl:gap-4 xl:p-4 flex-1 overflow-y-auto" style={{ gridTemplateColumns: 'repeat(auto-fill, minmax(520px, 1fr))' }}>
          {zoneList.map((z) => (
            <ZoneErrorBoundary key={z.index}>
              <ZoneDropTarget zoneIndex={z.index}>
                <ZoneDetail zone={z} sendCommand={sendCommand} />
              </ZoneDropTarget>
            </ZoneErrorBoundary>
          ))}
        </div>

        {/* Mobile/Tablet: single selected zone */}
        <div className="xl:hidden flex-1">
          {currentZone && (
            <ZoneErrorBoundary>
              <ZoneDetail zone={currentZone} sendCommand={sendCommand} />
            </ZoneErrorBoundary>
          )}
        </div>
      </main>
    </div>
  );
}
