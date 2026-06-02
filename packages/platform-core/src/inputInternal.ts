import type { DeviceInput } from "@cellsymphony/device-contracts";
import type { InterpretationProfile, AxisStrategy, TickStrategy } from "@cellsymphony/interpretation-core";
import type { PlatformEffect, PlatformState, RuntimeConfig } from "./index";
import type { TouchMode } from "./platformTypes";
import { clamp } from "./coreUtils";
import { clampInstrumentIndex, clampPartIndex, clampSampleSlotIndex, PLATFORM_CAPS } from "./platformCaps";
import { resolveTouchPanTarget, toGridSnapshot, touchPanPosFromGridX } from "./runtimeHelpers";
import { activateMomentaryFx, applyFxAssignment, releaseMomentaryFx } from "./touchFxRuntime";
import { visibleChildren } from "./menuView";
import { makeToast } from "./toast";

const TOUCH_PAGES: TouchMode[] = ["mix", "pan", "fx", "trigger-gate"];

type TouchGridDeps = {
  writeAnyValue: (state: any, key: string, val: unknown) => any;
};

export function handleTouchGridPress<TState>(
  state: PlatformState<TState>,
  input: Extract<DeviceInput, { type: "grid_press" }>,
  effects: PlatformEffect[],
  deps: TouchGridDeps
): PlatformState<TState> {
  if (state.system.touchMode === "mix") {
    const inst = clamp(Math.floor(input.x), 0, Math.min(PLATFORM_CAPS.instrumentCount, PLATFORM_CAPS.gridWidth) - 1);
    const instruments = Array.isArray((state.runtimeConfig as any).instruments) ? ((state.runtimeConfig as any).instruments as any[]) : [];
    if ((instruments[inst] as any)?.type === "none") return state;
    const volume = Math.round(clamp(Math.floor(input.y), 0, PLATFORM_CAPS.gridHeight - 1) / (PLATFORM_CAPS.gridHeight - 1) * 100);
    return deps.writeAnyValue(state, `instruments.${inst}.mixer.volume`, volume);
  }
  if (state.system.touchMode === "pan") {
    const inst = clamp(Math.floor(input.y), 0, Math.min(PLATFORM_CAPS.instrumentCount, PLATFORM_CAPS.gridHeight) - 1);
    const instruments = Array.isArray((state.runtimeConfig as any).instruments) ? ((state.runtimeConfig as any).instruments as any[]) : [];
    if ((instruments[inst] as any)?.type === "none") return state;
    const panPos = touchPanPosFromGridX(input.x);
    const target = resolveTouchPanTarget(state as PlatformState<unknown>, inst);
    if (target.route === "bus") {
      const afterBus = deps.writeAnyValue(state, `mixer.buses.${target.busIndex}.panPos`, panPos);
      return deps.writeAnyValue(afterBus, `instruments.${inst}.mixer.panPos`, panPos);
    }
    return deps.writeAnyValue(state, `instruments.${inst}.mixer.panPos`, panPos);
  }
  if (state.system.touchMode === "fx") {
    return activateMomentaryFx(state, input.x, input.y, effects);
  }
  if (state.system.touchMode === "trigger-gate") {
    // For trigger-gate mode, we're using logical coordinates directly from simulator
    // which are already correctly mapped (no need to use toLogicalIndex)
    const width = PLATFORM_CAPS.gridWidth;
    const height = PLATFORM_CAPS.gridHeight;
    
    // Single gate press - toggle that specific gate
    const activePartIndex = (state.runtimeConfig as any).activePartIndex ?? 0;
    const parts = (state.runtimeConfig as any).parts ?? [];
    const gates = parts?.[activePartIndex]?.l1?.triggerGates;
    
    if (gates) {
      // Direct indexing using row * width + column - this fixes the coordinate inversion issue
      const idx = input.y * width + input.x;
      const newGates = [...gates];
      newGates[idx] = !newGates[idx];
      
      const newParts = [...parts];
      newParts[activePartIndex] = {
        ...newParts[activePartIndex],
        l1: {
          ...newParts[activePartIndex]?.l1,
          triggerGates: newGates
        }
      };
      
      return {
        ...state,
        runtimeConfig: {
          ...state.runtimeConfig,
          parts: newParts
        }
      };
    }
  }
  return state;
}

export function handleTriggerGateExit<TState>(
  state: PlatformState<TState>,
  effects: PlatformEffect[]
): PlatformState<TState> {
  // Exit trigger-gate mode by switching back to mix mode
  if (state.system.touchMode === "trigger-gate") {
    return {
      ...state,
      system: {
        ...state.system,
        touchMode: "mix"
      }
    };
  }
  return state;
}

export function filterTriggerGatedIntents<TState>(
  intents: any[], 
  state: PlatformState<TState>, 
  partIdx: number
): any[] {
  // This function filters out intents that should be gated by trigger-gate mode
  // For now, return all intents (no filtering) to avoid breaking the system
  return intents;
}



export function touchPageFromRow(y: number, current: TouchMode): TouchMode {
  const index = Math.floor(y);
  const direct = TOUCH_PAGES[index];
  if (direct) return direct;
  return current === "none" ? "mix" : current;
}

export function applySampleAssignment<TState>(
  state: PlatformState<TState>,
  instrumentSlot: number,
  sampleSlot: number,
  x: number,
  y: number,
  mode: "single" | "row" | "column"
): PlatformState<TState> {
  const slot = clampInstrumentIndex(instrumentSlot);
  const sslot = clampSampleSlotIndex(sampleSlot);
  const gx = clamp(Math.floor(x), 0, PLATFORM_CAPS.gridWidth - 1);
  const gy = clamp(Math.floor(y), 0, PLATFORM_CAPS.gridHeight - 1);
  const instruments = Array.isArray((state.runtimeConfig as any).instruments) ? (state.runtimeConfig.instruments as any[]) : [];
  const inst = instruments[slot];
  if (!inst || inst.type !== "sample") return state;
  const sample = { ...(inst.sample ?? {}) };
  const levelsEnabled = sample.velocityLevelsEnabled === true;
  const assignments = Array.isArray(sample.assignments) ? ([...sample.assignments] as any[]) : [];
  const resolved = resolveNextAssignment(assignments, gx, gy, sslot, levelsEnabled);
  const points: Array<{ x: number; y: number }> = [];
  if (mode === "single") points.push({ x: gx, y: gy });
  else if (mode === "row") {
    for (let cx = 0; cx < PLATFORM_CAPS.gridWidth; cx += 1) points.push({ x: cx, y: gy });
  } else {
    for (let cy = 0; cy < PLATFORM_CAPS.gridHeight; cy += 1) points.push({ x: gx, y: cy });
  }
  for (const pt of points) {
    const idx = assignments.findIndex((a) => a.x === pt.x && a.y === pt.y);
    if (!resolved) {
      if (idx >= 0) assignments.splice(idx, 1);
      continue;
    }
    const next = { x: pt.x, y: pt.y, sampleSlot: sslot, ...(resolved.level ? { level: resolved.level } : {}) };
    if (idx >= 0) assignments[idx] = next;
    else assignments.push(next);
  }
  instruments[slot] = { ...inst, sample: { ...sample, assignments } };
  return { ...state, runtimeConfig: { ...(state.runtimeConfig as any), instruments } as any };
}

export function resolveNextAssignment(assignments: any[], x: number, y: number, sampleSlot: number, levelsEnabled: boolean): { level?: "high" | "medium" | "low" } | null {
  const current = assignments.find((a) => a.x === x && a.y === y);
  const selectedCurrent = current && Number(current.sampleSlot) === sampleSlot ? current : null;
  if (!levelsEnabled) {
    if (selectedCurrent) return null;
    return {};
  }
  const level = selectedCurrent?.level as "high" | "medium" | "low" | undefined;
  if (!selectedCurrent) return { level: "high" };
  if (level === "high") return { level: "medium" };
  if (level === "medium") return { level: "low" };
  return null;
}

export function gridChanged(before: { cells: boolean[] }, after: { cells: boolean[] }): boolean {
  const len = Math.min(before.cells.length, after.cells.length);
  for (let i = 0; i < len; i += 1) {
    if (before.cells[i] !== after.cells[i]) return true;
  }
  return false;
}

export function inputTransitionProfile(cfg: RuntimeConfig): InterpretationProfile {
  const tick: TickStrategy = { mode: "whole_grid_transitions" };
  const axisX: AxisStrategy = cfg.x.pitch.enabled ? { mode: "scale_step", step: Math.abs(cfg.x.pitch.steps) } : { mode: "timing_only" };
  const axisY: AxisStrategy = cfg.y.pitch.enabled ? { mode: "scale_step", step: Math.abs(cfg.y.pitch.steps) } : { mode: "timing_only" };
  return {
    id: "input_profile",
    event: { enabled: cfg.eventEnabled },
    state: { enabled: false, tick },
    x: axisX,
    y: axisY
  };
}
