"use client";

import { HugeiconsIcon } from "@hugeicons/react";
import {
  PlayIcon,
  PauseIcon,
  StopIcon,
  NextIcon,
  PreviousIcon,
} from "@hugeicons/core-free-icons";
import type { ZoneState } from "@/stores/useAppStore";
import { Button } from "@/components/ui/button";

interface TransportControlsProps {
  zone: ZoneState;
  sendCommand: (zone: number, action: string) => void;
}

export function TransportControls({ zone, sendCommand }: TransportControlsProps) {
  const { index, playback, source } = zone;
  const isPlaying = playback === "playing";
  const isIdle = source === "idle";
  const isAirPlay = source === "airplay";
  const isUrl = source === "url";
  const hasNavigation = source === "radio" || source === "subsonic_playlist";

  const cmd = (action: string) => sendCommand(index, action);

  return (
    <div className="flex items-center justify-center gap-2">
      {/* Previous */}
      <Button
        variant="ghost"
        size="icon"
        disabled={isIdle || isAirPlay || isUrl || !hasNavigation}
        onClick={() => cmd("previous")}
        className="size-10 rounded-full"
      >
        <HugeiconsIcon icon={PreviousIcon} size={20} />
      </Button>

      {/* Play / Pause */}
      <Button
        variant="default"
        size="icon"
        disabled={isIdle || isAirPlay}
        onClick={() => cmd(isPlaying ? "pause" : "play")}
        className="size-12 rounded-full"
      >
        <HugeiconsIcon icon={isPlaying ? PauseIcon : PlayIcon} size={24} />
      </Button>

      {/* Stop */}
      <Button
        variant="ghost"
        size="icon"
        disabled={isIdle || isAirPlay}
        onClick={() => cmd("stop")}
        className="size-10 rounded-full"
      >
        <HugeiconsIcon icon={StopIcon} size={20} />
      </Button>

      {/* Next */}
      <Button
        variant="ghost"
        size="icon"
        disabled={isIdle || isAirPlay || isUrl || !hasNavigation}
        onClick={() => cmd("next")}
        className="size-10 rounded-full"
      >
        <HugeiconsIcon icon={NextIcon} size={20} />
      </Button>
    </div>
  );
}
