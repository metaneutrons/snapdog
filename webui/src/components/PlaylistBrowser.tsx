"use client";

import { useState, useEffect } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { PlayIcon, MusicNote03Icon } from "@hugeicons/core-free-icons";
import { useTranslations } from "next-intl";
import { api } from "@/lib/api";
import type { PlaylistInfo, TrackInfo } from "@/lib/types";
import type { ZoneState } from "@/stores/useAppStore";

interface PlaylistBrowserProps {
  zone: ZoneState;
}

export function PlaylistBrowser({ zone }: PlaylistBrowserProps) {
  const t = useTranslations("playlist");
  const [playlists, setPlaylists] = useState<PlaylistInfo[]>([]);
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const [tracks, setTracks] = useState<TrackInfo[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.media.playlists()
      .then(setPlaylists)
      .catch((e: unknown) => console.error("API error", e))
      .finally(() => setLoading(false));
  }, []);

  const togglePlaylist = async (id: number) => {
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

  const playTrack = (playlistId: number, trackIndex: number) => {
    api.zones.playPlaylist(zone.index, playlistId, trackIndex).catch((e: unknown) => console.error("API error", e));
  };

  if (loading) {
    return <p className="text-sm text-muted-foreground p-4">{t("loading")}</p>;
  }

  if (playlists.length === 0) {
    return <p className="text-sm text-muted-foreground p-4">{t("empty")}</p>;
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
              aria-expanded={expandedId === pl.id}
            >
              <div className="size-8 rounded bg-primary/10 flex items-center justify-center shrink-0 overflow-hidden">
                {pl.cover_art ? (
                  <img
                    src={`/api/v1/media/playlists/${pl.id}/cover`}
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
                  {t("tracks", { count: pl.song_count })}{Number(pl.duration) > 0 ? ` · ${formatDuration(Number(pl.duration))}` : ""}
                </div>
              </div>
            </button>
            <button
              onClick={() => playTrack(pl.id, 0)}
              className="shrink-0 size-8 flex items-center justify-center rounded-full hover:bg-primary/10 transition-colors mr-1"
              title={t("play", { name: pl.name })}
              aria-label={t("play", { name: pl.name })}
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
                  <div className="size-8 rounded bg-primary/10 flex items-center justify-center shrink-0 overflow-hidden">
                    <img
                      src={`/api/v1/media/playlists/${pl.id}/tracks/${i}/cover`}
                      alt=""
                      className="size-full object-cover"
                      onError={(e) => { (e.target as HTMLImageElement).style.display = "none"; }}
                    />
                  </div>
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
