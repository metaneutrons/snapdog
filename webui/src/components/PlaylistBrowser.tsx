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

  const playTrack = (playlistId: string, trackIndex: number) => {
    api.zones.playPlaylist(zone.index, playlistId, trackIndex).catch(() => {});
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
    <div className="w-full space-y-1 xl:max-h-48 xl:overflow-y-auto">
      {playlists.map((pl) => (
        <div key={pl.id}>
          <div className={`flex items-center gap-1 rounded-lg transition-colors ${
            expandedId === pl.id ? "bg-muted" : ""
          }`}>
            <button
              onClick={() => togglePlaylist(pl.id)}
              className="flex-1 flex items-center gap-3 px-3 py-2.5 text-left hover:bg-muted rounded-lg"
            >
              <div className="size-8 rounded bg-primary/10 flex items-center justify-center shrink-0 overflow-hidden">
                {pl.cover_art ? (
                  <img
                    src={`/api/v1/media/playlists/${pl.cover_art}/cover`}
                    alt=""
                    className="size-full object-cover"
                    onError={(e) => { (e.target as HTMLImageElement).style.display = "none"; }}
                  />
                ) : (
                  <HugeiconsIcon icon={MusicNote03Icon} size={16} className="text-primary" />
                )}
              </div>
              <div className="min-w-0 flex-1">
                <div className="text-sm font-medium truncate">{pl.name}</div>
                <div className="text-xs text-muted-foreground">
                  {pl.song_count} tracks · {formatDuration(Number(pl.duration))}
                </div>
              </div>
            </button>
            <button
              onClick={() => playTrack(pl.id, 0)}
              className="shrink-0 size-8 flex items-center justify-center rounded-full hover:bg-primary/10 transition-colors mr-1"
              title={`Play ${pl.name}`}
            >
              <HugeiconsIcon icon={PlayIcon} size={16} className="text-primary" />
            </button>
          </div>

          {expandedId === pl.id && (
            <div className="ml-11 border-l border-border pl-3 space-y-0.5 py-1">
              {tracks.map((t, i) => (
                <button
                  key={t.id}
                  onClick={() => playTrack(pl.id, i)}
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
