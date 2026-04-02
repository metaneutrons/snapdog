"use client";

import { HugeiconsIcon } from "@hugeicons/react";
import { ShuffleIcon, RepeatIcon, RepeatOneIcon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import { api } from "@/lib/api";
import type { ZoneState } from "@/stores/useAppStore";

interface ShuffleRepeatProps {
  zone: ZoneState;
}

export function ShuffleRepeat({ zone }: ShuffleRepeatProps) {
  const enabled = zone.source === "subsonic_playlist";

  return (
    <div className="flex items-center justify-center gap-1">
      <Button
        variant="ghost"
        size="icon"
        disabled={!enabled}
        onClick={() => api.zones.toggleShuffle(zone.index).catch(() => {})}
        className={`size-8 rounded-full ${zone.shuffle ? "text-primary" : "text-muted-foreground"}`}
      >
        <HugeiconsIcon icon={ShuffleIcon} size={16} />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        disabled={!enabled}
        onClick={() => api.zones.toggleRepeat(zone.index).catch(() => {})}
        className={`size-8 rounded-full ${zone.repeat ? "text-primary" : "text-muted-foreground"}`}
      >
        <HugeiconsIcon icon={RepeatIcon} size={16} />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        disabled={!enabled}
        onClick={() => api.zones.toggleTrackRepeat(zone.index).catch(() => {})}
        className={`size-8 rounded-full ${zone.track_repeat ? "text-primary" : "text-muted-foreground"}`}
      >
        <HugeiconsIcon icon={RepeatOneIcon} size={16} />
      </Button>
    </div>
  );
}
