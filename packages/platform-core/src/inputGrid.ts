import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import { type DeviceInput } from "@cellsymphony/device-contracts";
import type { MusicalEvent } from "@cellsymphony/musical-events";
import type { Deps } from "./inputModifier";
import type { PlatformEffect, PlatformState } from "./index";
import { clamp } from "./coreUtils";
import { applySampleAssignment, danceModeFromRow, handleTouchGridPress } from "./inputInternal";
import { applyParamModMapping, paramBindingFromMenuNode } from "./paramMod";
import { visibleChildren } from "./menuView";
import { makeToast } from "./toast";
import { applyFxAssignment, releaseMomentaryFx } from "./touchFxRuntime";
import { clampPartIndex, clampSampleSlotIndex, PLATFORM_CAPS } from "./platformCaps";

export function handleGridInput<TState>(
  state: PlatformState<TState>,
  input: DeviceInput,
  behavior: BehaviorEngine<TState, unknown>,
  deps: Deps<TState>,
  nextState: PlatformState<TState>,
  events: MusicalEvent[],
  effects: PlatformEffect[]
): { state: PlatformState<TState>; events: MusicalEvent[]; effects: PlatformEffect[] } | null {
  const pressed = (i: any): boolean => (typeof i.pressed === "boolean" ? i.pressed : true);

  // Combined modifier + rightmost column → clone source
  if (input.type === "grid_press" && nextState.system.combinedModifierHeld && input.x === PLATFORM_CAPS.gridWidth - 1) {
    const srcIdx = clamp(Math.floor(input.y), 0, Math.min(PLATFORM_CAPS.partCount, PLATFORM_CAPS.gridHeight) - 1);
    nextState.system = { ...nextState.system, pendingCloneSource: srcIdx, toast: makeToast(`Clone P${srcIdx + 1} → select target`) };
    return { state: nextState, events, effects };
  }

  // FX assign mode
  if (nextState.system.fxAssignMode) {
    if (input.type === "button_a" && pressed(input)) {
      nextState.system = { ...nextState.system, fxAssignMode: null, toast: makeToast("Assign mode off") };
      return { state: nextState, events, effects };
    }
    if (input.type === "grid_press") {
      nextState = applyFxAssignment(nextState, input.x, input.y);
      deps.autoSaveEffect(nextState, effects);
      return { state: nextState, events, effects };
    }
  }

  // Sample assign mode
  if (nextState.system.sampleAssign) {
    if (input.type === "button_a" && pressed(input)) {
      nextState.system = { ...nextState.system, sampleAssign: null, toast: makeToast("Assign mode off") };
      return { state: nextState, events, effects };
    }

    if (input.type === "grid_press" && nextState.system.fnHeld && !nextState.system.shiftHeld && input.x === PLATFORM_CAPS.gridWidth - 1) {
      const danceMode = danceModeFromRow(input.y, (nextState.runtimeConfig as any).danceMode ?? nextState.system.danceMode);
      nextState = deps.writeAnyValue(nextState, "danceMode", danceMode);
      nextState.system = { ...nextState.system, toast: makeToast(`Dance: ${danceMode}`) };
      nextState.menu = { stack: [3], cursor: 0, editing: false };
      deps.autoSaveEffect(nextState, effects);
      return { state: nextState, events, effects };
    }

    if (input.type === "grid_press" && nextState.system.fnHeld && input.x === 0) {
      const idx = clamp(Math.floor(input.y), 0, Math.min(PLATFORM_CAPS.partCount, PLATFORM_CAPS.gridHeight) - 1);
      const pending = nextState.system.pendingCloneSource;
      if (pending !== null && pending !== idx) {
        const parts = Array.isArray((nextState.runtimeConfig as any).parts) ? [...((nextState.runtimeConfig as any).parts as any[])] : [];
        parts[idx] = structuredClone(parts[pending]);
        nextState = { ...nextState, runtimeConfig: { ...(nextState.runtimeConfig as any), parts } as any };
        const partStates = Array.isArray((nextState as any).partStates) ? [...(nextState as any).partStates] : [];
        if (partStates[pending] !== undefined) partStates[idx] = structuredClone(partStates[pending]);
        (nextState as any).partStates = partStates;
      }
      nextState = deps.writeAnyValue(nextState, "activePartIndex", idx);
      nextState = deps.reinitBehaviorState(nextState, "activePartIndex");
      nextState.system = { ...nextState.system, danceMode: "none", pendingCloneSource: null, toast: makeToast(pending !== null && pending !== idx ? `Cloned P${pending + 1} → P${idx + 1}` : `Part ${idx + 1}`) };
      const ms = nextState.menu.stack;
      if (ms.length >= 2 && ms[0] <= 1) {
        const partGroups = visibleChildren((deps.menuTree(nextState) as any).children?.[ms[0]], nextState);
        const newPartIdx = Math.min(idx, partGroups.length - 1);
        if (newPartIdx !== ms[1]) {
          const partKids = visibleChildren(partGroups[newPartIdx], nextState);
          nextState = { ...nextState, menu: { ...nextState.menu, stack: [ms[0], newPartIdx, ...ms.slice(2)], cursor: Math.min(nextState.menu.cursor, Math.max(0, partKids.length - 1)), editing: false } };
        }
      }
      deps.autoSaveEffect(nextState, effects);
      return { state: nextState, events, effects };
    }

    if (input.type === "grid_press") {
      const mode = nextState.system.combinedModifierHeld
        ? "column"
        : nextState.system.shiftHeld
          ? "row"
        : "single";
      nextState = applySampleAssignment(nextState, nextState.system.sampleAssign.instrumentSlot, nextState.system.sampleAssign.sampleSlot, input.x, input.y, mode as "single" | "row" | "column");
      nextState.system = { ...nextState.system };
      deps.autoSaveEffect(nextState, effects);
      return { state: nextState, events, effects };
    }

    if (input.type === "encoder_turn" && deps.isMainEncoderInput(input.id)) {
      const { instrumentSlot } = nextState.system.sampleAssign;
      const delta = clamp(input.delta, -1, 1);
      const instruments = Array.isArray((nextState.runtimeConfig as any).instruments) ? [...((nextState.runtimeConfig as any).instruments as any[])] : [];
      const inst = { ...instruments[instrumentSlot] };
      const sample = { ...(inst.sample ?? {}) };
      const currentSlot = sample.selectedSlot ?? 0;
      const newSlot = clampSampleSlotIndex(currentSlot + delta);
      sample.selectedSlot = newSlot;
      inst.sample = sample;
      instruments[instrumentSlot] = inst;
      nextState = { ...nextState, runtimeConfig: { ...(nextState.runtimeConfig as any), instruments } as any };
      nextState.system = { ...nextState.system, sampleAssign: { instrumentSlot, sampleSlot: newSlot }, toast: makeToast(`Slot ${newSlot + 1}`) };
      return { state: nextState, events, effects };
    }

    return { state: nextState, events, effects };
  }

  // Fn+rightmost column → Dance page
  if (input.type === "grid_press" && nextState.system.fnHeld && !nextState.system.shiftHeld && input.x === PLATFORM_CAPS.gridWidth - 1) {
    const danceMode = danceModeFromRow(input.y, (nextState.runtimeConfig as any).danceMode ?? nextState.system.danceMode);
    nextState = deps.writeAnyValue(nextState, "danceMode", danceMode);
    nextState.system = { ...nextState.system, toast: makeToast(`Dance: ${danceMode}`) };
    nextState.menu = { stack: [3], cursor: 0, editing: false };
    deps.autoSaveEffect(nextState, effects);
    return { state: nextState, events, effects };
  }

  // Trigger-gate Dance mode
  if (nextState.system.danceMode === "trigger-gate" && input.type === "grid_press") {
    const fnOnly = nextState.system.fnHeld && !nextState.system.shiftHeld;
    if (!fnOnly) {
      const mode = nextState.system.combinedModifierHeld
        ? "column"
        : nextState.system.shiftHeld
          ? "row"
        : "single";
      nextState = handleTouchGridPress(nextState, input, effects, deps, mode as "single" | "row" | "column");
      deps.autoSaveEffect(nextState, effects);
      return { state: nextState, events, effects };
    }
  }

  // Other Dance modes (mix, pan)
  if (nextState.system.danceMode !== "none" && !nextState.system.fnHeld && !nextState.system.shiftHeld && input.type === "grid_press") {
    nextState = handleTouchGridPress(nextState, input, effects, deps);
    if (state.system.danceMode === "mix" || state.system.danceMode === "pan") deps.autoSaveEffect(nextState, effects);
    return { state: nextState, events, effects };
  }

  // Shift+grid param modulation
  if (input.type === "grid_press" && nextState.system.shiftHeld && !nextState.system.fnHeld && nextState.system.danceMode === "none") {
    const view = deps.locate(deps.menuTree(nextState), nextState, nextState.menu);
    const selected = view.siblings[nextState.menu.cursor] as any;
    const binding = paramBindingFromMenuNode(selected);
    if (binding) {
      const mapped = applyParamModMapping(nextState, binding, input.x, input.y);
      if (mapped) {
        nextState = { ...mapped.state, system: { ...mapped.state.system, toast: makeToast(mapped.message) } };
        deps.autoSaveEffect(nextState, effects);
        return { state: nextState, events, effects };
      }
    }
  }

  // Grid release → xy release or momentary FX release
  if (nextState.system.danceMode !== "none" && !nextState.system.fnHeld && !nextState.system.shiftHeld && input.type === "grid_release") {
    if (nextState.system.danceMode === "xy") {
      const xyRelease = (nextState.runtimeConfig as any)?.xyRelease ?? "sample-hold";
      if (xyRelease === "reset-center") {
        nextState = {
          ...nextState,
          runtimeConfig: {
            ...nextState.runtimeConfig,
            xyTouch: { x: 0.5, y: 0.5, active: false }
          }
        } as any;
      } else {
        nextState = {
          ...nextState,
          runtimeConfig: {
            ...nextState.runtimeConfig,
            xyTouch: { ...(nextState.runtimeConfig as any).xyTouch, active: false }
          }
        } as any;
      }
      return { state: nextState, events, effects };
    }
    nextState = releaseMomentaryFx(nextState, input.x, input.y, effects);
    return { state: nextState, events, effects };
  }

  // Fn+left column → part selection
  if (input.type === "grid_press" && nextState.system.fnHeld && input.x === 0) {
    const idx = clamp(Math.floor(input.y), 0, Math.min(PLATFORM_CAPS.partCount, PLATFORM_CAPS.gridHeight) - 1);
    const pending = nextState.system.pendingCloneSource;
    if (pending !== null && pending !== idx) {
      const parts = Array.isArray((nextState.runtimeConfig as any).parts) ? [...((nextState.runtimeConfig as any).parts as any[])] : [];
      parts[idx] = structuredClone(parts[pending]);
      nextState = { ...nextState, runtimeConfig: { ...(nextState.runtimeConfig as any), parts } as any };
      const partStates = Array.isArray((nextState as any).partStates) ? [...(nextState as any).partStates] : [];
      if (partStates[pending] !== undefined) partStates[idx] = structuredClone(partStates[pending]);
      (nextState as any).partStates = partStates;
    }
    nextState = deps.writeAnyValue(nextState, "activePartIndex", idx);
    nextState = deps.reinitBehaviorState(nextState, "activePartIndex");
    nextState.system = { ...nextState.system, danceMode: "none", pendingCloneSource: null, toast: makeToast(pending !== null && pending !== idx ? `Cloned P${pending + 1} → P${idx + 1}` : `Part ${idx + 1}`) };
    const ms = nextState.menu.stack;
    if (ms.length >= 2 && ms[0] <= 1) {
      const partGroups = visibleChildren((deps.menuTree(nextState) as any).children?.[ms[0]], nextState);
      const newPartIdx = Math.min(idx, partGroups.length - 1);
      if (newPartIdx !== ms[1]) {
        const partKids = visibleChildren(partGroups[newPartIdx], nextState);
        nextState = { ...nextState, menu: { ...nextState.menu, stack: [ms[0], newPartIdx, ...ms.slice(2)], cursor: Math.min(nextState.menu.cursor, Math.max(0, partKids.length - 1)), editing: false } };
      }
    }
    deps.autoSaveEffect(nextState, effects);
    return { state: nextState, events, effects };
  }

  return null;
}
