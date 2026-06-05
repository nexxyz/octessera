import { PLATFORM_CAPS } from "./platformCaps";
import type { FxBusEffectType, GlobalFxEffectType } from "./platformTypes";

export const FX_SLOT_TYPES: FxBusEffectType[] = [
  "none",
  "reverb",
  "delay",
  "tremolo",
  "chorus",
  "flanger",
  "vibrato",
  "auto_pan",
  "filter_lfo",
  "wah",
  "eq",
  "compressor",
  "duck",
  "saturator",
  "distortion",
  "bitcrusher",
  "glitch"
];

export const GLOBAL_FX_SLOT_TYPES: GlobalFxEffectType[] = [
  "none",
  "vinyl",
  "eq",
  "compressor",
  "saturator",
  "distortion"
];

const FX_DEFAULT_PARAMS: Record<FxBusEffectType, Record<string, string | number>> = {
  none: {},
  reverb: { mixPct: 30, decay: 0.72, damp: 0.35 },
  delay: { timeMs: 250, feedback: 0.35, mixPct: 35 },
  tremolo: { rateHz: 4, depthPct: 60 },
  vibrato: { rateHz: 0.8, depthMs: 6, baseMs: 8, feedback: 0, mixPct: 100 },
  auto_pan: { rateHz: 0.5, depthPct: 100 },
  chorus: { rateHz: 0.8, depthMs: 14, baseMs: 22, feedback: 0, mixPct: 45 },
  flanger: { rateHz: 0.8, depthMs: 2, baseMs: 3, feedback: 0.35, mixPct: 45 },
  wah: { rateHz: 1.2, centerHz: 900, depthPct: 70, q: 6 },
  filter_lfo: { rateHz: 0.5, centerHz: 1600, depthPct: 70, q: 1 },
  duck: { source: "I1", threshold: 0.08, amountPct: 60, attackMs: 8, releaseMs: 160 },
  bitcrusher: { rateDiv: 4, bits: 6, mixPct: 100 },
  saturator: { drive: 1.8, mixPct: 100 },
  distortion: { drive: 2.5, clip: 0.6, mixPct: 100 },
  glitch: { chancePct: 8, sliceMs: 80, mixPct: 100 },
  compressor: { thresholdDb: -24, ratio: 4, attackMs: 10, releaseMs: 100, makeupDb: 0, mixPct: 100 },
  eq: { lowGainDb: 0, midGainDb: 0, midFreqHz: 1000, midQ: 1, highGainDb: 0, mixPct: 100 }
};

const GLOBAL_FX_DEFAULT_PARAMS: Record<GlobalFxEffectType, Record<string, string | number>> = {
  none: {},
  vinyl: { saturationPct: 15, cracklePct: 8, warpDepthPct: 5, mixPct: 100 },
  eq: structuredClone(FX_DEFAULT_PARAMS.eq),
  compressor: structuredClone(FX_DEFAULT_PARAMS.compressor),
  saturator: { drive: 1.8, mixPct: 100 },
  distortion: { drive: 2.5, clip: 0.6, mixPct: 100 }
};

export function isBusEffectType(value: unknown): value is FxBusEffectType {
  return typeof value === "string" && (FX_SLOT_TYPES as string[]).includes(value);
}

export function isGlobalFxEffectType(value: unknown): value is GlobalFxEffectType {
  return typeof value === "string" && (GLOBAL_FX_SLOT_TYPES as string[]).includes(value);
}

export function defaultFxParams(type: unknown): Record<string, string | number> {
  const safeType = isBusEffectType(type) ? type : "none";
  return structuredClone(FX_DEFAULT_PARAMS[safeType]);
}

export function defaultFxParam(type: unknown, paramKey: string): string | number | undefined {
  const safeType = isBusEffectType(type) ? type : "none";
  return FX_DEFAULT_PARAMS[safeType][paramKey];
}

export function defaultGlobalFxParams(type: unknown): Record<string, string | number> {
  const safeType = isGlobalFxEffectType(type) ? type : "none";
  return structuredClone(GLOBAL_FX_DEFAULT_PARAMS[safeType]);
}

export function defaultGlobalFxParam(type: unknown, paramKey: string): string | number | undefined {
  const safeType = isGlobalFxEffectType(type) ? type : "none";
  return GLOBAL_FX_DEFAULT_PARAMS[safeType][paramKey];
}

export function sanitizeFxParams(type: unknown, raw: unknown): Record<string, string | number> {
  const defaults = defaultFxParams(type);
  if (!raw || typeof raw !== "object") return defaults;
  const incoming = raw as Record<string, unknown>;
  const out: Record<string, string | number> = { ...defaults };
  for (const key of Object.keys(defaults)) {
    const fallback = defaults[key];
    const value = incoming[key];
    if (typeof fallback === "number") {
      const numeric = Number(value);
      out[key] = Number.isFinite(numeric) ? numeric : fallback;
      continue;
    }
    if (key === "source") {
      out[key] = sanitizeDuckSource(value);
    } else if (typeof value === "string") {
      out[key] = value;
    }
  }
  return out;
}

export function sanitizeGlobalFxParams(type: unknown, raw: unknown): Record<string, string | number> {
  const defaults = defaultGlobalFxParams(type);
  if (!raw || typeof raw !== "object") return defaults;
  const incoming = raw as Record<string, unknown>;
  const out: Record<string, string | number> = { ...defaults };
  for (const key of Object.keys(defaults)) {
    const fallback = defaults[key];
    const value = incoming[key];
    if (typeof fallback === "number") {
      const numeric = Number(value);
      out[key] = Number.isFinite(numeric) ? numeric : fallback;
      continue;
    }
    if (typeof value === "string") {
      out[key] = value;
    }
  }
  return out;
}

function sanitizeDuckSource(value: unknown): string {
  const source = String(value ?? "I1");
  const inst = /^I(\d+)$/.exec(source);
  if (inst) {
    const n = Number(inst[1]);
    return Number.isFinite(n) && n >= 1 && n <= PLATFORM_CAPS.instrumentCount ? `I${n}` : "I1";
  }
  const bus = /^B(\d+)$/.exec(source);
  if (bus) {
    const n = Number(bus[1]);
    return Number.isFinite(n) && n >= 1 && n <= PLATFORM_CAPS.busCount ? `B${n}` : "I1";
  }
  return "I1";
}
