"use client";

import { HugeiconsIcon } from "@hugeicons/react";
import {
  PlayIcon,
  PauseIcon,
  StopIcon,
  NextIcon,
  PreviousIcon,
} from "@hugeicons/core-free-icons";
import { motion } from "framer-motion";
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
  const hasNavigation = source === "radio" || source === "subsonic_playlist" || isAirPlay;

  const cmd = (action: string) => sendCommand(index, action);

  return (
    <div className="flex items-center justify-center gap-2">
      {/* Previous */}
      <motion.div whileTap={{ scale: 0.9 }}>
        <Button
          variant="ghost"
          size="icon"
          disabled={isIdle || isAirPlay || isUrl || !hasNavigation}
          onClick={() => cmd("previous")}
          className="size-10 rounded-full"
        >
          <HugeiconsIcon icon={PreviousIcon} size={20} />
        </Button>
      </motion.div>

      {/* Play / Pause */}
      <motion.div whileTap={{ scale: 0.9 }}>
        <Button
          variant="default"
          size="icon"
          disabled={isIdle}
          onClick={() => cmd(isPlaying ? "pause" : "play")}
          className="size-12 rounded-full"
        >
          <HugeiconsIcon icon={isPlaying ? PauseIcon : PlayIcon} size={24} />
        </Button>
      </motion.div>

      {/* Stop */}
      <motion.div whileTap={{ scale: 0.9 }}>
        <Button
          variant="ghost"
          size="icon"
          disabled={isIdle}
          onClick={() => cmd("stop")}
          className="size-10 rounded-full"
        >
          <HugeiconsIcon icon={StopIcon} size={20} />
        </Button>
      </motion.div>

      {/* Next */}
      <motion.div whileTap={{ scale: 0.9 }}>
        <Button
          variant="ghost"
          size="icon"
          disabled={isIdle || isAirPlay || isUrl || !hasNavigation}
          onClick={() => cmd("next")}
          className="size-10 rounded-full"
        >
          <HugeiconsIcon icon={NextIcon} size={20} />
        </Button>
      </motion.div>
    </div>
  );
}
