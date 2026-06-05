import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import { type DeviceInput } from "@cellsymphony/device-contracts";
import type { MusicalEvent } from "@cellsymphony/musical-events";
import { interpretGrid } from "@cellsymphony/interpretation-core";
import { mapIntentsToMusicalEvents } from "@cellsymphony/mapping-core";
import { applyParamModMapping, paramBindingFromMenuNode } from "./paramMod";
import { clamp } from "./coreUtils";
import { clampPartIndex, clampInstrumentIndex, PLATFORM_CAPS } from "./platformCaps";
import type { PlatformEffect, PlatformState } from "./index";
import { applySampleAssignment, filterTriggerGatedIntents, handleTouchGridPress, gridChanged, inputTransitionProfile } from "./inputInternal";
import { applyModulationResult, applyNoteBehavior, withScaleSteps } from "./musicTransforms";
import { makeToast } from "./toast";
import { activateMomentaryFx, applyFxAssignment, releaseMomentaryFx } from "./touchFxRuntime";
import { resolveAuxAutoMap } from "./auxAutoMap";
import { visibleChildren } from "./menuView";
import { startMomentaryFxPreview, stopMomentaryFxPreview } from "./momentaryFxPreview";
import { AUX_MAPPING_OVERLAY_DELAY_MS, EVENT_BLIP_MS, deadlineMs, heldForMs, nowMs } from "./timing";
import { toGridSnapshot } from "./runtimeHelpers";
import { emergencyBrakeState } from "./transportSafety";
import { applyModifierState, reinitBehaviorConfig, type Deps } from "./inputModifier";
import { handleConfirmInput } from "./inputConfirm";
import { handleMIDIClock, handleMIDIStartStop } from "./inputMIDI";
import { handleGridInput } from "./inputGrid";
import { buildAuxDeps, handleAuxEncoderInput } from "./inputAux";
import { clampSampleSlotIndex } from "./platformCaps";

export function routeInputWithDeps<TState>(state: PlatformState<TState>, input: DeviceInput, behavior: BehaviorEngine<TState, unknown>, deps: Deps<TState>): { state: PlatformState<TState>; events: MusicalEvent[]; effects: PlatformEffect[] } {
  const events: MusicalEvent[] = [];
  const effects: PlatformEffect[] = [];
  let nextState = { ...state };
  const pressed = (i: any): boolean => (typeof i.pressed === "boolean" ? i.pressed : true);

  const auxDeps = buildAuxDeps(deps);

  // Wake-up logic
  const now = nowMs();
  const isMidiRealtime = input.type === "midi_clock" || input.type === "midi_start" || input.type === "midi_continue" || input.type === "midi_stop";
  const wasAsleep = state.system.oledMode === "off" || state.system.oledMode === "splash";
  nextState.system = { ...nextState.system, lastInteractionMs: isMidiRealtime ? state.system.lastInteractionMs : now, oledMode: !isMidiRealtime && wasAsleep ? "normal" : state.system.oledMode };
  if (!isMidiRealtime && wasAsleep) return { state: nextState, events, effects };

  // Confirm dialog handler
  if (nextState.system.confirm) {
    return handleConfirmInput(state, input, behavior, deps, nextState, events, effects) ?? { state: nextState, events, effects };
  }

  // Modifier state
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

  // Grid input handler
  const gridResult = handleGridInput(state, input, behavior, deps, nextState, events, effects);
  if (gridResult) return gridResult;

  // MIDI clock
  if (input.type === "midi_clock") {
    const midiResult = handleMIDIClock(state, input, behavior, deps, nextState, events, effects);
    return { events: midiResult.events, state: midiResult.state, effects };
  }

  // MIDI start/continue/stop
  if (input.type === "midi_start" || input.type === "midi_continue" || input.type === "midi_stop") {
    const midiResult = handleMIDIStartStop(state, input, behavior, deps, nextState, events, effects);
    return { state: midiResult.state, events, effects };
  }

  // Button S / A / encoder controls
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
    const nowMsVal = nowMs();
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
        nextState.system = { ...nextState.system, stopLatched: false, transportFlash: "measure", transportFlashUntilMs: nowMsVal + 220 };
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

  // Shift+aux press → bind/unbind current menu parameter
  if (input.type === "encoder_press" && input.id?.startsWith("aux") && nextState.system.shiftHeld) {
    nextState = deps.assignAuxEncoder(nextState, input.id, effects, deps);
    return { state: nextState, events, effects };
  }

  // Aux encoder input
  const auxResult = handleAuxEncoderInput(state, input, deps, auxDeps, nextState, events, effects);
  nextState = auxResult.state;

  // Grid change → interpret → map → modulate → note behavior
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
