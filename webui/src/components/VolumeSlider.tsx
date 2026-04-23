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
  /** Compact mode for client chips (smaller controls, no value display) */
  compact?: boolean;
}

export function VolumeSlider({
  volume,
  muted,
  onVolumeChange,
  onMuteToggle,
  onUnmute,
  compact = false,
}: VolumeSliderProps) {
  const [localVolume, setLocalVolume] = useState(volume);
  const t = useTranslations("volume");
  const [dragging, setDragging] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  // Clean up debounce timer on unmount
  useEffect(() => () => clearTimeout(timerRef.current), []);

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
      <Slider
        value={[muted ? 0 : localVolume]}
        max={100}
        step={1}
        onValueChange={handleChange}
        onValueCommit={handleCommit}
        onDragStart={(e: React.DragEvent) => e.preventDefault()}
        className="flex-1 min-w-0"
        aria-label={t("label")}
      />
      <span className={`text-muted-foreground tabular-nums text-right ${compact ? "text-[10px] w-5" : "text-xs w-7"}`}>
        {localVolume}
      </span>
    </div>
  );
}
