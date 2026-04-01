"use client";

import { useState } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { VolumeLowIcon, VolumeMute02Icon, ArrowDown01Icon } from "@hugeicons/core-free-icons";
import { Slider } from "@/components/ui/slider";
import { Button } from "@/components/ui/button";
import { api } from "@/lib/api";
import { useAppStore, type ZoneState } from "@/stores/useAppStore";
import type { ClientInfo } from "@/lib/types";

function ClientCard({ client, zoneList }: { client: ClientInfo; zoneList: { index: number; name: string }[] }) {
  const [showZoneSelect, setShowZoneSelect] = useState(false);

  return (
    <div className="flex items-center gap-3 px-3 py-2">
      <div className="relative shrink-0">
        <span className="text-lg">{client.icon || "🔊"}</span>
        <div className={`absolute -bottom-0.5 -right-0.5 size-2 rounded-full ${client.connected ? "bg-green-500" : "bg-destructive"}`} />
      </div>
      <div className="min-w-0 flex-1 space-y-1">
        <div className="flex items-center justify-between">
          <span className="text-sm font-medium truncate">{client.name}</span>
          <Button
            variant="ghost"
            size="icon"
            className="size-6 rounded-full shrink-0"
            onClick={() => api.clients.toggleMute(client.index).catch(() => {})}
          >
            <HugeiconsIcon icon={client.muted ? VolumeMute02Icon : VolumeLowIcon} size={14} />
          </Button>
        </div>
        <Slider
          value={[client.muted ? 0 : client.volume]}
          max={100}
          step={1}
          onValueChange={(v) => api.clients.setVolume(client.index, v[0]).catch(() => {})}
          className="w-full"
        />
        <div className="relative">
          <button
            onClick={() => setShowZoneSelect(!showZoneSelect)}
            className="text-[10px] text-muted-foreground flex items-center gap-0.5 hover:text-foreground transition-colors"
          >
            Zone: {zoneList.find((z) => z.index === client.zone_index)?.name ?? "?"}
            <HugeiconsIcon icon={ArrowDown01Icon} size={10} />
          </button>
          {showZoneSelect && (
            <div className="absolute top-5 left-0 z-10 bg-popover border border-border rounded-md shadow-md py-1 min-w-32">
              {zoneList.map((z) => (
                <button
                  key={z.index}
                  onClick={() => {
                    api.clients.setZone(client.index, z.index).catch(() => {});
                    setShowZoneSelect(false);
                  }}
                  className={`w-full text-left px-3 py-1 text-xs hover:bg-muted transition-colors ${
                    z.index === client.zone_index ? "text-primary font-medium" : ""
                  }`}
                >
                  {z.name}
                </button>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export function ClientList({ zone }: { zone: ZoneState }) {
  const [expanded, setExpanded] = useState(false);
  const clients = useAppStore((s) => s.clients);
  const zones = useAppStore((s) => s.zones);

  const zoneClients = Array.from(clients.values()).filter((c) => c.zone_index === zone.index);
  const zoneList = Array.from(zones.values()).map((z) => ({ index: z.index, name: z.name }));

  if (zoneClients.length === 0) return null;

  return (
    <div className="w-full max-w-xs">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center justify-between px-3 py-2 text-xs text-muted-foreground hover:text-foreground transition-colors"
      >
        <span>{zoneClients.length} client{zoneClients.length !== 1 ? "s" : ""}</span>
        <HugeiconsIcon icon={ArrowDown01Icon} size={12} className={`transition-transform ${expanded ? "rotate-180" : ""}`} />
      </button>
      {expanded && (
        <div className="space-y-1 border-t border-border pt-1">
          {zoneClients.map((c) => (
            <ClientCard key={c.index} client={c} zoneList={zoneList} />
          ))}
        </div>
      )}
    </div>
  );
}
