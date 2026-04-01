"use client";

import type { ZoneState } from "@/stores/useAppStore";

export function RadioStations({ zone }: { zone: ZoneState }) {
  if (zone.source !== "radio") return null;

  const stationName = zone.track?.title || `Station ${zone.track?.radio_index ?? "?"}`;

  return (
    <div className="text-center">
      <p className="text-xs uppercase tracking-wider text-muted-foreground">Now on air</p>
      <p className="text-sm font-medium mt-0.5">{stationName}</p>
    </div>
  );
}
