"use client";

import { useState, useEffect, useMemo } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { PlayIcon, MusicNote03Icon, Search01Icon } from "@hugeicons/core-free-icons";
import { useTranslations } from "next-intl";
import { api } from "@/lib/api";
import { logApiError } from "@/lib/log-api-error";
import { formatTime } from "@/lib/format-time";
import type { PlaylistInfo, TrackInfo } from "@/lib/types";
import type { ZoneState } from "@/stores/useAppStore";
import { Input } from "@/components/ui/input";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

interface PlaylistBrowserProps {
  zone: ZoneState;
}

export function PlaylistBrowser({ zone }: PlaylistBrowserProps) {
  const t = useTranslations("playlist");
  const [playlists, setPlaylists] = useState<PlaylistInfo[]>([]);
  const [search, setSearch] = useState("");
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const [tracks, setTracks] = useState<TrackInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (!open) return;
    api.media.playlists()
      .then(setPlaylists)
      .catch(logApiError)
      .finally(() => setLoading(false));
  }, [open]);

  const filteredPlaylists = useMemo(() => {
    if (!search) return playlists;
    const q = search.toLowerCase();
    return playlists.filter((pl) => pl.name.toLowerCase().includes(q));
  }, [playlists, search]);

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
    api.zones.playPlaylist(zone.index, playlistId, trackIndex).catch(logApiError);
    setOpen(false);
  };

  const formatDuration = (sec: number) => formatTime(sec * 1000);

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="outline" className="w-full justify-start gap-2 h-12 rounded-xl text-muted-foreground hover:text-foreground border-dashed">
          <HugeiconsIcon icon={MusicNote03Icon} size={18} />
          <span>{t("browse")}</span>
        </Button>
      </DialogTrigger>
      <DialogContent className="max-w-3xl h-[80vh] flex flex-col p-0 gap-0">
        <DialogHeader className="p-4 border-b border-border">
          <DialogTitle>{t("title")}</DialogTitle>
          <div className="relative mt-2">
            <HugeiconsIcon icon={Search01Icon} size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
            <Input
              placeholder={t("search")}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="pl-9 bg-muted/50 border-none"
            />
          </div>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto p-4">
          {loading ? (
            <div className="flex items-center justify-center h-32 text-muted-foreground animate-pulse">
              {t("loading")}
            </div>
          ) : filteredPlaylists.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-64 text-muted-foreground gap-2">
              <HugeiconsIcon icon={MusicNote03Icon} size={48} className="opacity-20" />
              <p>{search ? t("noResults") : t("empty")}</p>
            </div>
          ) : (
            <div className="grid grid-cols-[repeat(auto-fill,minmax(140px,1fr))] gap-4">
              {filteredPlaylists.map((pl) => (
                <div key={pl.id} className="group relative">
                  <button
                    onClick={() => togglePlaylist(pl.id)}
                    className="w-full space-y-2 text-left"
                  >
                    <div className="aspect-square rounded-xl bg-primary/5 flex items-center justify-center overflow-hidden border border-border/50 group-hover:border-primary/30 transition-colors shadow-sm">
                      {pl.cover_art ? (
                        <img
                          src={`/api/v1/media/playlists/${pl.id}/cover`}
                          alt=""
                          loading="lazy"
                          className="size-full object-cover transition-transform group-hover:scale-105"
                        />
                      ) : (
                        <HugeiconsIcon icon={MusicNote03Icon} size={32} className="text-primary/40" />
                      )}
                      <div className="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center gap-2">
                        <Button
                          size="icon"
                          variant="secondary"
                          className="rounded-full size-10"
                          onClick={(e) => {
                            e.stopPropagation();
                            playTrack(pl.id, 0);
                          }}
                        >
                          <HugeiconsIcon icon={PlayIcon} size={20} fill="currentColor" />
                        </Button>
                      </div>
                    </div>
                    <div className="min-w-0 px-1">
                      <div className="text-sm font-semibold truncate group-hover:text-primary transition-colors">{pl.name}</div>
                      <div className="text-[10px] text-muted-foreground uppercase tracking-wider">
                        {t("tracks", { count: pl.song_count })}
                      </div>
                    </div>
                  </button>

                  {expandedId === pl.id && (
                    <div className="fixed inset-0 z-[60] flex items-center justify-center p-4 bg-background/60 backdrop-blur-sm" onClick={() => setExpandedId(null)}>
                      <div
                        className="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-md max-h-[70vh] flex flex-col overflow-hidden"
                        onClick={(e) => e.stopPropagation()}
                      >
                        <div className="p-4 border-b border-border flex items-center gap-3">
                           <div className="size-12 rounded-lg overflow-hidden bg-primary/10">
                             <img src={`/api/v1/media/playlists/${pl.id}/cover`} alt="" className="size-full object-cover" />
                           </div>
                           <div className="min-w-0 flex-1">
                             <h3 className="font-bold truncate">{pl.name}</h3>
                             <p className="text-xs text-muted-foreground uppercase tracking-wider">{t("tracks", { count: pl.song_count })}</p>
                           </div>
                           <Button variant="ghost" size="icon" onClick={() => setExpandedId(null)}>✕</Button>
                        </div>
                        <div className="flex-1 overflow-y-auto p-2 space-y-1">
                          {tracks.map((t, i) => (
                            <button
                              key={t.id}
                              onClick={() => playTrack(pl.id, i)}
                              className="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-left hover:bg-muted transition-colors group"
                            >
                              <div className="size-10 rounded bg-primary/10 flex items-center justify-center shrink-0 overflow-hidden relative">
                                <img
                                  src={`/api/v1/media/playlists/${pl.id}/tracks/${i}/cover`}
                                  alt=""
                                  loading="lazy"
                                  className="size-full object-cover"
                                />
                                <div className="absolute inset-0 bg-primary/20 opacity-0 group-hover:opacity-100 flex items-center justify-center">
                                  <HugeiconsIcon icon={PlayIcon} size={16} fill="currentColor" className="text-white" />
                                </div>
                              </div>
                              <div className="min-w-0 flex-1">
                                <div className="text-sm font-medium truncate">{t.title}</div>
                                <div className="text-xs text-muted-foreground truncate">{t.artist}</div>
                              </div>
                              <span className="text-[10px] tabular-nums text-muted-foreground">{formatDuration(t.duration)}</span>
                            </button>
                          ))}
                        </div>
                      </div>
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
