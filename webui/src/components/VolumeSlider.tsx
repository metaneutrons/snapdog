"use client";

import { useState, useEffect, useRef, useCallback } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  VolumeHighIcon,
  VolumeLowIcon,
  VolumeMute02Icon,
} from "@hugeicons/core-free-icons";
import { useTranslations } from "next-intl";
import { Slider } from "@/components/ui/slider";
import { Button } from "@/components/ui/button";

interface VolumeSliderProps {
  volume: number;
  muted: boolean;
  onVolumeChange: (volume: number) => void;
  onMuteToggle: () => void;
  onUnmute: () => void;
  /** Maximum volume limit (0–100). Shows a red marker and caps the slider. */
  max?: number;
  /** Compact mode for client chips (smaller controls, no value display) */
  compact?: boolean;
}

export function VolumeSlider({
  volume,
  muted,
  onVolumeChange,
  onMuteToggle,
  onUnmute,
  max = 100,
  compact = false,
}: VolumeSliderProps) {
  const [localVolume, setLocalVolume] = useState(volume);
  const t = useTranslations("volume");
  const [dragging, setDragging] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => {
    if (!dragging) setLocalVolume(volume);
  }, [volume, dragging]);

  const volumeIcon = muted
    ? VolumeMute02Icon
    : localVolume > 50
      ? VolumeHighIcon
      : VolumeLowIcon;

  const handleChange = useCallback(
    (value: number[]) => {
      const v = value[0];
      setLocalVolume(v);
      setDragging(true);
      if (muted) onUnmute();
      clearTimeout(timerRef.current);
      timerRef.current = setTimeout(() => onVolumeChange(v), 50);
    },
    [muted, onVolumeChange, onUnmute],
  );

  const handleCommit = useCallback(
    (value: number[]) => {
      setDragging(false);
      clearTimeout(timerRef.current);
      onVolumeChange(value[0]);
    },
    [onVolumeChange],
  );

  const iconSize = compact ? 14 : 18;
  const btnSize = compact ? "size-6" : "size-8";

  return (
    <div
      className={`flex items-center gap-${compact ? "1.5" : "3"} w-full ${compact ? "" : "sm:max-w-xs"}`}
    >
      <Button
        variant="ghost"
        size="icon"
        onClick={onMuteToggle}
        onDragStart={(e) => e.preventDefault()}
        className={`${btnSize} shrink-0 rounded-full`}
        aria-label={muted ? t("unmute") : t("mute")}
      >
        <HugeiconsIcon icon={volumeIcon} size={iconSize} />
      </Button>
      <div className="relative flex-1 min-w-0">
        <Slider
          value={[muted ? 0 : localVolume]}
          max={max}
          step={1}
          onValueChange={handleChange}
          onValueCommit={handleCommit}
          onDragStart={(e: React.DragEvent) => e.preventDefault()}
          className="flex-1 min-w-0"
          aria-label={t("label")}
        />
        {max < 100 && (
          <div
            className="absolute top-0 h-full w-0.5 bg-red-500/70 rounded-full pointer-events-none"
            style={{ left: `${max}%` }}
            title={`Max: ${max}%`}
          />
        )}
      </div>
      <span className={`text-muted-foreground tabular-nums text-right ${compact ? "text-[10px] w-5" : "text-xs w-7"}`}>
        {localVolume}
      </span>
    </div>
  );
}
