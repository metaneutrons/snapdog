"use client";

import { useEffect, useCallback, useState, Component, type ReactNode } from "react";
import { useTranslations } from "next-intl";
import { useAppStore, type ZoneState } from "@/stores/useAppStore";
import { useWebSocket } from "@/hooks/useWebSocket";
import { useClientDrop } from "@/hooks/useClientDrop";
import { api } from "@/lib/api";
import type { WsNotification } from "@/lib/types";
import { ApiKeyPrompt } from "@/components/ApiKeyPrompt";
import { Skeleton } from "@/components/ui/skeleton";
import { NowPlaying } from "@/components/NowPlaying";
import { TransportControls } from "@/components/TransportControls";
import { EqOverlay } from "@/components/EqOverlay";
import { Button } from "@/components/ui/button";
import { VolumeSlider } from "@/components/VolumeSlider";
import { SeekBar } from "@/components/SeekBar";
import { ShuffleRepeat } from "@/components/ShuffleRepeat";
import { PlaylistBrowser } from "@/components/PlaylistBrowser";
import { ClientList } from "@/components/ClientList";
import { Marquee } from "@/components/Marquee";
import { ConnectionStatus } from "@/components/ConnectionStatus";
import { LocalePicker } from "@/components/LocalePicker";
import { ProgrammingMode } from "@/components/ProgrammingMode";

// ── Error Boundary ────────────────────────────────────────────

function ErrorFallback({ error, onRetry }: { error: Error; onRetry: () => void }) {
  const t = useTranslations("zone");
  return (
    <div className="flex flex-1 items-center justify-center p-6 text-center">
      <div className="space-y-2">
        <p className="text-sm font-medium text-destructive">{t("error")}</p>
        <p className="text-xs text-muted-foreground">{error.message}</p>
        <button onClick={onRetry} className="text-xs text-primary hover:underline">{t("retry")}</button>
      </div>
    </div>
  );
}

class ZoneErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };
  static getDerivedStateFromError(error: Error) { return { error }; }
  render() {
    if (this.state.error) {
      return <ErrorFallback error={this.state.error} onRetry={() => this.setState({ error: null })} />;
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
  const { dragOver, dragHandlers } = useClientDrop(zone.index);
  const t = useTranslations();
  const isPlaying = zone.playback === "playing";
  const hasCover = zone.track?.cover_url && zone.source !== "idle" && !imgError;
  return (
    <button
      onClick={onSelect}
      {...dragHandlers}
      aria-current={selected ? "true" : undefined}
      className={`w-full flex items-center gap-3 px-3 py-3 rounded-lg text-left transition-colors ${
        dragOver
          ? "bg-primary/20 ring-2 ring-primary"
          : selected
            ? "bg-primary/10 text-primary"
            : "hover:bg-muted text-foreground"
      }`}
    >
      {/* Cover thumbnail or zone icon */}
      <div className="relative size-10 rounded-md bg-muted flex items-center justify-center overflow-hidden shrink-0">
        {hasCover ? (
          <img
            src={zone.track!.cover_url!}
            alt=""
            className="size-full object-cover"
            onError={() => setImgError(true)}
          />
        ) : (
          <span className="text-lg">{zone.icon || "🔊"}</span>
        )}
        {isPlaying && (
          <>
            <div className="absolute bottom-0.5 right-0.5 size-2 rounded-full bg-primary animate-pulse" />
            <span className="sr-only">{t("zone.playing")}</span>
          </>
        )}
      </div>
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium truncate">{zone.name}</div>
        <div className="text-xs text-muted-foreground truncate">
          {zone.track && zone.source !== "idle"
            ? `${zone.track.artist} — ${zone.track.title}`
            : t("zone.idle")}
        </div>
      </div>
      <div className="text-xs text-muted-foreground tabular-nums">{zone.volume}</div>
    </button>
  );
}

function MobileZoneTab({ zone, selected, onSelect }: { zone: ZoneState; selected: boolean; onSelect: () => void }) {
  const { dragOver, dragHandlers } = useClientDrop(zone.index);
  return (
    <button
      onClick={onSelect}
      {...dragHandlers}
      className={`shrink-0 px-3 py-2 text-sm rounded-t-md transition-colors ${
        dragOver
          ? "bg-primary/20 ring-2 ring-primary"
          : selected
            ? "text-primary border-b-2 border-primary font-medium"
            : "text-muted-foreground"
      }`}
      role="tab"
      aria-selected={selected}
    >
      {zone.name}
    </button>
  );
}

// ── Zone Detail — composes all control components ─────────────

const SOURCE_KEYS: Record<string, string> = {
  radio: "radio",
  subsonic_playlist: "subsonic_playlist",
  subsonic_track: "subsonic_track",
  airplay: "airplay",
  spotify: "spotify",
  url: "url",
};

function ZoneHeader({ zone }: { zone: ZoneState }) {
  const t = useTranslations();
  const sourceKey = SOURCE_KEYS[zone.source];
  return (
    <div className="flex items-center justify-between gap-2">
      <h2 className="text-sm font-semibold truncate">{zone.name}</h2>
      <div className="flex items-center gap-1.5 shrink-0">
        {zone.presence && (
          <span
            className="text-[10px] px-1 py-0.5 rounded-full bg-green-500/15 text-green-600"
            role="status"
            aria-label={zone.presenceTimerActive ? t("zone.presenceTimerActive") : t("zone.presenceDetected")}
          >
            {zone.presenceTimerActive ? "⏱️" : "🟢"}
          </span>
        )}
        {sourceKey ? (
        <span className="text-[10px] font-medium uppercase tracking-wider px-1.5 py-0.5 rounded-full bg-primary/10 text-primary">
          {t(`source.${sourceKey}`)}
        </span>
      ) : (
        <span className="text-[10px] text-muted-foreground">{t("zone.idle")}</span>
      )}
      </div>
    </div>
  );
}

function ZoneDropTarget({ zoneIndex, children }: { zoneIndex: number; children: ReactNode }) {
  const { dragOver, dragHandlers } = useClientDrop(zoneIndex);

  return (
    <div
      className={`border rounded-xl bg-card overflow-hidden transition-colors ${dragOver ? "border-primary border-2" : "border-border"}`}
      {...dragHandlers}
    >
      {children}
    </div>
  );
}

function TrackInfo({ zone }: { zone: ZoneState }) {
  const t = useTranslations("zone");
  const track = zone.track;
  const isIdle = zone.source === "idle" || !track;

  return (
    <div className="text-center sm:text-left space-y-0.5 w-full">
      <Marquee className="text-base font-bold leading-snug">{isIdle ? "\u00A0" : (track.title || t("unknownTitle"))}</Marquee>
      <Marquee className="text-sm text-muted-foreground">{isIdle ? t("noAudio") : (track.artist || t("unknownArtist"))}</Marquee>
      <Marquee className="text-xs text-muted-foreground/70">{isIdle ? "\u00A0" : (track.album || "\u00A0")}</Marquee>
    </div>
  );
}

function ZoneDetail({ zone }: { zone: ZoneState }) {
  const [showEq, setShowEq] = useState(false);
  const [eqEnabled, setEqEnabled] = useState(false);
  const t = useTranslations();

  useEffect(() => {
    api.eq.get(zone.index).then((c) => setEqEnabled(c.enabled)).catch(() => {});
  }, [zone.index]);
  return (
    <div className="flex flex-1 flex-col overflow-y-auto">
      <div className="w-full max-w-[calc(100%-2rem)] mx-auto sm:max-w-[600px] space-y-3 px-4 py-4 sm:px-5 sm:py-4">
        <div className="hidden sm:block"><ZoneHeader zone={zone} /></div>
        {/* Compact+: horizontal layout for cover + controls */}
        <div className="sm:flex sm:gap-5 sm:items-start">
          <div className="sm:w-48 lg:w-56 sm:shrink-0">
            <NowPlaying zone={zone} />
          </div>
          <div className="space-y-3 sm:flex-1 sm:min-w-0 sm:max-w-sm sm:min-h-56 sm:justify-between">
            <TrackInfo zone={zone} />
            <SeekBar zone={zone} />
            <div className="flex items-center gap-2">
              <div className="flex-1"><TransportControls zone={zone} /></div>
              <Button variant="ghost" size="sm" onClick={() => setShowEq(true)} className={`text-xs px-2 ${eqEnabled ? "text-orange-500 font-bold" : ""}`} aria-label={t("eq.title", { zone: zone.name })}>
                EQ
              </Button>
            </div>
            <ShuffleRepeat zone={zone} />
            <VolumeSlider
              volume={zone.volume}
              muted={zone.muted}
              onVolumeChange={(v) => api.zones.setVolume(zone.index, v).catch((e: unknown) => console.error("API error", e))}
              onMuteToggle={() => api.zones.toggleMute(zone.index).catch((e: unknown) => console.error("API error", e))}
              onUnmute={() => api.zones.setMute(zone.index, false).catch((e: unknown) => console.error("API error", e))}
            />
          </div>
        </div>
        <ClientList zone={zone} />
        <PlaylistBrowser zone={zone} />
      </div>
      {showEq && <EqOverlay zoneId={zone.index} label={zone.name} onClose={() => { setShowEq(false); api.eq.get(zone.index).then((c) => setEqEnabled(c.enabled)).catch(() => {}); }} />}
    </div>
  );
}

// ── App Shell ─────────────────────────────────────────────────

export default function Home() {
  const t = useTranslations();
  const {
    zones: zoneMap,
    selectedZone,
    selectZone,
    isLoading,
    needsAuth,
    setConnected,
    loadAll,
    updateZone,
    updateZoneTrack,
    updateZoneProgress,
    updateZonePresence,
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
          updateZoneTrack(n.zone, { ...n, cover_url: n.cover_url });
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
        case "zone_presence_changed":
          updateZonePresence(n.zone, n.presence, n.enabled, n.timer_active);
          break;
        case "zone_eq_changed":
          // EqOverlay manages its own state; this ensures exhaustive switch coverage
          break;
      }
    },
    [updateZone, updateZoneTrack, updateZoneProgress, updateZonePresence, updateClient],
  );

  const { isConnected: wsConnected } = useWebSocket(handleNotification);

  useEffect(() => { setConnected(wsConnected); }, [wsConnected, setConnected]);
  useEffect(() => { loadAll(); }, [loadAll]);

  const zoneList = Array.from(zoneMap.values());
  const currentZone = zoneMap.get(selectedZone) ?? zoneList[0];

  if (needsAuth) {
    return <ApiKeyPrompt onAuthenticated={() => loadAll()} />;
  }

  if (isLoading) {
    return (
      <div className="flex flex-1 h-full">
        {/* Skeleton sidebar */}
        <aside className="hidden lg:flex xl:hidden flex-col border-r border-border bg-card lg:w-56 shrink-0">
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
      <a href="#main-content" className="sr-only focus:not-sr-only focus:absolute focus:z-[100] focus:top-2 focus:left-2 focus:px-4 focus:py-2 focus:bg-primary focus:text-primary-foreground focus:rounded-md">
        {t("app.skipToContent")}
      </a>
      <ConnectionStatus />
      {/* ── Sidebar / Rail (tablet only) ──────────────────── */}
      <aside className="hidden lg:flex xl:hidden flex-col border-r border-border bg-card lg:w-56 shrink-0" aria-label={t("zone.navigation")}>
        <div className="px-4 py-4 border-b border-border flex items-center justify-between">
          <h1 className="text-base font-semibold">SnapDog</h1>
          <ProgrammingMode /><LocalePicker />
        </div>
        <nav className="flex-1 overflow-y-auto p-2 space-y-0.5" aria-label={t("zone.zones")}>
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
      <main className="flex flex-1 flex-col min-w-0" id="main-content">
        {/* Header (mobile + compact + wide — hidden when sidebar visible at lg–xl) */}
        <header className="flex lg:hidden items-center justify-between px-4 py-3 border-b border-border">
          <h1 className="text-base font-semibold">SnapDog</h1>
          <ProgrammingMode /><LocalePicker />
        </header>

        {/* Wide header (xl+) */}
        <header className="hidden xl:flex items-center justify-between px-6 py-3 border-b border-border">
          <h1 className="text-base font-semibold">SnapDog</h1>
          <ProgrammingMode /><LocalePicker />
        </header>

        {/* Zone tabs (mobile + compact + normal without sidebar visible) */}
        <div className="flex lg:hidden overflow-x-auto border-b border-border px-2 gap-1 scrollbar-none" role="tablist" aria-label={t("zone.zones")}>
          {zoneList.map((z) => (
            <MobileZoneTab key={z.index} zone={z} selected={z.index === selectedZone} onSelect={() => selectZone(z.index)} />
          ))}
        </div>

        {/* Desktop: all zones in responsive grid (xl+) */}
        <div className="hidden xl:flex xl:flex-wrap xl:gap-4 xl:p-4 xl:justify-center flex-1 overflow-y-auto">
          {zoneList.map((z) => (
            <ZoneErrorBoundary key={z.index}>
              <div className="w-full" style={{ minWidth: '480px', maxWidth: '600px', flex: '1 1 480px' }}>
                <ZoneDropTarget zoneIndex={z.index}>
                  <ZoneDetail zone={z} />
                </ZoneDropTarget>
              </div>
            </ZoneErrorBoundary>
          ))}
        </div>

        {/* Mobile/Tablet: single selected zone */}
        <div className="xl:hidden flex-1">
          {currentZone && (
            <ZoneErrorBoundary>
              <ZoneDetail zone={currentZone} />
            </ZoneErrorBoundary>
          )}
        </div>
      </main>
    </div>
  );
}
