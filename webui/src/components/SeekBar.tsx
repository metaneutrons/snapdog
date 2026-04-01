"use client";

import { useCallback } from "react";
import { Slider } from "@/components/ui/slider";
import { api } from "@/lib/api";
import type { ZoneState } from "@/stores/useAppStore";

function formatTime(ms: number): string {
  const totalSec = Math.floor(ms / 1000);
  const min = Math.floor(totalSec / 60);
  const sec = totalSec % 60;
  return `${min}:${sec.toString().padStart(2, "0")}`;
}

interface SeekBarProps {
  zone: ZoneState;
}

export function SeekBar({ zone }: SeekBarProps) {
  const track = zone.track;
  if (!track || zone.source === "idle") return null;

  const duration = track.duration_ms;
  const position = track.position_ms;
  const canSeek = zone.source === "subsonic_playlist" || zone.source === "subsonic_track";

  const handleSeek = useCallback(
    (value: number[]) => {
      if (!canSeek) return;
      api.zones.seekPosition(zone.index, value[0]).catch(() => {});
    },
    [zone.index, canSeek],
  );

  return (
    <div className="w-full max-w-xs space-y-1">
      <Slider
        value={[position]}
        max={duration || 1}
        step={1000}
        onValueChange={handleSeek}
        disabled={!canSeek}
        className="w-full"
      />
      <div className="flex justify-between text-[10px] text-muted-foreground tabular-nums">
        <span>{formatTime(position)}</span>
        <span>{duration > 0 ? formatTime(duration) : "--:--"}</span>
      </div>
    </div>
  );
}
