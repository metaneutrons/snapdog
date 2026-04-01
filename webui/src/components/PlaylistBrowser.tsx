"use client";

import { useState, useEffect } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { PlayIcon, MusicNote03Icon } from "@hugeicons/core-free-icons";
import { api } from "@/lib/api";
import type { PlaylistInfo, TrackInfo } from "@/lib/types";
import type { ZoneState } from "@/stores/useAppStore";

interface PlaylistBrowserProps {
  zone: ZoneState;
}

export function PlaylistBrowser({ zone }: PlaylistBrowserProps) {
  const [playlists, setPlaylists] = useState<PlaylistInfo[]>([]);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [tracks, setTracks] = useState<TrackInfo[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.media.playlists()
      .then(setPlaylists)
      .catch(() => {})
      .finally(() => setLoading(false));
  }, []);

  const togglePlaylist = async (id: string) => {
    if (expandedId === id) {
      setExpandedId(null);
      return;
    }
    setExpandedId(id);
    try {
      setTracks(await api.media.tracks(id));
    } catch {
      setTracks([]);
    }
  };

  const playTrack = (trackIndex: number) => {
    api.zones.playTrack(zone.index, trackIndex).catch(() => {});
  };

  if (loading) {
    return <p className="text-sm text-muted-foreground p-4">Loading playlists…</p>;
  }

  if (playlists.length === 0) {
    return <p className="text-sm text-muted-foreground p-4">No playlists available</p>;
  }

  const formatDuration = (sec: number) => {
    const m = Math.floor(sec / 60);
    const s = sec % 60;
    return `${m}:${s.toString().padStart(2, "0")}`;
  };

  return (
    <div className="w-full space-y-1">
      {playlists.map((pl) => (
        <div key={pl.id}>
          <button
            onClick={() => togglePlaylist(pl.id)}
            className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-left transition-colors hover:bg-muted ${
              expandedId === pl.id ? "bg-muted" : ""
            }`}
          >
            <div className="size-8 rounded bg-primary/10 flex items-center justify-center shrink-0">
              <HugeiconsIcon icon={MusicNote03Icon} size={16} className="text-primary" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="text-sm font-medium truncate">{pl.name}</div>
              <div className="text-xs text-muted-foreground">
                {pl.song_count} tracks · {formatDuration(Number(pl.duration))}
              </div>
            </div>
          </button>

          {expandedId === pl.id && (
            <div className="ml-11 border-l border-border pl-3 space-y-0.5 py-1">
              {tracks.map((t, i) => (
                <button
                  key={t.id}
                  onClick={() => playTrack(i)}
                  className="w-full flex items-center gap-2 px-2 py-1.5 rounded text-left hover:bg-muted transition-colors group"
                >
                  <span className="text-xs text-muted-foreground w-5 text-right tabular-nums">
                    {t.track || i + 1}
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="text-sm truncate">{t.title}</div>
                    <div className="text-xs text-muted-foreground truncate">{t.artist}</div>
                  </div>
                  <HugeiconsIcon
                    icon={PlayIcon}
                    size={14}
                    className="shrink-0 opacity-0 group-hover:opacity-100 text-primary transition-opacity"
                  />
                </button>
              ))}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
