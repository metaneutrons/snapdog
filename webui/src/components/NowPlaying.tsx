"use client";

import { useState, useEffect } from "react";
import type { ZoneState } from "@/stores/useAppStore";
import { zones } from "@/lib/api";

const SOURCE_LABELS: Record<string, string> = {
  radio: "Radio",
  subsonic_playlist: "Subsonic",
  subsonic_track: "Subsonic",
  airplay: "AirPlay",
  url: "URL",
};

export function NowPlaying({ zone }: { zone: ZoneState }) {
  const track = zone.track;
  const isIdle = zone.source === "idle" || !track;
  const isRadio = zone.source === "radio";
  const coverKey = isRadio
    ? `${zone.index}-${track?.title}`
    : `${zone.index}-${track?.title}-${track?.artist}`;
  const [coverVersion, setCoverVersion] = useState(Date.now);
  const coverUrl = `${zones.coverUrl(zone.index)}?v=${coverKey}-${coverVersion}`;
  const [coverError, setCoverError] = useState(false);
  const [lastKey, setLastKey] = useState(coverKey);

  // Reset error state and bump version when track changes
  if (coverKey !== lastKey) {
    setCoverError(false);
    setLastKey(coverKey);
    setCoverVersion(Date.now);
  }

  // Retry cover art after a delay if it failed (cover may arrive after metadata)
  useEffect(() => {
    if (!coverError || isIdle) return;
    const timer = setTimeout(() => {
      setCoverError(false);
      setCoverVersion(Date.now);
    }, 1500);
    return () => clearTimeout(timer);
  }, [coverError, isIdle]);

  const fallback = (
    <div className="flex flex-col items-center justify-center size-full bg-gradient-to-br from-muted to-muted/60">
      <span className="text-6xl md:text-4xl mb-2 drop-shadow-md">{zone.icon || "🎵"}</span>
      <span className="text-xs font-medium text-muted-foreground/60 tracking-wider uppercase">{zone.name}</span>
    </div>
  );

  if (isIdle) {
    return (
      <div className="relative w-full aspect-square rounded-2xl md:rounded-xl overflow-hidden shadow-lg">
        {fallback}
      </div>
    );
  }

  return (
    <div className="relative w-full aspect-square rounded-2xl md:rounded-xl overflow-hidden bg-muted shadow-lg shrink-0">
      {coverError ? fallback : (
        <>
          <img
            key={`bg-${coverKey}-${coverVersion}`}
            src={coverUrl}
            alt=""
            className="absolute inset-0 w-full h-full object-cover scale-110 blur-2xl opacity-40"
            onError={() => setCoverError(true)}
          />
          <img
            key={`fg-${coverKey}-${coverVersion}`}
            src={coverUrl}
            alt={`${track.title} cover`}
            className="absolute inset-0 w-full h-full object-contain"
            onError={() => setCoverError(true)}
          />
        </>
      )}
    </div>
  );
}
