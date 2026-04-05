"use client";

import { useState, useEffect, useRef, useCallback } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  VolumeHighIcon,
  VolumeLowIcon,
  VolumeMute02Icon,
} from "@hugeicons/core-free-icons";
import { Slider } from "@/components/ui/slider";
import { Button } from "@/components/ui/button";
import { api } from "@/lib/api";
import type { ZoneState } from "@/stores/useAppStore";

interface VolumeSliderProps {
  zone: ZoneState;
}

export function VolumeSlider({ zone }: VolumeSliderProps) {
  const [localVolume, setLocalVolume] = useState(zone.volume);
  const [dragging, setDragging] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  // Sync from server only when not dragging
  useEffect(() => {
    if (!dragging) setLocalVolume(zone.volume);
  }, [zone.volume, dragging]);

  const volumeIcon = zone.muted
    ? VolumeMute02Icon
    : localVolume > 50
      ? VolumeHighIcon
      : VolumeLowIcon;

  const handleVolumeChange = useCallback(
    (value: number[]) => {
      const v = value[0];
      setLocalVolume(v);
      setDragging(true);
      // Auto-unmute when user interacts with slider
      if (zone.muted) {
        api.zones.setMute(zone.index, false).catch(() => {});
      }
      // Debounced API call
      clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => {
        api.zones.setVolume(zone.index, v).catch(() => {});
      }, 50);
    },
    [zone.index, zone.muted],
  );

  const handleVolumeCommit = useCallback(
    (value: number[]) => {
      setDragging(false);
      clearTimeout(timerRef.current);
      api.zones.setVolume(zone.index, value[0]).catch(() => {});
    },
    [zone.index],
  );

  const toggleMute = useCallback(() => {
    api.zones.toggleMute(zone.index).catch(() => {});
  }, [zone.index]);

  return (
    <div className="flex items-center gap-3 w-full md:max-w-xs">
      <Button
        variant="ghost"
        size="icon"
        onClick={toggleMute}
        className="size-8 shrink-0 rounded-full"
      >
        <HugeiconsIcon icon={volumeIcon} size={18} />
      </Button>
      <Slider
        value={[zone.muted ? 0 : localVolume]}
        max={100}
        step={1}
        onValueChange={handleVolumeChange}
        onValueCommit={handleVolumeCommit}
        className="flex-1"
      />
      <span className="text-xs text-muted-foreground tabular-nums w-7 text-right">
        {localVolume}
      </span>
    </div>
  );
}
