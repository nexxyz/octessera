import { clamp } from "./coreUtils";
import { PLATFORM_CAPS } from "./platformCaps";
import type { PlatformEffect, PlatformState } from "./platformTypes";
import { defaultMomentaryFxParams } from "./momentaryFx";
import type { MomentaryFxType } from "./platformTypes";
import { makeToast } from "./toast";
import { targetFromKey } from "./momentaryFxTarget";
import { nowMs } from "./timing";

export function activateMomentaryFx<TState>(state: PlatformState<TState>, x: number, y: number, effects: PlatformEffect[]): PlatformState<TState> {
  const cellX = clamp(Math.floor(x), 0, PLATFORM_CAPS.gridWidth - 1);
  const cellY = clamp(Math.floor(y), 0, PLATFORM_CAPS.gridHeight - 1);
  const assignments = Array.isArray((state.runtimeConfig as any).touchFx?.assignments) ? ((state.runtimeConfig as any).touchFx.assignments as any[]) : [];
  const assignment = assignments.find((a) => a?.x === cellX && a?.y === cellY);
  const config = assignment?.config;
  if (!config || config.fxType === "none") return state;
  const normalizedCfg = { ...structuredClone(config), targetKey: String((config as any).targetKey ?? "master") } as any;
  const activeFx = state.system.activeFx.filter((fx) => !(fx.cellX === cellX && fx.cellY === cellY));
  const replaced = activeFx.find((fx) => fx.fxType === normalizedCfg.fxType);
  const withoutSameType = activeFx.filter((fx) => fx.fxType !== normalizedCfg.fxType);
  if (!replaced && withoutSameType.length >= PLATFORM_CAPS.touchFxMaxConcurrent) return state;
  if (replaced) effects.push({ type: "audio_command", command: { type: "momentary_fx_stop", id: momentaryFxId(replaced.cellX, replaced.cellY) } });
  const next = { cellX, cellY, fxType: normalizedCfg.fxType, config: normalizedCfg, activatedAtMs: nowMs() };
  effects.push({
    type: "audio_command",
    command: { type: "momentary_fx_start", id: momentaryFxId(cellX, cellY), fxType: normalizedCfg.fxType, params: structuredClone(normalizedCfg.params ?? {}), target: targetFromKey(normalizedCfg.targetKey) }
  });
  return { ...state, system: { ...state.system, activeFx: [...withoutSameType, next] } };
}

export function releaseMomentaryFx<TState>(state: PlatformState<TState>, x: number, y: number, effects: PlatformEffect[]): PlatformState<TState> {
  if (state.system.danceMode !== "fx") return state;
  const cellX = clamp(Math.floor(x), 0, PLATFORM_CAPS.gridWidth - 1);
  const cellY = clamp(Math.floor(y), 0, PLATFORM_CAPS.gridHeight - 1);
  const nextActive = state.system.activeFx.filter((fx) => fx.cellX !== cellX || fx.cellY !== cellY);
  if (nextActive.length === state.system.activeFx.length) return state;
  effects.push({ type: "audio_command", command: { type: "momentary_fx_stop", id: momentaryFxId(cellX, cellY) } });
  return { ...state, system: { ...state.system, activeFx: nextActive } };
}

function momentaryFxId(cellX: number, cellY: number): string {
  return `momentary-fx:${cellX}:${cellY}`;
}

export function applyFxAssignment<TState>(state: PlatformState<TState>, x: number, y: number): PlatformState<TState> {
  const assign = state.system.fxAssignMode;
  if (!assign) return state;
  const cellX = clamp(Math.floor(x), 0, PLATFORM_CAPS.gridWidth - 1);
  const cellY = clamp(Math.floor(y), 0, PLATFORM_CAPS.gridHeight - 1);
  const normalizedAssign = { ...structuredClone(assign.config), targetKey: String((assign.config as any).targetKey ?? "master") } as any;
  const touchFx = (state.runtimeConfig as any).touchFx ?? { selected: normalizedAssign, assignments: [] };
  const selected = touchFx.selected
    ? { ...touchFx.selected, targetKey: String((touchFx.selected as any).targetKey ?? "master") }
    : normalizedAssign;
  const assignments = Array.isArray(touchFx.assignments) ? [...touchFx.assignments] : [];
  const idx = assignments.findIndex((a) => a?.x === cellX && a?.y === cellY);
  if (normalizedAssign.fxType === "none") {
    if (idx >= 0) assignments.splice(idx, 1);
  } else {
    const next = { x: cellX, y: cellY, config: normalizedAssign };
    if (idx >= 0) {
      const existing = assignments[idx];
      const sameType = existing?.config?.fxType === normalizedAssign.fxType;
      const sameTarget = String(existing?.config?.targetKey ?? "master") === normalizedAssign.targetKey;
      if (sameType && sameTarget && momentaryParamsEqual(normalizedAssign.fxType, existing?.config?.params, normalizedAssign.params)) {
        assignments.splice(idx, 1);
        return {
          ...state,
          runtimeConfig: { ...(state.runtimeConfig as any), touchFx: { ...touchFx, assignments } } as any,
          system: { ...state.system, toast: makeToast("FX cleared") }
        };
      }
      assignments[idx] = next;
    } else {
      assignments.push(next);
    }
  }
  return {
    ...state,
    runtimeConfig: { ...(state.runtimeConfig as any), touchFx: { ...touchFx, selected, assignments } } as any,
    system: { ...state.system, toast: makeToast(normalizedAssign.fxType === "none" ? "FX cleared" : `FX mapped: ${normalizedAssign.fxType}`) }
  };
}

function normalizeMomentaryParams(fxType: MomentaryFxType, raw: unknown): Record<string, number> {
  const base = defaultMomentaryFxParams(fxType);
  const src = raw && typeof raw === "object" ? (raw as Record<string, unknown>) : {};
  const out: Record<string, number> = {};
  for (const k of Object.keys(base)) {
    const v = Number(src[k] ?? (base as any)[k]);
    out[k] = Number.isFinite(v) ? v : Number((base as any)[k]);
  }
  return out;
}

function momentaryParamsEqual(fxType: MomentaryFxType, a: unknown, b: unknown): boolean {
  const aa = normalizeMomentaryParams(fxType, a);
  const bb = normalizeMomentaryParams(fxType, b);
  const keys = Object.keys(aa);
  for (const k of keys) {
    if (aa[k] !== bb[k]) return false;
  }
  return true;
}
