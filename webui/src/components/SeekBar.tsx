"use client";

import { useState, useEffect, useRef, useCallback } from "react";
import { Slider } from "@/components/ui/slider";
import { api } from "@/lib/api";
import type { ZoneState } from "@/stores/useAppStore";

function formatTime(ms: number): string {
  const totalSec = Math.floor(ms / 1000);
  const min = Math.floor(totalSec / 60);
  const sec = totalSec % 60;
  return `${min}:${sec.toString().padStart(2, "0")}`;
}

export function SeekBar({ zone }: { zone: ZoneState }) {
  const track = zone.track;
  const duration = track?.duration_ms ?? 0;
  const serverPosition = track?.position_ms ?? 0;
  const isPlaying = zone.playback === "playing";
  const isIdle = zone.source === "idle" || !track;
  const canSeek = zone.source === "subsonic_playlist" || zone.source === "subsonic_track";

  const [localPosition, setLocalPosition] = useState(serverPosition);
  const [dragging, setDragging] = useState(false);
  const lastServerRef = useRef(serverPosition);

  // Sync from server when position changes externally
  useEffect(() => {
    if (!dragging && serverPosition !== lastServerRef.current) {
      setLocalPosition(serverPosition);
      lastServerRef.current = serverPosition;
    }
  }, [serverPosition, dragging]);

  // Client-side interpolation: tick forward every 250ms while playing
  useEffect(() => {
    if (!isPlaying || dragging || isIdle) return;
    const interval = setInterval(() => {
      setLocalPosition((prev) => Math.min(prev + 250, duration));
    }, 250);
    return () => clearInterval(interval);
  }, [isPlaying, dragging, isIdle, duration]);

  const handleSeek = useCallback(
    (value: number[]) => {
      if (!canSeek) return;
      setLocalPosition(value[0]);
      setDragging(true);
    },
    [canSeek],
  );

  const handleSeekCommit = useCallback(
    (value: number[]) => {
      if (!canSeek) return;
      setDragging(false);
      lastServerRef.current = value[0];
      api.zones.seekPosition(zone.index, value[0]).catch(() => {});
    },
    [zone.index, canSeek],
  );

  if (isIdle) return null;

  return (
    <div className="w-full max-w-xs space-y-1">
      <Slider
        value={[localPosition]}
        max={duration || 1}
        step={1000}
        onValueChange={handleSeek}
        onValueCommit={handleSeekCommit}
        disabled={!canSeek}
        className="w-full"
      />
      <div className="flex justify-between text-[10px] text-muted-foreground tabular-nums">
        <span>{formatTime(localPosition)}</span>
        <span>{duration > 0 ? formatTime(duration) : "--:--"}</span>
      </div>
    </div>
  );
}
