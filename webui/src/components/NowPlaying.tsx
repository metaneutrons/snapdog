"use client";

import { useState, useEffect } from "react";
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
  const sourceLabel = SOURCE_LABELS[zone.source];
  const coverKey = `${zone.index}-${track?.title}-${track?.artist}`;
  const [coverVersion, setCoverVersion] = useState(0);
  const coverUrl = `${zones.coverUrl(zone.index)}?v=${coverKey}-${coverVersion}`;
  const [coverError, setCoverError] = useState(false);
  const [lastKey, setLastKey] = useState(coverKey);

  // Reset error state and bump version when track changes
  if (coverKey !== lastKey) {
    setCoverError(false);
    setLastKey(coverKey);
    setCoverVersion((v) => v + 1);
  }

  // Retry cover art after a delay if it failed (cover may arrive after metadata)
  useEffect(() => {
    if (!coverError || isIdle) return;
    const timer = setTimeout(() => {
      setCoverError(false);
      setCoverVersion((v) => v + 1);
    }, 1500);
    return () => clearTimeout(timer);
  }, [coverError, isIdle]);

  if (isIdle) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-12">
        <span className="text-5xl">{zone.icon || "🔊"}</span>
        <h2 className="text-lg font-semibold">{zone.name}</h2>
        <p className="text-sm text-muted-foreground">No audio playing</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col items-center gap-5">
      {/* Cover art with blurred background */}
      <div className="relative w-full aspect-square rounded-2xl xl:rounded-xl overflow-hidden bg-muted shadow-lg shrink-0">
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
      <div className="text-center space-y-1 w-full">
        <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">{zone.name}</p>
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
