"use client";

import { useState, useEffect, useCallback, useMemo } from "react";
import { useTranslations } from "next-intl";
import { useFocusTrap } from "@/hooks/useFocusTrap";
import { Button } from "@/components/ui/button";
import { Slider } from "@/components/ui/slider";
import { api, type EqBand, type EqConfig } from "@/lib/api";
import { logApiError } from "@/lib/log-api-error";
import { computeResponse } from "@/lib/eq-response";

const FILTER_TYPES: readonly EqBand["type"][] = ["low_shelf", "high_shelf", "peaking", "low_pass", "high_pass"] as const;
const PRESETS = ['flat', 'bass_boost', 'treble_boost', 'vocal', 'rock', 'jazz', 'classical', 'electronic', 'loudness', 'late_night'] as const;

const MAX_EQ_BANDS = 10;
const FREQ_MIN_HZ = 20;
const FREQ_MAX_HZ = 20000;
const GAIN_MIN_DB = -12;
const GAIN_MAX_DB = 12;
const GAIN_STEP_DB = 0.5;
const Q_MIN = 0.1;
const Q_MAX = 10;
const Q_STEP = 0.1;
const CURVE_WIDTH = 600;
const CURVE_HEIGHT = 160;
const CURVE_DB_RANGE = 15;
const CURVE_MIN_HEIGHT = 120;

const PRESET_LABELS: Record<string, string> = {
  flat: 'Flat',
  bass_boost: 'Bass Boost',
  treble_boost: 'Treble Boost',
  vocal: 'Vocal',
  rock: 'Rock',
  jazz: 'Jazz',
  classical: 'Classical',
  electronic: 'Electronic',
  loudness: 'Loudness',
  late_night: 'Late Night',
};

interface EqOverlayProps {
  zoneId?: number;
  clientId?: number;
  label: string;
  onClose: (enabled: boolean) => void;
}

const DEFAULT_BAND: EqBand = { freq: 1000, gain: 0, q: 1.0, type: "peaking" };

export function EqOverlay({ zoneId, clientId, label, onClose }: EqOverlayProps) {
  const t = useTranslations("eq");
  const trapRef = useFocusTrap<HTMLDivElement>();
  const [config, setConfig] = useState<EqConfig>({ enabled: false, bands: [], preset: "flat" });
  const [abBypass, setAbBypass] = useState(false);
  const [loading, setLoading] = useState(true);

  const eqApi = useMemo(() => clientId
    ? { get: () => api.clientEq.get(clientId), set: (c: EqConfig) => api.clientEq.set(clientId, c), applyPreset: (n: string) => api.clientEq.applyPreset(clientId, n) }
    : { get: () => api.eq.get(zoneId!), set: (c: EqConfig) => api.eq.set(zoneId!, c), applyPreset: (n: string) => api.eq.applyPreset(zoneId!, n) },
    [zoneId, clientId]);

  // Load current EQ
  useEffect(() => {
    eqApi.get().then((c) => { setConfig(c); setLoading(false); }).catch(() => setLoading(false));
  }, [eqApi]);

  // Push to server
  const pushConfig = useCallback(
    (c: EqConfig) => { eqApi.set(c).catch(logApiError); },
    [eqApi],
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
    if (config.bands.length >= MAX_EQ_BANDS) return;
    update({ bands: [...config.bands, { ...DEFAULT_BAND }] });
  };

  const removeBand = (idx: number) => {
    update({ bands: config.bands.filter((_, i) => i !== idx) });
  };

  const applyPreset = (name: string) => {
    eqApi.applyPreset(name).then(setConfig).catch(logApiError);
  };

  // Off: send only enabled:false (bands stay persisted on server), clear UI
  // On: reload from server (which has the bands), then enable
  const toggleEnabled = (on: boolean) => {
    if (on) {
      eqApi.get().then((c) => {
        const next = { ...c, enabled: true };
        setConfig(next);
        pushConfig(next);
      }).catch(logApiError);
    } else {
      eqApi.set({ ...config, enabled: false }).catch(logApiError);
      setConfig((prev) => ({ ...prev, enabled: false }));
    }
  };

  // A/B: temporarily disable EQ, restore on deactivate
  const toggleAB = () => {
    const next = !abBypass;
    setAbBypass(next);
    pushConfig({ ...config, enabled: !next });
  };

  const handleClose = () => {
    if (abBypass) {
      pushConfig({ ...config, enabled: true });
    }
    onClose(config.enabled);
  };

  // Frequency response curve
  const response = useMemo(
    () => (config.bands.length > 0 ? computeResponse(config.bands) : []),
    [config.bands],
  );

  if (loading) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" role="dialog" aria-modal="true" aria-label={t("title", { zone: label })} onKeyDown={(e) => { if (e.key === "Escape") handleClose(); }}>
      <div className="absolute inset-0 bg-background/80 backdrop-blur-sm" onClick={handleClose} role="presentation" />
      <div className="relative z-10 w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto rounded-2xl border border-border bg-card p-6 shadow-xl space-y-5" ref={trapRef}>
        {/* Header: title + On/Off toggle (left), A/B + close (right) */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <h2 className="text-lg font-semibold">{t("title", { zone: label })}</h2>
            <div className="inline-flex rounded-lg bg-muted p-0.5" role="radiogroup" aria-label={t("toggle")}>
              <button role="radio" aria-checked={!config.enabled} className={`px-3 py-1 text-xs rounded-md transition-colors ${!config.enabled ? 'bg-background shadow-sm font-medium' : 'text-muted-foreground'}`} onClick={() => toggleEnabled(false)}>Off</button>
              <button role="radio" aria-checked={config.enabled} className={`px-3 py-1 text-xs rounded-md transition-colors ${config.enabled ? 'bg-background shadow-sm font-medium' : 'text-muted-foreground'}`} onClick={() => toggleEnabled(true)}>On</button>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="ghost" size="sm" onClick={toggleAB} disabled={!config.enabled} className={abBypass ? "text-orange-500 font-semibold" : "text-muted-foreground"} aria-pressed={abBypass}>
              A/B
            </Button>
            <Button variant="ghost" size="sm" onClick={handleClose} aria-label={t("close")}>✕</Button>
          </div>
        </div>

        {/* Curve + Presets + Bands — curve always visible, rest hidden when off, all dimmed when A/B */}
        {config.enabled ? (
          <div className={`space-y-5 transition-opacity ${abBypass ? 'opacity-50 pointer-events-none' : ''}`}>
            <FrequencyResponseCurve response={response} curveLabel={t("curve")} />
            {/* Preset chips */}
            <div className="flex gap-1.5 overflow-x-auto scrollbar-none py-1 -mx-1 px-1" role="radiogroup" aria-label={t("presets")}>
              {PRESETS.map((p) => (
                <button
                  key={p}
                  onClick={() => applyPreset(p)}
                  role="radio"
                  aria-checked={config.preset === p}
                  className={`shrink-0 px-3 py-1 text-xs rounded-full transition-colors ${
                    config.preset === p
                      ? 'bg-primary text-primary-foreground'
                      : 'bg-muted hover:bg-muted/80 text-foreground'
                  }`}
                >
                  {PRESET_LABELS[p] || p}
                </button>
              ))}
            </div>

            {/* Band rows */}
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

            {config.bands.length < MAX_EQ_BANDS && (
              <Button variant="ghost" size="sm" onClick={addBand} className="w-full">
                {t("addBand")}
              </Button>
            )}
          </div>
        ) : (
          <FrequencyResponseCurve response={[]} curveLabel={t("curve")} />
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
  const t = useTranslations("eq");
  return (
    <div className="flex items-center gap-2 text-sm">
      <span className="w-5 text-muted-foreground text-xs">{index + 1}</span>
      <select
        value={band.type}
        onChange={(e) => onChange({ type: e.target.value as EqBand["type"] })}
        className="bg-muted border border-border rounded px-1.5 py-1 text-xs w-20"
        aria-label={t("filterType", { n: index + 1 })}
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
            min={Math.log10(FREQ_MIN_HZ)}
            max={Math.log10(FREQ_MAX_HZ)}
            step={0.01}
            onValueChange={([v]) => onChange({ freq: Math.round(Math.pow(10, v)) })}
            className="flex-1"
            aria-label={t("frequency")}
          />
          <span className="w-12 text-xs tabular-nums text-right">{band.freq >= 1000 ? `${(band.freq / 1000).toFixed(1)}k` : band.freq}</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="w-8 text-xs text-muted-foreground">dB</span>
          <Slider
            value={[band.gain]}
            min={GAIN_MIN_DB}
            max={GAIN_MAX_DB}
            step={GAIN_STEP_DB}
            onValueChange={([v]) => onChange({ gain: v })}
            className="flex-1"
            aria-label={t("gain")}
          />
          <span className="w-12 text-xs tabular-nums text-right">{band.gain > 0 ? "+" : ""}{band.gain.toFixed(1)}</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="w-8 text-xs text-muted-foreground">Q</span>
          <Slider
            value={[band.q]}
            min={Q_MIN}
            max={Q_MAX}
            step={Q_STEP}
            onValueChange={([v]) => onChange({ q: Math.round(v * 10) / 10 })}
            className="flex-1"
            aria-label={t("qFactor")}
          />
          <span className="w-12 text-xs tabular-nums text-right">{band.q.toFixed(1)}</span>
        </div>
      </div>
      <Button variant="ghost" size="sm" onClick={onRemove} className="text-muted-foreground px-1" aria-label={t("removeBand")}>✕</Button>
    </div>
  );
}

// ── Frequency Response Curve ──────────────────────────────────

function FrequencyResponseCurve({ response, curveLabel }: { response: { freq: number; db: number }[]; curveLabel: string }) {
  const width = CURVE_WIDTH;
  const height = CURVE_HEIGHT;
  const pad = { top: 10, right: 10, bottom: 20, left: 35 };
  const plotW = width - pad.left - pad.right;
  const plotH = height - pad.top - pad.bottom;
  const dbRange = CURVE_DB_RANGE;

  const freqToX = (f: number) => pad.left + ((Math.log10(f) - Math.log10(FREQ_MIN_HZ)) / (Math.log10(FREQ_MAX_HZ) - Math.log10(FREQ_MIN_HZ))) * plotW;
  const dbToY = (db: number) => pad.top + plotH / 2 - (db / dbRange) * (plotH / 2);

  const path = response.length > 0
    ? response.map((p, i) => `${i === 0 ? "M" : "L"}${freqToX(p.freq).toFixed(1)},${dbToY(p.db).toFixed(1)}`).join("")
    : "";

  if (response.length > 0 && !path) {
    console.warn("EQ: response has data but path is empty", response);
  }

  const gridFreqs = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
  const gridDbs = [-12, -6, 0, 6, 12];

  return (
    <svg viewBox={`0 0 ${width} ${height}`} className="w-full rounded-lg bg-muted/30 border border-border" style={{ minHeight: CURVE_MIN_HEIGHT }} role="img" aria-label={curveLabel}>
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
          <path d={path + `L${freqToX(FREQ_MAX_HZ)},${dbToY(0)}L${freqToX(FREQ_MIN_HZ)},${dbToY(0)}Z`} fill="oklch(0.65 0.18 40)" fillOpacity={0.15} />
          <path d={path} fill="none" stroke="oklch(0.65 0.18 40)" strokeWidth={2} />
        </>
      )}
      {/* 0 dB label */}
      <text x={pad.left - 4} y={dbToY(0) + 3} textAnchor="end" fontSize={9} fill="currentColor" opacity={0.6} fontWeight="bold">0</text>
    </svg>
  );
}
