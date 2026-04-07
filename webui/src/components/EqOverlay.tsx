"use client";

import { useState, useEffect, useCallback, useMemo } from "react";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { api, type EqBand, type EqConfig } from "@/lib/api";
import { computeResponse } from "@/lib/eq-response";

const FILTER_TYPES = ["low_shelf", "high_shelf", "peaking", "low_pass", "high_pass"] as const;
const PRESETS = ["flat", "bass_boost", "treble_boost", "vocal", "loudness"];

interface EqOverlayProps {
  zoneId: number;
  zoneName: string;
  onClose: () => void;
}

const DEFAULT_BAND: EqBand = { freq: 1000, gain: 0, q: 1.0, type: "peaking" };

export function EqOverlay({ zoneId, zoneName, onClose }: EqOverlayProps) {
  const [config, setConfig] = useState<EqConfig>({ enabled: false, bands: [], preset: "flat" });
  const [abState, setAbState] = useState<EqConfig | null>(null); // stored config for A/B
  const [loading, setLoading] = useState(true);

  // Load current EQ
  useEffect(() => {
    api.eq.get(zoneId).then((c) => { setConfig(c); setLoading(false); }).catch(() => setLoading(false));
  }, [zoneId]);

  // Debounced push to server
  const pushConfig = useCallback(
    (c: EqConfig) => { api.eq.set(zoneId, c).catch(() => {}); },
    [zoneId],
  );

  const update = useCallback(
    (patch: Partial<EqConfig>) => {
      setConfig((prev) => {
        const next = { ...prev, ...patch, preset: null };
        pushConfig(next);
        return next;
      });
    },
    [pushConfig],
  );

  const updateBand = useCallback(
    (idx: number, patch: Partial<EqBand>) => {
      setConfig((prev) => {
        const bands = prev.bands.map((b, i) => (i === idx ? { ...b, ...patch } : b));
        const next = { ...prev, bands, preset: null };
        pushConfig(next);
        return next;
      });
    },
    [pushConfig],
  );

  const addBand = () => {
    if (config.bands.length >= 10) return;
    update({ bands: [...config.bands, { ...DEFAULT_BAND }] });
  };

  const removeBand = (idx: number) => {
    update({ bands: config.bands.filter((_, i) => i !== idx) });
  };

  const applyPreset = (name: string) => {
    api.eq.applyPreset(zoneId, name).then(setConfig).catch(() => {});
  };

  const toggleAB = () => {
    if (abState) {
      // Restore A
      const restored = abState;
      setAbState(null);
      setConfig(restored);
      pushConfig(restored);
    } else {
      // Store A, apply flat (B)
      setAbState(config);
      const flat: EqConfig = { enabled: false, bands: [], preset: "flat" };
      setConfig(flat);
      pushConfig(flat);
    }
  };

  // Frequency response curve
  const response = useMemo(
    () => (config.enabled ? computeResponse(config.bands) : []),
    [config.enabled, config.bands],
  );

  if (loading) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-background/80 backdrop-blur-sm" onClick={onClose} />
      <div className="relative z-10 w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto rounded-2xl border border-border bg-card p-6 shadow-xl space-y-5">
        {/* Header */}
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold">EQ — {zoneName}</h2>
          <div className="flex items-center gap-2">
            <Button variant="ghost" size="sm" onClick={toggleAB} className={abState ? "text-primary" : ""}>
              A/B
            </Button>
            <Button variant="ghost" size="sm" onClick={onClose}>✕</Button>
          </div>
        </div>

        {/* Enable + Preset */}
        <div className="flex items-center gap-3 flex-wrap">
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={config.enabled}
              onChange={(e) => update({ enabled: e.target.checked })}
              className="accent-primary"
            />
            Enabled
          </label>
          <select
            value={config.preset ?? ""}
            onChange={(e) => applyPreset(e.target.value)}
            className="text-sm bg-muted border border-border rounded px-2 py-1"
          >
            <option value="" disabled>Preset…</option>
            {PRESETS.map((p) => (
              <option key={p} value={p}>{p.replace("_", " ")}</option>
            ))}
          </select>
        </div>

        {/* Frequency Response Curve */}
        <FrequencyResponseCurve response={response} />

        {/* Bands */}
        <div className="space-y-3">
          {config.bands.map((band, idx) => (
            <BandRow
              key={idx}
              band={band}
              index={idx}
              onChange={(patch) => updateBand(idx, patch)}
              onRemove={() => removeBand(idx)}
            />
          ))}
        </div>

        {config.bands.length < 10 && (
          <Button variant="ghost" size="sm" onClick={addBand} className="w-full">
            + Add Band
          </Button>
        )}
      </div>
    </div>
  );
}

// ── Band Row ──────────────────────────────────────────────────

function BandRow({
  band,
  index,
  onChange,
  onRemove,
}: {
  band: EqBand;
  index: number;
  onChange: (patch: Partial<EqBand>) => void;
  onRemove: () => void;
}) {
  return (
    <div className="flex items-center gap-2 text-sm">
      <span className="w-5 text-muted-foreground text-xs">{index + 1}</span>
      <select
        value={band.type}
        onChange={(e) => onChange({ type: e.target.value as EqBand["type"] })}
        className="bg-muted border border-border rounded px-1.5 py-1 text-xs w-20"
      >
        {FILTER_TYPES.map((t) => (
          <option key={t} value={t}>{t.replace("_", " ")}</option>
        ))}
      </select>
      <div className="flex-1 space-y-0.5">
        <div className="flex items-center gap-2">
          <span className="w-8 text-xs text-muted-foreground">Hz</span>
          <Slider
            value={[Math.log10(band.freq)]}
            min={Math.log10(20)}
            max={Math.log10(20000)}
            step={0.01}
            onValueChange={([v]) => onChange({ freq: Math.round(Math.pow(10, v)) })}
            className="flex-1"
          />
          <span className="w-12 text-xs tabular-nums text-right">{band.freq >= 1000 ? `${(band.freq / 1000).toFixed(1)}k` : band.freq}</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="w-8 text-xs text-muted-foreground">dB</span>
          <Slider
            value={[band.gain]}
            min={-12}
            max={12}
            step={0.5}
            onValueChange={([v]) => onChange({ gain: v })}
            className="flex-1"
          />
          <span className="w-12 text-xs tabular-nums text-right">{band.gain > 0 ? "+" : ""}{band.gain.toFixed(1)}</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="w-8 text-xs text-muted-foreground">Q</span>
          <Slider
            value={[band.q]}
            min={0.1}
            max={10}
            step={0.1}
            onValueChange={([v]) => onChange({ q: Math.round(v * 10) / 10 })}
            className="flex-1"
          />
          <span className="w-12 text-xs tabular-nums text-right">{band.q.toFixed(1)}</span>
        </div>
      </div>
      <Button variant="ghost" size="sm" onClick={onRemove} className="text-muted-foreground px-1">✕</Button>
    </div>
  );
}

// ── Frequency Response Curve ──────────────────────────────────

function FrequencyResponseCurve({ response }: { response: { freq: number; db: number }[] }) {
  const width = 600;
  const height = 160;
  const pad = { top: 10, right: 10, bottom: 20, left: 35 };
  const plotW = width - pad.left - pad.right;
  const plotH = height - pad.top - pad.bottom;
  const dbRange = 15; // ±15 dB

  const freqToX = (f: number) => pad.left + ((Math.log10(f) - Math.log10(20)) / (Math.log10(20000) - Math.log10(20))) * plotW;
  const dbToY = (db: number) => pad.top + plotH / 2 - (db / dbRange) * (plotH / 2);

  const path = response.length > 0
    ? response.map((p, i) => `${i === 0 ? "M" : "L"}${freqToX(p.freq).toFixed(1)},${dbToY(p.db).toFixed(1)}`).join("")
    : "";

  const gridFreqs = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
  const gridDbs = [-12, -6, 0, 6, 12];

  return (
    <svg viewBox={`0 0 ${width} ${height}`} className="w-full rounded-lg bg-muted/30 border border-border">
      {/* Grid lines */}
      {gridFreqs.map((f) => (
        <line key={`f${f}`} x1={freqToX(f)} x2={freqToX(f)} y1={pad.top} y2={pad.top + plotH} stroke="currentColor" strokeOpacity={0.1} />
      ))}
      {gridDbs.map((db) => (
        <g key={`db${db}`}>
          <line x1={pad.left} x2={pad.left + plotW} y1={dbToY(db)} y2={dbToY(db)} stroke="currentColor" strokeOpacity={db === 0 ? 0.3 : 0.1} />
          <text x={pad.left - 4} y={dbToY(db) + 3} textAnchor="end" fontSize={9} fill="currentColor" opacity={0.4}>{db}</text>
        </g>
      ))}
      {/* Freq labels */}
      {[100, 1000, 10000].map((f) => (
        <text key={`fl${f}`} x={freqToX(f)} y={height - 4} textAnchor="middle" fontSize={9} fill="currentColor" opacity={0.4}>
          {f >= 1000 ? `${f / 1000}k` : f}
        </text>
      ))}
      {/* Response curve */}
      {path && (
        <>
          <path d={path + `L${freqToX(20000)},${dbToY(0)}L${freqToX(20)},${dbToY(0)}Z`} fill="hsl(var(--primary))" fillOpacity={0.1} />
          <path d={path} fill="none" stroke="hsl(var(--primary))" strokeWidth={2} />
        </>
      )}
      {/* 0 dB label */}
      <text x={pad.left - 4} y={dbToY(0) + 3} textAnchor="end" fontSize={9} fill="currentColor" opacity={0.6} fontWeight="bold">0</text>
    </svg>
  );
}
