"use client";

import { useState } from "react";
import { useTranslations } from "next-intl";
import { useClientDrop } from "@/hooks/useClientDrop";
import type { ZoneState } from "@/stores/useAppStore";

interface ZoneRailItemProps {
  zone: ZoneState;
  selected: boolean;
  onSelect: () => void;
}

export function ZoneRailItem({ zone, selected, onSelect }: ZoneRailItemProps) {
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
