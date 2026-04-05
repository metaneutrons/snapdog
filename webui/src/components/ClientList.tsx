"use client";

import { useState } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { ArrowDown01Icon, DragDropVerticalIcon } from "@hugeicons/core-free-icons";
import { api } from "@/lib/api";
import { useAppStore, type ZoneState } from "@/stores/useAppStore";
import type { ClientInfo } from "@/lib/types";
import { VolumeSlider } from "@/components/VolumeSlider";

function ClientCard({ client }: { client: ClientInfo }) {

  return (
    <div
      className="flex items-stretch gap-2 px-3 py-2.5 rounded-lg bg-muted shadow-[inset_0_2px_4px_rgba(0,0,0,0.15)] border border-border/50 cursor-grab active:cursor-grabbing active:opacity-50 active:shadow-lg hover:border-primary/30 transition-all"
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData("application/x-snapdog-client", String(client.index));
        e.dataTransfer.effectAllowed = "move";
      }}
    >
      {/* Drag handle — full height grip */}
      <div className="shrink-0 flex items-center text-muted-foreground/30">
        <div className="flex flex-col gap-[3px]">
          <div className="flex gap-[3px]"><div className="size-[3px] rounded-full bg-current" /><div className="size-[3px] rounded-full bg-current" /></div>
          <div className="flex gap-[3px]"><div className="size-[3px] rounded-full bg-current" /><div className="size-[3px] rounded-full bg-current" /></div>
          <div className="flex gap-[3px]"><div className="size-[3px] rounded-full bg-current" /><div className="size-[3px] rounded-full bg-current" /></div>
          <div className="flex gap-[3px]"><div className="size-[3px] rounded-full bg-current" /><div className="size-[3px] rounded-full bg-current" /></div>
        </div>
      </div>
      <div className="min-w-0 flex-1 space-y-1.5">
        {/* Name row: icon + connection indicator + name */}
        <div className="flex items-center gap-1.5">
          <span className="text-lg shrink-0">{client.icon || "🔊"}</span>
          <div className={`size-2 rounded-full shrink-0 ${client.connected ? "bg-green-500" : "bg-destructive"}`} />
          <span className="text-sm font-medium truncate">{client.name}</span>
        </div>
        {/* Volume */}
        <VolumeSlider
          volume={client.volume}
          muted={client.muted}
          onVolumeChange={(v) => api.clients.setVolume(client.index, v).catch(() => {})}
          onMuteToggle={() => api.clients.toggleMute(client.index).catch(() => {})}
          onUnmute={() => api.clients.setMute(client.index, false).catch(() => {})}
          compact
        />
      </div>
    </div>
  );
}

export function ClientList({ zone }: { zone: ZoneState }) {
  const [expanded, setExpanded] = useState(() => typeof window !== "undefined" && window.innerWidth >= 768);
  const clients = useAppStore((s) => s.clients);

  const zoneClients = Array.from(clients.values()).filter((c) => c.zone_index === zone.index);

  if (zoneClients.length === 0) return null;

  return (
    <div className="w-full">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center justify-between px-3 py-2 text-xs text-muted-foreground hover:text-foreground transition-colors"
      >
        <span>{zoneClients.length} client{zoneClients.length !== 1 ? "s" : ""}</span>
        <HugeiconsIcon icon={ArrowDown01Icon} size={12} className={`transition-transform ${expanded ? "rotate-180" : ""}`} />
      </button>
      {expanded && (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(14rem,1fr))] gap-1 border-t border-border pt-1">
          {zoneClients.map((c) => (
            <div key={c.index}>
              <ClientCard client={c} />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
