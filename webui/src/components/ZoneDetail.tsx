"use client";

import { useState } from "react";
import { useTranslations } from "next-intl";
import { api } from "@/lib/api";
import { logApiError } from "@/lib/log-api-error";
import { useEqEnabled } from "@/hooks/useEqEnabled";
import type { ZoneState } from "@/stores/useAppStore";
import type { SourceType } from "@/lib/types";
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

const SOURCE_KEYS: Partial<Record<SourceType, string>> = {
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

export function ZoneDetail({ zone }: { zone: ZoneState }) {
  const [showEq, setShowEq] = useState(false);
  const [eqEnabled, setEqEnabled] = useEqEnabled({ zoneId: zone.index });
  const t = useTranslations();

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
              onVolumeChange={(v) => api.zones.setVolume(zone.index, v).catch(logApiError)}
              onMuteToggle={() => api.zones.toggleMute(zone.index).catch(logApiError)}
              onUnmute={() => api.zones.setMute(zone.index, false).catch(logApiError)}
            />
          </div>
        </div>
        <ClientList zone={zone} />
        <PlaylistBrowser zone={zone} />
      </div>
      {showEq && <EqOverlay zoneId={zone.index} label={zone.name} onClose={(enabled) => { setShowEq(false); setEqEnabled(enabled); }} />}
    </div>
  );
}
