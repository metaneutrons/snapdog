"use client";

import { useState } from "react";
import Image from "next/image";
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
  const coverUrl = zones.coverUrl(zone.index);
  const sourceLabel = SOURCE_LABELS[zone.source];
  const [coverError, setCoverError] = useState(false);

  // Reset error state when zone/track changes
  const coverKey = `${zone.index}-${track?.title}`;

  if (isIdle) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-3 p-6">
        <span className="text-5xl">{zone.icon || "🔊"}</span>
        <h2 className="text-xl font-semibold">{zone.name}</h2>
        <p className="text-sm text-muted-foreground">No audio playing</p>
      </div>
    );
  }

  return (
    <div className="flex flex-1 flex-col items-center p-6 gap-5 overflow-y-auto">
      {/* Cover art with blurred background */}
      <div className="relative w-full max-w-xs aspect-square rounded-2xl overflow-hidden bg-muted shadow-lg">
        {coverError ? (
          <div className="flex items-center justify-center size-full">
            <span className="text-6xl">{zone.icon || "🎵"}</span>
          </div>
        ) : (
          <>
            <Image
              key={`bg-${coverKey}`}
              src={coverUrl}
              alt=""
              fill
              className="object-cover scale-110 blur-2xl opacity-40"
              onError={() => setCoverError(true)}
              unoptimized
            />
            <Image
              key={`fg-${coverKey}`}
              src={coverUrl}
              alt={`${track.title} cover`}
              fill
              className="object-cover"
              onError={() => setCoverError(true)}
              priority
              unoptimized
            />
          </>
        )}
      </div>

      {/* Metadata */}
      <div className="text-center space-y-1 max-w-xs w-full">
        <div className="flex items-center justify-center gap-2">
          <h3 className="text-lg font-semibold truncate">{track.title || "Unknown"}</h3>
          {sourceLabel && (
            <span className="shrink-0 text-[10px] font-medium uppercase tracking-wider px-1.5 py-0.5 rounded-full bg-primary/10 text-primary">
              {sourceLabel}
            </span>
          )}
        </div>
        <p className="text-sm text-muted-foreground truncate">{track.artist || "Unknown Artist"}</p>
        {track.album && (
          <p className="text-xs text-muted-foreground/70 truncate">{track.album}</p>
        )}
      </div>
    </div>
  );
}
