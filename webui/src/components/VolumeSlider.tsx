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
  sendCommand: (zone: number, action: string, value?: number | boolean) => void;
}

export function VolumeSlider({ zone, sendCommand }: VolumeSliderProps) {
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
      // Debounced API call
      clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => {
        sendCommand(zone.index, "set_volume", v);
      }, 50);
    },
    [zone.index, sendCommand],
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
    sendCommand(zone.index, "toggle_mute");
    api.zones.toggleMute(zone.index).catch(() => {});
  }, [zone.index, sendCommand]);

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
