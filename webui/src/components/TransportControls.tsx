import { HugeiconsIcon } from "@hugeicons/react";
import {
  PlayIcon,
  PauseIcon,
  StopIcon,
  NextIcon,
  PreviousIcon,
} from "@hugeicons/core-free-icons";
import { motion, useReducedMotion } from "framer-motion";
import { useTranslations } from "next-intl";
import type { ZoneState } from "@/stores/useAppStore";
import { Button } from "@/components/ui/button";
import { api } from "@/lib/api";

interface TransportControlsProps {
  zone: ZoneState;
}

export function TransportControls({ zone }: TransportControlsProps) {
  const t = useTranslations("transport");
  const { index, playback, source } = zone;
  const reduced = useReducedMotion();
  const tap = reduced ? {} : { scale: 0.9 };
  const isPlaying = playback === "playing";
  const isIdle = source === "idle";
  const isAirPlay = source === "airplay";
  const isUrl = source === "url";
  const hasNavigation = source === "radio" || source === "subsonic_playlist" || isAirPlay;

  const cmd = (action: string) => {
    switch (action) {
      case "play": api.zones.play(index).catch(() => {}); break;
      case "pause": api.zones.pause(index).catch(() => {}); break;
      case "stop": api.zones.stop(index).catch(() => {}); break;
      case "next": api.zones.next(index).catch(() => {}); break;
      case "previous": api.zones.previous(index).catch(() => {}); break;
    }
  };

  return (
    <div className="flex items-center justify-center gap-2">
      <motion.div whileTap={tap}>
        <Button variant="ghost" size="icon" disabled={isIdle || isAirPlay || isUrl || !hasNavigation} onClick={() => cmd("previous")} className="size-10 rounded-full" aria-label={t("previous")}>
          <HugeiconsIcon icon={PreviousIcon} size={20} />
        </Button>
      </motion.div>
      <motion.div whileTap={tap}>
        <Button variant="default" size="icon" disabled={isIdle} onClick={() => cmd(isPlaying ? "pause" : "play")} className="size-12 rounded-full" aria-label={isPlaying ? t("pause") : t("play")}>
          <HugeiconsIcon icon={isPlaying ? PauseIcon : PlayIcon} size={24} />
        </Button>
      </motion.div>
      <motion.div whileTap={tap}>
        <Button variant="ghost" size="icon" disabled={isIdle} onClick={() => cmd("stop")} className="size-10 rounded-full" aria-label={t("stop")}>
          <HugeiconsIcon icon={StopIcon} size={20} />
        </Button>
      </motion.div>
      <motion.div whileTap={tap}>
        <Button variant="ghost" size="icon" disabled={isIdle || isAirPlay || isUrl || !hasNavigation} onClick={() => cmd("next")} className="size-10 rounded-full" aria-label={t("next")}>
          <HugeiconsIcon icon={NextIcon} size={20} />
        </Button>
      </motion.div>
    </div>
  );
}
