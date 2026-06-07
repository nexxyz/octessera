import { PLATFORM_CAPS } from "./platformCaps";
import type { PartConfig, TriggerProbabilityCellState, TriggerProbabilityMode } from "./platformTypes";

export const DEFAULT_TRIGGER_PROBABILITY_LOW_PCT = 25;
export const DEFAULT_TRIGGER_PROBABILITY_HIGH_PCT = 75;

export function createDefaultTriggerProbabilityMap(fill: TriggerProbabilityCellState = "full"): TriggerProbabilityCellState[] {
  return Array.from({ length: PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight }, () => fill);
}

export function normalizeTriggerProbabilityMode(value: unknown): TriggerProbabilityMode {
  return value === "zero" || value === "custom" || value === "full" ? value : "full";
}

export function normalizeTriggerProbabilityCellState(value: unknown): TriggerProbabilityCellState {
  return value === "zero" || value === "low" || value === "high" || value === "full" ? value : "full";
}

export function normalizeTriggerProbabilityThreshold(value: unknown, fallback: number): number {
  const num = Number(value);
  if (!Number.isFinite(num)) return fallback;
  return Math.max(0, Math.min(100, Math.round(num)));
}

export function resolveTriggerProbabilityLowPct(part: PartConfig | undefined): number {
  const high = resolveTriggerProbabilityHighPct(part);
  return Math.min(normalizeTriggerProbabilityThreshold(part?.l2?.triggerProbabilityLowPct, DEFAULT_TRIGGER_PROBABILITY_LOW_PCT), high);
}

export function resolveTriggerProbabilityHighPct(part: PartConfig | undefined): number {
  const lowRaw = normalizeTriggerProbabilityThreshold(part?.l2?.triggerProbabilityLowPct, DEFAULT_TRIGGER_PROBABILITY_LOW_PCT);
  const highRaw = normalizeTriggerProbabilityThreshold(part?.l2?.triggerProbabilityHighPct, DEFAULT_TRIGGER_PROBABILITY_HIGH_PCT);
  return Math.max(lowRaw, highRaw);
}

export function resolveTriggerProbabilityMode(part: PartConfig | undefined): TriggerProbabilityMode {
  return normalizeTriggerProbabilityMode(part?.l2?.triggerProbabilityMode);
}

export function resolveTriggerProbabilityMap(part: PartConfig | undefined): TriggerProbabilityCellState[] {
  const src = part?.l2?.triggerProbabilityMap;
  const out = createDefaultTriggerProbabilityMap();
  if (!Array.isArray(src)) return out;
  for (let i = 0; i < out.length; i += 1) out[i] = normalizeTriggerProbabilityCellState(src[i]);
  return out;
}

export function resolveTriggerProbabilityPct(part: PartConfig | undefined, idx: number): number {
  const mode = resolveTriggerProbabilityMode(part);
  if (mode === "zero") return 0;
  if (mode === "full") return 100;
  const map = resolveTriggerProbabilityMap(part);
  const cell = map[idx] ?? "full";
  if (cell === "zero") return 0;
  if (cell === "low") return resolveTriggerProbabilityLowPct(part);
  if (cell === "high") return resolveTriggerProbabilityHighPct(part);
  return 100;
}

export function cycleTriggerProbabilityCellState(current: TriggerProbabilityCellState): TriggerProbabilityCellState {
  if (current === "zero") return "low";
  if (current === "low") return "high";
  if (current === "high") return "full";
  return "zero";
}

export function shouldAllowTrigger(part: PartConfig | undefined, idx: number, random01: number): boolean {
  const pct = resolveTriggerProbabilityPct(part, idx);
  if (pct <= 0) return false;
  if (pct >= 100) return true;
  return random01 < pct / 100;
}
