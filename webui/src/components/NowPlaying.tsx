"use client";

import { useState } from "react";
import type { ZoneState } from "@/stores/useAppStore";

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
  const coverUrl = track?.cover || null;
  const [coverError, setCoverError] = useState(false);
  const [lastCover, setLastCover] = useState(coverUrl);

  // Reset error state when cover URL changes
  if (coverUrl !== lastCover) {
    setCoverError(false);
    setLastCover(coverUrl);
  }

  const fallback = (
    <div className="flex flex-col items-center justify-center size-full bg-gradient-to-br from-muted to-muted/60">
      <span className="text-6xl md:text-4xl mb-2 drop-shadow-md">{zone.icon || "🎵"}</span>
      <span className="text-xs font-medium text-muted-foreground/60 tracking-wider uppercase">{zone.name}</span>
    </div>
  );

  if (isIdle || !coverUrl) {
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
            key={`bg-${coverUrl}`}
            src={coverUrl}
            alt=""
            className="absolute inset-0 w-full h-full object-cover scale-110 blur-2xl opacity-40"
            onError={() => setCoverError(true)}
          />
          <img
            key={`fg-${coverUrl}`}
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
