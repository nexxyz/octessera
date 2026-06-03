import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import { type DeviceInput } from "@cellsymphony/device-contracts";
import type { MusicalEvent } from "@cellsymphony/musical-events";
import { interpretGrid } from "@cellsymphony/interpretation-core";
import { mapIntentsToMusicalEvents } from "@cellsymphony/mapping-core";
import { clamp } from "./coreUtils";
import { clampInstrumentIndex, clampPartIndex, clampSampleSlotIndex, PLATFORM_CAPS } from "./platformCaps";
import type { PlatformEffect, PlatformState } from "./index";
import { applySampleAssignment, filterTriggerGatedIntents, handleTouchGridPress, gridChanged, inputTransitionProfile, touchPageFromRow } from "./inputInternal";
import { applyModulationResult, applyNoteBehavior, withScaleSteps } from "./musicTransforms";
import { makeToast } from "./toast";
import { activateMomentaryFx, applyFxAssignment, releaseMomentaryFx } from "./touchFxRuntime";
import { resolveAuxAutoMap } from "./auxAutoMap";
import { visibleChildren } from "./menuView";
import { startMomentaryFxPreview, stopMomentaryFxPreview } from "./momentaryFxPreview";
import { AUX_MAPPING_OVERLAY_DELAY_MS, EVENT_BLIP_MS, deadlineMs, heldForMs, nowMs } from "./timing";
import { toGridSnapshot } from "./runtimeHelpers";
import { emergencyBrakeState } from "./transportSafety";
import type { SystemState } from "./platformTypes";
import { applyParamModMapping, paramBindingFromMenuNode } from "./paramMod";
function applyModifierState(system: SystemState, input: DeviceInput, down: boolean, now: number): { system: SystemState; combinedPressed: boolean; combinedReleased: boolean; handled: boolean } {
  if (input.type !== "button_shift" && input.type !== "button_fn") return { system, combinedPressed: false, combinedReleased: false, handled: false };
  const physicalShiftHeld = input.type === "button_shift" ? down : system.physicalShiftHeld;
  const physicalFnHeld = input.type === "button_fn" ? down : system.physicalFnHeld;
  const combinedModifierHeld = physicalShiftHeld && physicalFnHeld;
  const shiftHeld = combinedModifierHeld ? false : physicalShiftHeld;
  const fnHeld = combinedModifierHeld ? false : physicalFnHeld;
  return {
    system: {
      ...system,
      physicalShiftHeld,
      physicalFnHeld,
      shiftHeld,
      fnHeld,
      combinedModifierHeld,
      shiftHeldSinceMs: shiftHeld ? (system.shiftHeld ? system.shiftHeldSinceMs : now) : null
    },
    combinedPressed: combinedModifierHeld && !system.combinedModifierHeld,
    combinedReleased: !combinedModifierHeld && system.combinedModifierHeld,
    handled: true
  };
}

function reinitBehaviorConfig<TState>(
  state: PlatformState<TState>,
  deps: { resolveBehavior: (id: string) => any }
): PlatformState<TState> {
  const behaviorId = String((state.runtimeConfig as any).parts?.[(state.runtimeConfig as any).activePartIndex ?? 0]?.l1?.behaviorId ?? state.runtimeConfig.activeBehavior);
  const b = deps.resolveBehavior(behaviorId);
  const part: any = (state.runtimeConfig as any).parts?.[(state.runtimeConfig as any).activePartIndex ?? 0];
  const ns = (part?.l1?.behaviorConfig ?? state.runtimeConfig.behaviorConfig?.[behaviorId]) as Record<string, unknown> | undefined;
  const cfg: any = {};
  if (b.configMenu) for (const item of b.configMenu(b.init({}))) { const val = ns?.[item.key]; if (val !== undefined) cfg[item.key] = val; }
  const nextState = { ...state, behaviorState: b.init(cfg) };
  const activePart = clampPartIndex((state.runtimeConfig as any).activePartIndex ?? 0);
  if (Array.isArray((nextState as any).partStates) && (nextState as any).partStates.length > activePart) {
    (nextState as any).partStates[activePart] = nextState.behaviorState;
  }
  return nextState;
}

type Deps<TState> = {
  isMainEncoderInput: (id: "main" | "aux1" | "aux2" | "aux3" | "aux4" | undefined) => boolean;
  applyAuxUnbindChoice: (state: PlatformState<TState>, encoderId: string, choice: string) => PlatformState<TState>;
  writeAnyValue: (state: PlatformState<TState>, key: string, value: unknown) => PlatformState<TState>;
  backMenu: (menu: any) => any;
  applyExternalClockPulses: (state: PlatformState<TState>, behavior: BehaviorEngine<TState, unknown>, pulses: number) => { state: PlatformState<TState>; events: MusicalEvent[] };
  locate: (root: any, state: PlatformState<TState>, menu: any) => any;
  menuTree: (state: PlatformState<TState>) => any;
  resolveBehavior: (activeId: string) => BehaviorEngine<any, any>;
  readAnyValue: (state: PlatformState<TState>, key: string) => unknown;
  openContextHelp: (state: PlatformState<TState>) => PlatformState<TState>;
  pressMenu: (state: PlatformState<TState>, effects: PlatformEffect[]) => PlatformState<TState>;
  turnMenu: (state: PlatformState<TState>, delta: -1 | 1, effects: PlatformEffect[]) => PlatformState<TState>;
  assignAuxEncoder: any;
  pressAuxEncoder: any;
  turnAuxEncoder: any;
  pressAuxEncoderMapped: any;
  turnAuxEncoderMapped: any;
  reinitBehaviorState: (state: PlatformState<TState>, key: string) => PlatformState<TState>;
  autoSaveEffect: (state: PlatformState<TState>, effects: PlatformEffect[]) => void;
  formatDisplayValue: (key: string, value: unknown, runtimeConfig?: any) => string;
  isSpawnActionType: (actionType: string) => boolean;
  spawnActionTypeForBehavior: (behaviorId: string) => string | null;
  executeConfirmed: (state: PlatformState<TState>, action: any, effects: PlatformEffect[], behavior: BehaviorEngine<TState, unknown>) => PlatformState<TState>;
};

export function routeInputWithDeps<TState>(state: PlatformState<TState>, input: DeviceInput, behavior: BehaviorEngine<TState, unknown>, deps: Deps<TState>): { state: PlatformState<TState>; events: MusicalEvent[]; effects: PlatformEffect[] } {
  const events: MusicalEvent[] = [];
  const effects: PlatformEffect[] = [];
  let nextState = { ...state };
  const pressed = (i: any): boolean => (typeof i.pressed === "boolean" ? i.pressed : true);

  const auxDeps = {
    menuTree: deps.menuTree,
    resolveBehavior: deps.resolveBehavior,
    readAnyValue: deps.readAnyValue,
    writeAnyValue: deps.writeAnyValue,
    reinitBehaviorState: deps.reinitBehaviorState,
    autoSaveEffect: deps.autoSaveEffect,
    formatDisplayValue: deps.formatDisplayValue,
    isSpawnActionType: deps.isSpawnActionType,
    spawnActionTypeForBehavior: deps.spawnActionTypeForBehavior
  };

  {
    const now = nowMs();
    const sys = nextState.system;
    const isMidiRealtime = input.type === "midi_clock" || input.type === "midi_start" || input.type === "midi_continue" || input.type === "midi_stop";
    const wasAsleep = sys.oledMode === "off" || sys.oledMode === "splash";
    nextState.system = { ...sys, lastInteractionMs: isMidiRealtime ? sys.lastInteractionMs : now, oledMode: !isMidiRealtime && wasAsleep ? "normal" : sys.oledMode };
    if (!isMidiRealtime && wasAsleep) return { state: nextState, events, effects };
  }

  if (nextState.system.confirm) {
    const c = nextState.system.confirm;
    const modifier = applyModifierState(nextState.system, input, pressed(input), nowMs());
    if (modifier.handled) {
      nextState.system = modifier.system;
      if (modifier.combinedPressed) nextState.behaviorState = behavior.onInput(nextState.behaviorState, { type: "button_combined_modifier", pressed: true }, { bpm: nextState.transport.bpm, emit: (event) => events.push(event) });
      if (modifier.combinedReleased) nextState.behaviorState = behavior.onInput(nextState.behaviorState, { type: "button_combined_modifier", pressed: false }, { bpm: nextState.transport.bpm, emit: (event) => events.push(event) });
      return { state: nextState, events, effects };
    }
    if (input.type === "encoder_turn" && deps.isMainEncoderInput(input.id)) {
      if (c.kind === "help_info" && c.action.kind === "help_info") {
        const contentSlots = Math.max(1, 8 - 2);
        const maxScroll = Math.max(0, c.action.lines.length - contentSlots);
        nextState.system = { ...nextState.system, confirm: { ...c, scroll: clamp(c.scroll + input.delta, 0, maxScroll) } };
      } else {
        nextState.system = { ...nextState.system, confirm: { ...c, cursor: clamp(c.cursor + input.delta, 0, c.options.length - 1) } };
      }
    } else if (input.type === "encoder_press" && deps.isMainEncoderInput(input.id)) {
      const choice = c.options[c.cursor];
      if (c.kind === "aux_unbind" && c.action.kind === "aux_unbind") {
        if (choice !== "Cancel") {
          nextState = deps.applyAuxUnbindChoice(nextState, c.action.encoderId, choice);
          deps.autoSaveEffect(nextState, effects);
        }
      } else if (c.kind === "text_dirty_exit") {
        if (choice === "Save") nextState = deps.executeConfirmed(nextState, c.action, effects, behavior);
        else if (c.action.kind === "text_dirty_exit") {
          nextState = deps.writeAnyValue(nextState, c.action.key, c.action.original);
          nextState.system = { ...nextState.system, textEdit: null };
          nextState.menu = { ...nextState.menu, editing: false };
          if (c.action.backAfter) nextState.menu = deps.backMenu(nextState.menu);
        }
      } else if (choice === "Yes" || choice === "Confirm") {
        nextState = deps.executeConfirmed(nextState, c.action, effects, behavior);
      }
      nextState.system = { ...nextState.system, confirm: null };
    } else if (input.type === "button_a" && pressed(input)) {
      nextState.system = { ...nextState.system, confirm: null };
    }
    nextState.behaviorState = behavior.onInput(nextState.behaviorState, input, { bpm: nextState.transport.bpm, emit: (event) => events.push(event) });
    return { state: nextState, events, effects };
  }

  const modifier = applyModifierState(nextState.system, input, pressed(input), nowMs());
  if (modifier.handled) {
    const down = pressed(input);
    nextState.system = {
      ...modifier.system,
      auxOverlayScroll: input.type === "button_shift" ? 0 : modifier.system.auxOverlayScroll,
      pendingCloneSource: down ? modifier.system.pendingCloneSource : null
    };
    if (modifier.combinedPressed) nextState.behaviorState = behavior.onInput(nextState.behaviorState, { type: "button_combined_modifier", pressed: true }, { bpm: nextState.transport.bpm, emit: (event) => events.push(event) });
    if (modifier.combinedReleased) nextState.behaviorState = behavior.onInput(nextState.behaviorState, { type: "button_combined_modifier", pressed: false }, { bpm: nextState.transport.bpm, emit: (event) => events.push(event) });
    return { state: nextState, events, effects };
  }

  if (nextState.system.sampleAssign) {
    if (input.type === "button_a" && pressed(input)) {
      nextState.system = { ...nextState.system, sampleAssign: null, toast: makeToast("Assign mode off") };
      return { state: nextState, events, effects };
    }

    // FN navigation routes through sample assign mode
    if (input.type === "grid_press" && nextState.system.fnHeld && !nextState.system.shiftHeld && input.x === PLATFORM_CAPS.gridWidth - 1) {
      const touchMode = touchPageFromRow(input.y, nextState.system.touchMode);
      nextState.system = { ...nextState.system, touchMode, toast: makeToast(`Touch: ${touchMode}`) };
      nextState.menu = { stack: [3], cursor: 0, editing: false };
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
      nextState.system = { ...nextState.system, touchMode: "none", pendingCloneSource: null, toast: makeToast(pending !== null && pending !== idx ? `Cloned P${pending + 1} → P${idx + 1}` : `Part ${idx + 1}`) };
      const ms = nextState.menu.stack;
      if (ms.length >= 2 && ms[0] <= 1) {
        const partGroups = visibleChildren((deps.menuTree(nextState) as any).children?.[ms[0]], nextState);
        const newPartIdx = Math.min(idx, partGroups.length - 1);
        if (newPartIdx !== ms[1]) {
          const partKids = visibleChildren(partGroups[newPartIdx], nextState);
          nextState = { ...nextState, menu: { ...nextState.menu, stack: [ms[0], newPartIdx, ...ms.slice(2)], cursor: Math.min(nextState.menu.cursor, Math.max(0, partKids.length - 1)), editing: false } };
        }
      }
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

  if (input.type === "midi_clock") {
    if (nextState.runtimeConfig.midi.syncMode === "external" && nextState.runtimeConfig.midi.clockInEnabled) {
      const pulses = Math.max(0, Math.floor((input as any).pulses ?? 0));
      const advanced = deps.applyExternalClockPulses(nextState, behavior, pulses);
      nextState = advanced.state;
      events.push(...advanced.events);
      if (advanced.events.some((e) => e.type === "note_on")) nextState.system = { ...nextState.system, eventBlipUntilMs: deadlineMs(nowMs(), EVENT_BLIP_MS) };
    }
    return { state: nextState, events, effects };
  }

  if (input.type === "midi_start" || input.type === "midi_continue" || input.type === "midi_stop") {
    if (nextState.runtimeConfig.midi.syncMode === "external" && nextState.runtimeConfig.midi.clockInEnabled && nextState.runtimeConfig.midi.respondToStartStop) {
      if (input.type === "midi_stop") {
        nextState.transport = { ...nextState.transport, playing: false };
        nextState.system = { ...nextState.system, stopLatched: true };
      } else if (!nextState.system.pausedByUser) {
        if (input.type === "midi_start") {
          nextState.transport = { ...nextState.transport, playing: true, ppqnPulse: 0, tick: 0 };
          nextState.partScanIndex = nextState.partScanIndex.map(() => 0);
          nextState.partScanPulseAccumulator = nextState.partScanPulseAccumulator.map(() => 0);
          nextState.partAlgorithmPulseAccumulator = nextState.partAlgorithmPulseAccumulator.map(() => 0);
          nextState.scanIndex = 0;
          nextState.scanPulseAccumulator = 0;
          nextState.algorithmPulseAccumulator = 0;
          nextState.ppqnPulseRemainder = 0;
          nextState.system = { ...nextState.system, stopLatched: false, pendingResync: false, externalPpqnPulse: 0 };
        } else {
          nextState.transport = { ...nextState.transport, playing: true };
          nextState.system = { ...nextState.system, stopLatched: false };
        }
      }
    }
    return { state: nextState, events, effects };
  }

  if (input.type === "grid_press" && nextState.system.combinedModifierHeld && input.x === PLATFORM_CAPS.gridWidth - 1) {
    const srcIdx = clamp(Math.floor(input.y), 0, Math.min(PLATFORM_CAPS.partCount, PLATFORM_CAPS.gridHeight) - 1);
    nextState.system = { ...nextState.system, pendingCloneSource: srcIdx, toast: makeToast(`Clone P${srcIdx + 1} → select target`) };
    return { state: nextState, events, effects };
  }

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

  if (input.type === "grid_press" && nextState.system.fnHeld && !nextState.system.shiftHeld && input.x === PLATFORM_CAPS.gridWidth - 1) {
    const touchMode = touchPageFromRow(input.y, nextState.system.touchMode);
    nextState.system = { ...nextState.system, touchMode, toast: makeToast(`Touch: ${touchMode}`) };
    nextState.menu = { stack: [3], cursor: 0, editing: false };
    return { state: nextState, events, effects };
  }

  if (nextState.system.touchMode === "trigger-gate" && input.type === "grid_press") {
    const fnOnly = nextState.system.fnHeld && !nextState.system.shiftHeld;
    if (!fnOnly) {
      const mode = nextState.system.combinedModifierHeld
        ? "column"
        : nextState.system.shiftHeld
          ? "row"
          : "single";
      nextState = handleTouchGridPress(nextState, input, effects, deps, mode as "single" | "row" | "column");
      return { state: nextState, events, effects };
    }
  }

  if (nextState.system.touchMode !== "none" && !nextState.system.fnHeld && !nextState.system.shiftHeld && input.type === "grid_press") {
     nextState = handleTouchGridPress(nextState, input, effects, deps);
     if (state.system.touchMode === "mix" || state.system.touchMode === "pan") deps.autoSaveEffect(nextState, effects);
     return { state: nextState, events, effects };
   }

   if (input.type === "grid_press" && nextState.system.shiftHeld && !nextState.system.fnHeld && nextState.system.touchMode === "none") {
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

  if (nextState.system.touchMode !== "none" && !nextState.system.fnHeld && !nextState.system.shiftHeld && input.type === "grid_release") {
    nextState = releaseMomentaryFx(nextState, input.x, input.y, effects);
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
    nextState.system = { ...nextState.system, touchMode: "none", pendingCloneSource: null, toast: makeToast(pending !== null && pending !== idx ? `Cloned P${pending + 1} → P${idx + 1}` : `Part ${idx + 1}`) };
    const ms = nextState.menu.stack;
    if (ms.length >= 2 && ms[0] <= 1) {
      const partGroups = visibleChildren((deps.menuTree(nextState) as any).children?.[ms[0]], nextState);
      const newPartIdx = Math.min(idx, partGroups.length - 1);
      if (newPartIdx !== ms[1]) {
        const partKids = visibleChildren(partGroups[newPartIdx], nextState);
        nextState = { ...nextState, menu: { ...nextState.menu, stack: [ms[0], newPartIdx, ...ms.slice(2)], cursor: Math.min(nextState.menu.cursor, Math.max(0, partKids.length - 1)), editing: false } };
      }
    }
    return { state: nextState, events, effects };
  }

  if (input.type === "button_s" && pressed(input)) {
    if (nextState.system.shiftHeld && nextState.runtimeConfig.midi.syncMode !== "external") {
      const result = emergencyBrakeState(nextState);
      nextState = result.state;
      events.push(...result.events);
      return { state: nextState, events, effects };
    }
    if (nextState.runtimeConfig.midi.syncMode === "external" && nextState.system.shiftHeld) {
      nextState.system = { ...nextState.system, pendingResync: true };
      return { state: nextState, events, effects };
    }
    const view = deps.locate(deps.menuTree(nextState), nextState, nextState.menu);
    if (view.path.endsWith("/FX Page") || view.path.includes("L4: Touch/FX Page")) {
      nextState = startMomentaryFxPreview(nextState, effects);
      return { state: nextState, events, effects };
    }
    if (view.path.endsWith("/Choose Sample")) {
      const selected = view.siblings[nextState.menu.cursor] as any;
      if (selected?.kind === "action" && selected.action?.type === "sample_pick" && typeof selected.action.path === "string") {
        const browser = nextState.system.sampleBrowser;
        if (browser) {
          effects.push({ type: "audio_command", command: { type: "sample_preview", instrumentSlot: browser.instrumentSlot, sampleSlot: browser.sampleSlot, path: selected.action.path, velocity: 100 } });
        }
      }
      return { state: nextState, events, effects };
    }
  }

  if (input.type === "button_s" && !pressed(input)) {
    const before = nextState;
    nextState = stopMomentaryFxPreview(nextState, effects);
    if (before !== nextState) return { state: nextState, events, effects };
  }

  if (input.type === "button_s" && pressed(input)) {
    const wasPlaying = nextState.transport.playing;

    if (nextState.system.fnHeld && nextState.runtimeConfig.midi.syncMode !== "external") {
      nextState = { ...nextState, system: { ...nextState.system, triggerMuted: !nextState.system.triggerMuted, stopLatched: false, toast: makeToast(!nextState.system.triggerMuted ? "Triggers off" : "Triggers on") } };
      return { state: nextState, events, effects };
    }
    const now = nowMs();
    const playing = !wasPlaying;
    nextState.transport = { ...nextState.transport, playing };
    if (nextState.runtimeConfig.midi.syncMode === "external") {
      nextState.system = { ...nextState.system, pausedByUser: !playing };
      return { state: nextState, events, effects };
    }
    if (playing) {
      const isStopToPlay = nextState.system.stopLatched || (nextState.transport.ppqnPulse === 0 && nextState.transport.tick === 0);
      if (isStopToPlay) {
        nextState.transport = { ...nextState.transport, ppqnPulse: 0, tick: 0 };
        nextState.partScanIndex = nextState.partScanIndex.map(() => 0);
        nextState.partScanPulseAccumulator = nextState.partScanPulseAccumulator.map(() => 0);
        nextState.partAlgorithmPulseAccumulator = nextState.partAlgorithmPulseAccumulator.map(() => 0);
        nextState.scanPulseAccumulator = 0;
        nextState.algorithmPulseAccumulator = 0;
        nextState.ppqnPulseRemainder = 0;
        nextState.scanIndex = 0;
        nextState.system = { ...nextState.system, stopLatched: false, transportFlash: "measure", transportFlashUntilMs: now + 220 };
      } else {
        nextState.system = { ...nextState.system, stopLatched: false };
      }
    }
  } else if (input.type === "button_a" && pressed(input)) {
    const view = deps.locate(deps.menuTree(nextState), nextState, nextState.menu);
    const selected = view.siblings[nextState.menu.cursor];
    if (nextState.menu.editing && selected && selected.kind === "text" && nextState.system.shiftHeld) {
      const raw = String(deps.readAnyValue(nextState, selected.key) ?? "");
      const cursor = clamp(nextState.system.nameCursor, 0, raw.length);
      if (cursor > 0) {
        const next = raw.slice(0, cursor - 1) + raw.slice(cursor);
        nextState = { ...nextState, system: { ...nextState.system, draftName: next, nameCursor: cursor - 1 } };
      }
    } else if (nextState.system.shiftHeld && !nextState.system.fnHeld) {
      nextState = reinitBehaviorConfig(nextState, deps);
      nextState.system = { ...nextState.system, toast: makeToast("Grid cleared") };
    } else {
      if (nextState.menu.editing && selected && selected.kind === "text") {
        const current = String(deps.readAnyValue(nextState, selected.key) ?? "");
        const sess = nextState.system.textEdit;
        const dirty = sess && sess.key === selected.key ? current !== sess.original : false;
        if (dirty && sess) {
          nextState.system = { ...nextState.system, confirm: { kind: "text_dirty_exit", action: { kind: "text_dirty_exit", key: sess.key, original: sess.original, saveAction: sess.saveAction, backAfter: true, mode: "save" }, cursor: 0, options: ["Save", "Discard"], scroll: 0 } };
        } else {
          nextState.system = { ...nextState.system, textEdit: null };
          nextState.menu = deps.backMenu(nextState.menu);
        }
      } else nextState.menu = deps.backMenu(nextState.menu);
    }
  } else if (input.type === "encoder_press" && deps.isMainEncoderInput(input.id)) {
    if (nextState.system.combinedModifierHeld) return { state: deps.openContextHelp(nextState), events, effects };
    nextState = deps.pressMenu(nextState, effects);
  } else if (input.type === "encoder_turn" && deps.isMainEncoderInput(input.id)) {
    nextState = nextState.system.shiftHeld && heldForMs(nowMs(), nextState.system.shiftHeldSinceMs, AUX_MAPPING_OVERLAY_DELAY_MS)
      ? { ...nextState, system: { ...nextState.system, auxOverlayScroll: Math.max(0, (nextState.system.auxOverlayScroll ?? 0) + input.delta) } }
      : deps.turnMenu(nextState, input.delta, effects);
  }

  if (input.type === "encoder_press" && input.id && !deps.isMainEncoderInput(input.id)) {
    if (nextState.system.shiftHeld) {
      nextState = deps.assignAuxEncoder(nextState, input.id, effects, auxDeps);
    } else {
      const view = deps.locate(deps.menuTree(nextState), nextState, nextState.menu);
      const selected = view.siblings[nextState.menu.cursor] as any;
      const selectedKey = (selected && (selected.kind === "number" || selected.kind === "enum" || selected.kind === "bool")) ? String(selected.key ?? "") : undefined;
      const selectedAction = selected && selected.kind === "action" ? (selected.action as any) : null;
      const auto = resolveAuxAutoMap(nextState, { path: view.path, selectedKey, selectedAction }, deps.resolveBehavior);
      const slot = input.id === "aux1" ? auto.aux1 : input.id === "aux2" ? auto.aux2 : input.id === "aux3" ? auto.aux3 : auto.aux4;
      nextState = slot?.press
        ? deps.pressAuxEncoderMapped(nextState, input.id, slot.press, effects, (event: MusicalEvent) => events.push(event), auxDeps)
        : deps.pressAuxEncoder(nextState, input.id, effects, (event: MusicalEvent) => events.push(event), auxDeps);
    }
  }
  if (input.type === "encoder_turn" && input.id && !deps.isMainEncoderInput(input.id)) {
    const view = deps.locate(deps.menuTree(nextState), nextState, nextState.menu);
    const selected = view.siblings[nextState.menu.cursor] as any;
    const selectedKey = (selected && (selected.kind === "number" || selected.kind === "enum" || selected.kind === "bool")) ? String(selected.key ?? "") : undefined;
    const selectedAction = selected && selected.kind === "action" ? (selected.action as any) : null;
    const auto = resolveAuxAutoMap(nextState, { path: view.path, selectedKey, selectedAction }, deps.resolveBehavior);
    const slot = input.id === "aux1" ? auto.aux1 : input.id === "aux2" ? auto.aux2 : input.id === "aux3" ? auto.aux3 : auto.aux4;
    nextState = slot?.turn
      ? deps.turnAuxEncoderMapped(nextState, input.id, slot.turn, input.delta, effects, auxDeps)
      : deps.turnAuxEncoder(nextState, input.id, input.delta, effects, auxDeps);
  }
  const beforeInputGrid = behavior.interpretInputTransitions ? toGridSnapshot(behavior.renderModel(nextState.behaviorState)) : null;
  nextState.behaviorState = behavior.onInput(nextState.behaviorState, input, { bpm: nextState.transport.bpm, emit: (event) => events.push(event) });
  if (beforeInputGrid) {
    const afterInputGrid = toGridSnapshot(behavior.renderModel(nextState.behaviorState));
    if (gridChanged(beforeInputGrid, afterInputGrid) && (nextState.transport.playing || nextState.runtimeConfig.inputEventsWhilePaused)) {
      const profile = inputTransitionProfile(nextState.runtimeConfig);
      const intents = filterTriggerGatedIntents(interpretGrid(beforeInputGrid, afterInputGrid, nextState.transport.tick, profile), nextState, clampPartIndex((nextState.runtimeConfig as any).activePartIndex ?? 0));
      if (intents.length > 0) {
        const mapped = mapIntentsToMusicalEvents(intents, withScaleSteps(nextState.mappingConfig, nextState.runtimeConfig));
        const activePart = clampPartIndex((nextState.runtimeConfig as any).activePartIndex ?? 0);
        const modulation = applyModulationResult(intents, mapped, nextState.runtimeConfig, nextState.runtimeConfig, activePart);
        nextState = { ...nextState, runtimeConfig: modulation.runtimeConfig };
        const modulated = modulation.events;
        const instruments: any[] = Array.isArray((nextState.runtimeConfig as any).instruments) ? ((nextState.runtimeConfig as any).instruments as any[]) : [];
        const shaped = applyNoteBehavior(modulated, instruments, 0, nextState.system.heldNotes);
        nextState.system = { ...nextState.system, heldNotes: shaped.heldNotes };
        events.push(...shaped.events);
      }
    }
    const cellCount = PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight;
    const tt = (nextState.behaviorState as any)?.triggerTypes;
    if (Array.isArray(tt) && tt.length >= cellCount) {
      const newTT = [...tt];
      let changed = false;
      for (let i = 0; i < cellCount; i++) {
        if (beforeInputGrid.cells[i] !== afterInputGrid.cells[i]) {
          newTT[i] = afterInputGrid.cells[i] ? "activate" : "deactivate";
          changed = true;
        }
      }
      if (changed) (nextState.behaviorState as any).triggerTypes = newTT;
    }
  }
  if (events.some((e) => e.type === "note_on")) nextState.system = { ...nextState.system, eventBlipUntilMs: deadlineMs(nowMs(), EVENT_BLIP_MS) };
  return { state: nextState, events, effects };
}
