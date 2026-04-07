// Biquad transfer function computation for frequency response visualization.
// Computes magnitude response from biquad coefficients at given frequencies.

import type { EqBand } from "@/lib/api";

/** Compute biquad coefficients from band parameters (Audio EQ Cookbook). */
function bandToCoeffs(band: EqBand, sampleRate: number) {
  const { freq, gain, q, type: filterType } = band;
  const w0 = (2 * Math.PI * freq) / sampleRate;
  const cosW0 = Math.cos(w0);
  const sinW0 = Math.sin(w0);
  const alpha = sinW0 / (2 * q);
  const A = Math.pow(10, gain / 40); // sqrt of linear gain

  let b0: number, b1: number, b2: number, a0: number, a1: number, a2: number;

  switch (filterType) {
    case "peaking":
      b0 = 1 + alpha * A;
      b1 = -2 * cosW0;
      b2 = 1 - alpha * A;
      a0 = 1 + alpha / A;
      a1 = -2 * cosW0;
      a2 = 1 - alpha / A;
      break;
    case "low_shelf": {
      const sq = 2 * Math.sqrt(A) * alpha;
      b0 = A * (A + 1 - (A - 1) * cosW0 + sq);
      b1 = 2 * A * (A - 1 - (A + 1) * cosW0);
      b2 = A * (A + 1 - (A - 1) * cosW0 - sq);
      a0 = A + 1 + (A - 1) * cosW0 + sq;
      a1 = -2 * (A - 1 + (A + 1) * cosW0);
      a2 = A + 1 + (A - 1) * cosW0 - sq;
      break;
    }
    case "high_shelf": {
      const sq = 2 * Math.sqrt(A) * alpha;
      b0 = A * (A + 1 + (A - 1) * cosW0 + sq);
      b1 = -2 * A * (A - 1 + (A + 1) * cosW0);
      b2 = A * (A + 1 + (A - 1) * cosW0 - sq);
      a0 = A + 1 - (A - 1) * cosW0 + sq;
      a1 = 2 * (A - 1 - (A + 1) * cosW0);
      a2 = A + 1 - (A - 1) * cosW0 - sq;
      break;
    }
    case "low_pass":
      b0 = (1 - cosW0) / 2;
      b1 = 1 - cosW0;
      b2 = (1 - cosW0) / 2;
      a0 = 1 + alpha;
      a1 = -2 * cosW0;
      a2 = 1 - alpha;
      break;
    case "high_pass":
      b0 = (1 + cosW0) / 2;
      b1 = -(1 + cosW0);
      b2 = (1 + cosW0) / 2;
      a0 = 1 + alpha;
      a1 = -2 * cosW0;
      a2 = 1 - alpha;
      break;
  }

  return { b0: b0! / a0!, b1: b1! / a0!, b2: b2! / a0!, a1: a1! / a0!, a2: a2! / a0! };
}

/** Compute magnitude in dB at a given frequency for one biquad. */
function magnitudeAt(coeffs: ReturnType<typeof bandToCoeffs>, freq: number, sampleRate: number): number {
  const w = (2 * Math.PI * freq) / sampleRate;
  const cosW = Math.cos(w);
  const cos2W = Math.cos(2 * w);
  const sinW = Math.sin(w);
  const sin2W = Math.sin(2 * w);

  const numReal = coeffs.b0 + coeffs.b1 * cosW + coeffs.b2 * cos2W;
  const numImag = -(coeffs.b1 * sinW + coeffs.b2 * sin2W);
  const denReal = 1 + coeffs.a1 * cosW + coeffs.a2 * cos2W;
  const denImag = -(coeffs.a1 * sinW + coeffs.a2 * sin2W);

  const numMag = Math.sqrt(numReal * numReal + numImag * numImag);
  const denMag = Math.sqrt(denReal * denReal + denImag * denImag);

  return 20 * Math.log10(numMag / denMag);
}

/** Compute combined frequency response for all bands. Returns array of {freq, db} points. */
export function computeResponse(
  bands: EqBand[],
  sampleRate: number = 48000,
  numPoints: number = 200,
): { freq: number; db: number }[] {
  const minFreq = 20;
  const maxFreq = 20000;
  const logMin = Math.log10(minFreq);
  const logMax = Math.log10(maxFreq);

  const coeffsList = bands.map((b) => bandToCoeffs(b, sampleRate));
  const points: { freq: number; db: number }[] = [];

  for (let i = 0; i < numPoints; i++) {
    const logFreq = logMin + (i / (numPoints - 1)) * (logMax - logMin);
    const freq = Math.pow(10, logFreq);
    let db = 0;
    for (const coeffs of coeffsList) {
      db += magnitudeAt(coeffs, freq, sampleRate);
    }
    points.push({ freq, db });
  }

  return points;
}
