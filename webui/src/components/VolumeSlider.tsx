"use client";

import { useRef, useCallback } from "react";
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
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  const volumeIcon = zone.muted
    ? VolumeMute02Icon
    : zone.volume > 50
      ? VolumeHighIcon
      : VolumeLowIcon;

  const handleVolumeChange = useCallback(
    (value: number[]) => {
      const v = value[0];
      // Optimistic: update via WS immediately
      sendCommand(zone.index, "set_volume", v);
      // Debounced REST call as backup
      clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => {
        api.zones.setVolume(zone.index, v).catch(() => {});
      }, 300);
    },
    [zone.index, sendCommand],
  );

  const toggleMute = useCallback(() => {
    sendCommand(zone.index, "toggle_mute");
    api.zones.toggleMute(zone.index).catch(() => {});
  }, [zone.index, sendCommand]);

  return (
    <div className="flex items-center gap-3 w-full max-w-xs">
      <Button
        variant="ghost"
        size="icon"
        onClick={toggleMute}
        className="size-8 shrink-0 rounded-full"
      >
        <HugeiconsIcon icon={volumeIcon} size={18} />
      </Button>
      <Slider
        value={[zone.muted ? 0 : zone.volume]}
        max={100}
        step={1}
        onValueChange={handleVolumeChange}
        className="flex-1"
      />
      <span className="text-xs text-muted-foreground tabular-nums w-7 text-right">
        {zone.volume}
      </span>
    </div>
  );
}
