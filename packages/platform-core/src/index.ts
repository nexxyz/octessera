import { type BehaviorEngine, getBehavior, registerBehavior } from "@cellsymphony/behavior-api";
import { noneBehavior } from "@cellsymphony/behaviors-none";
import { lifeBehavior } from "@cellsymphony/behaviors-life";
import { sequencerBehavior } from "@cellsymphony/behaviors-sequencer";
import { keysBehavior } from "@cellsymphony/behaviors-keys";
import { brainBehavior } from "@cellsymphony/behaviors-brain";
import { antBehavior } from "@cellsymphony/behaviors-ant";
import { bounceBehavior } from "@cellsymphony/behaviors-bounce";
import { shapesBehavior } from "@cellsymphony/behaviors-pulse";
import { raindropsBehavior } from "@cellsymphony/behaviors-raindrops";
import { dlaBehavior } from "@cellsymphony/behaviors-dla";
import { gliderBehavior } from "@cellsymphony/behaviors-glider";
import {
  type DeviceInput,
  type DisplayFrame,
  type PageId,
  type SimulatorFrame,
  type TransportFrame
} from "@cellsymphony/device-contracts";
import { type MappingConfig } from "@cellsymphony/mapping-core";
import type { MusicalEvent } from "@cellsymphony/musical-events";
import { resolveMenuHelp, type HelpTarget } from "./menuHelp";
import { menuHelpTargetFromNode } from "./menuHelpTargets";
import { compactMenuPath } from "./compactMenuPath";
import {
  getSectionColorFromPath,
  isSpawnActionType,
  spawnActionTypeForBehavior
} from "./menuPresentation";
import { applyGlobalSound, pitchFromIntent } from "./musicTransforms";
import { axisGroup, midiInputNodes, midiOutputNodes, presetListNodes, presetRenameNodes, sampleBrowserNodes } from "./menuNodes";
import { currentMenuView as renderCurrentMenuView, locate, visibleChildren } from "./menuView";
import { pressMenuInput, turnMenuInput } from "./menuInput";
import { applyAuxUnbindChoice, assignAuxEncoder } from "./auxBindings";
import { pressAuxEncoder, pressAuxEncoderMapped, turnAuxEncoder, turnAuxEncoderMapped } from "./auxEncoderActions";
import { handleMenuAction } from "./actions";
import { getSynthPreset } from "./synthPresets";
export { GRID_DOMAIN, createGridDomain, type GridCell, type GridDomain } from "./gridDomain";
export { PAN_CENTER_POS, PAN_POSITION_COUNT, PAN_POSITION_MAX, PLATFORM_CAPS } from "./platformCaps";
export { cutoffDisplayToHz } from "./coreUtils";
import {
  factoryPayload,
  formatTimestamp,
  readAnyValue,
  reinitBehaviorState,
  textEditTurn,
  writeAnyValue
} from "./stateHelpers";
import { applyExternalClockPulses, tickTransport } from "./transportRuntime";
import { buildMenuTree } from "./menuTree";
import { routeInputWithDeps } from "./inputRouter";
import { createInitialPlatformState } from "./initialState";
import { makeToast } from "./toast";
import { EVENT_BLIP_MS, SLEEP_SPLASH_MS, nowMs as runtimeNowMs, deadlineMs } from "./timing";
import {
  applyConfigPayload as applyConfigPayloadRuntime,
  applyStoreResult as applyStoreResultRuntime,
  extractConfigPayload as extractConfigPayloadRuntime
} from "./storeRuntime";
import { clampPartIndex, PLATFORM_CAPS } from "./platformCaps";
import {
  clamp,
  cutoffDisplayToHz,
  fitOledMenuLine as fitOledMenuLineToColumns,
  fitOledText as fitOledTextToColumns,
  fitOledTextToWidth,
  formatDisplayValue,
  readValue,
  wrapOledText,
  writeValue
} from "./coreUtils";
registerBehavior(noneBehavior); registerBehavior(lifeBehavior); registerBehavior(sequencerBehavior);
registerBehavior(keysBehavior); registerBehavior(brainBehavior); registerBehavior(antBehavior);
registerBehavior(bounceBehavior); registerBehavior(shapesBehavior); registerBehavior(raindropsBehavior);
registerBehavior(dlaBehavior); registerBehavior(gliderBehavior);
function resolveBehavior(activeId: string): BehaviorEngine<any, any> {
  return getBehavior(activeId) ?? sequencerBehavior;
}
import { buildSimulatorFrame } from "./simulatorFrameBuilder";
import { ghostCellsForInactiveParts } from "./runtimeHelpers";
import { paramBindingFromMenuNode } from "./paramMod";
import { emergencyBrakeState } from "./transportSafety";
import {
  OLED_HEIGHT,
  OLED_TEXT_COLUMNS,
  OLED_TEXT_LINES,
  OLED_WIDTH,
  type ActionSpec,
  type BarValue,
  type ConfigPayload,
  type MenuNode,
  type MenuState,
  type MidiPortInfo,
  type NoteUnit,
  type NumericDisplayMode,
  type PendingAction,
  type PlatformEffect,
  type PlatformState,
  type RuntimeConfig,
  type ScanAxis,
  type StoreResult,
  type SystemState
} from "./platformTypes";
export {
  OLED_HEIGHT,
  OLED_TEXT_COLUMNS,
  OLED_TEXT_LINES,
  OLED_WIDTH
} from "./platformTypes";
export type { ActionSpec, ActiveFx, AudioCommand, BarValue, ConfigPayload, FxCellConfig, MenuNode, MomentaryFxConfig, MomentaryFxType, NumericDisplayMode, PlatformEffect, PlatformState, RuntimeConfig, StoreResult, TouchMode } from "./platformTypes";

export function createInitialState<TState>(behavior: BehaviorEngine<TState, unknown>): PlatformState<TState> {
  return createInitialPlatformState(behavior);
}

export function routeInput<TState>(
  state: PlatformState<TState>,
  input: DeviceInput,
  behavior: BehaviorEngine<TState, unknown>
): { state: PlatformState<TState>; events: MusicalEvent[]; effects: PlatformEffect[] } {
  const routed = routeInputWithDeps(state, input, behavior, {
    isMainEncoderInput,
    applyAuxUnbindChoice,
    writeAnyValue,
    backMenu,
    applyExternalClockPulses,
    locate,
    menuTree,
    resolveBehavior,
    readAnyValue,
    openContextHelp,
    pressMenu,
    turnMenu,
    assignAuxEncoder,
    pressAuxEncoder,
    turnAuxEncoder,
    pressAuxEncoderMapped,
    turnAuxEncoderMapped,
    reinitBehaviorState: (s, k) => reinitBehaviorState(s, k, resolveBehavior),
    autoSaveEffect,
    formatDisplayValue,
    isSpawnActionType,
    spawnActionTypeForBehavior,
    executeConfirmed
  });
  const active = clampPartIndex((routed.state.runtimeConfig as any).activePartIndex ?? 0);
  const partStates = Array.isArray((routed.state as any).partStates) ? ([...((routed.state as any).partStates as any[])] as any[]) : [];
  while (partStates.length < PLATFORM_CAPS.partCount) partStates.push(routed.state.behaviorState);
  partStates[active] = routed.state.behaviorState;
  return { ...routed, state: { ...routed.state, partStates } };
}

function executeConfirmed<TState>(
  state: PlatformState<TState>,
  action: PendingAction,
  effects: PlatformEffect[],
  behavior: BehaviorEngine<TState, unknown>
): PlatformState<TState> {
  if (action.kind === "factory_load") {
    const factoryBehavior = resolveBehavior("life") as BehaviorEngine<TState, unknown>;
    const factory = factoryPayload(factoryBehavior, createInitialState, extractConfigPayload);
    const next = applyConfigPayload(state, factory, factoryBehavior);
    return { ...next, system: { ...next.system, currentPresetName: null } };
  }
  if (action.kind === "default_load") {
    effects.push({ type: "store_load_default" });
    return state;
  }
  if (action.kind === "default_save") {
    effects.push({ type: "store_save_default", payload: extractConfigPayload(state), mode: "immediate" });
    return state;
  }
  if (action.kind === "preset_load") {
    effects.push({ type: "store_load_preset", name: action.name });
    return state;
  }
  if (action.kind === "preset_delete") {
    effects.push({ type: "store_delete_preset", name: action.name });
    return state;
  }
  if (action.kind === "preset_save") {
    effects.push({ type: "store_save_preset", name: action.name, payload: extractConfigPayload(state) });
    return state;
  }
  if (action.kind === "preset_rename") {
    effects.push({ type: "store_load_preset", name: action.from });
    return { ...state, system: { ...state.system, pendingRename: { from: action.from, to: action.to } } };
  }
  if (action.kind === "midi_panic") {
    effects.push({ type: "midi_panic" });
    return state;
  }
  if (action.kind === "synth_preset_load") {
    const preset = getSynthPreset(action.presetId as any);
    if (!preset) return state;
    const instruments = Array.isArray(state.runtimeConfig.instruments) ? state.runtimeConfig.instruments.slice() : [];
    const slot = Math.max(0, Math.min(PLATFORM_CAPS.instrumentCount - 1, action.slot | 0));
    const current = instruments[slot];
    if (!current) return state;
    instruments[slot] = { ...current, synth: structuredClone(preset.synth) };
    const next = {
      ...state,
      runtimeConfig: { ...state.runtimeConfig, instruments },
      system: { ...state.system, toast: makeToast(`Loaded synth: ${preset.label}`) }
    };
    if (next.runtimeConfig.autoSaveDefault) {
      effects.push({ type: "store_save_default", payload: extractConfigPayload(next), mode: "deferred" });
    }
    return next;
  }
  if (action.kind === "text_dirty_exit") {
    // Save path for a text exit prompt.
    // Clear edit session and exit editing, then optionally run configured action.
    let next: PlatformState<TState> = {
      ...state,
      system: { ...state.system, textEdit: null },
      menu: { ...state.menu, editing: false }
    };
    if (action.saveAction) {
      next = handleAction(next, action.saveAction, effects);
    }
    if (action.backAfter) {
      next = { ...next, menu: backMenu(next.menu) };
    }
    return next;
  }
  return state;
}

function textBackspace<TState>(state: PlatformState<TState>, key: string): PlatformState<TState> {
  const raw = String(readAnyValue(state, key) ?? "");
  const cursor = clamp(state.system.nameCursor, 0, raw.length);
  if (cursor <= 0) return state;
  const next = raw.slice(0, cursor - 1) + raw.slice(cursor);
  return {
    ...state,
    system: { ...state.system, draftName: next, nameCursor: cursor - 1 }
  };
}

export function tick<TState>(
  state: PlatformState<TState>,
  behavior: BehaviorEngine<TState, unknown>,
  elapsedSeconds: number = FRAME_SECONDS
): { state: PlatformState<TState>; events: MusicalEvent[]; effects: PlatformEffect[] } {
  const events: MusicalEvent[] = [];
  const effects: PlatformEffect[] = [];
  let next = { ...state };
  const nowMs = runtimeNowMs();

  // OLED sleep/splash timing.
  {
    const sleepMs = Math.max(0, Math.floor(next.runtimeConfig.screenSleepSeconds * 1000));
    if (next.system.oledMode === "normal" && sleepMs > 0 && nowMs - next.system.lastInteractionMs >= sleepMs) {
      next.system = {
        ...next.system,
        oledMode: "splash",
        oledSplashText: "Going to sleep",
        oledSplashUntilMs: deadlineMs(nowMs, SLEEP_SPLASH_MS)
      };
    } else if (next.system.oledMode === "splash" && nowMs >= next.system.oledSplashUntilMs) {
      // Startup splash returns to normal; sleep splash turns OLED off.
      const nextMode = next.system.oledSplashText === "Starting up" ? "normal" : "off";
      next.system = {
        ...next.system,
        oledMode: nextMode,
        toast:
          nextMode === "normal"
            ? makeToast("Help=Sh+Fn+Enter", { nowMs, durationMs: 2500 })
            : next.system.toast
      };
    }
  }

  // Transport flash decay.
  if (next.system.transportFlashUntilMs > 0 && nowMs > next.system.transportFlashUntilMs) {
    next.system = { ...next.system, transportFlashUntilMs: 0, transportFlash: "none" };
  }

  // Auto-save flash decay.
  if (next.system.autoSaveFlashUntilMs > 0 && nowMs > next.system.autoSaveFlashUntilMs) {
    next.system = { ...next.system, autoSaveFlashUntilMs: 0, autoSaveFlash: "none" };
  }

  const advanced = tickTransport(next, behavior, elapsedSeconds);
  next = advanced.state;
  events.push(...advanced.events);

  if (events.some((e) => e.type === "note_on")) {
    next.system = { ...next.system, eventBlipUntilMs: deadlineMs(nowMs, EVENT_BLIP_MS) };
  }
  return { state: next, events, effects };
}

export function extractConfigPayload<TState>(state: PlatformState<TState>): ConfigPayload {
  return extractConfigPayloadRuntime(state);
}

export function applyConfigPayload<TState>(
  state: PlatformState<TState>,
  payload: ConfigPayload,
  behavior: BehaviorEngine<TState, unknown>
): PlatformState<TState> {
  return applyConfigPayloadRuntime(state, payload, behavior, {
    resolveBehavior,
    factoryPayload: (b: BehaviorEngine<any, any>) => factoryPayload(b, createInitialState, extractConfigPayload)
  });
}

export function applyStoreResult<TState>(
  state: PlatformState<TState>,
  result: StoreResult,
  behavior: BehaviorEngine<TState, unknown>
): { state: PlatformState<TState>; effects: PlatformEffect[] } {
  return applyStoreResultRuntime(state, result, behavior, {
    resolveBehavior,
    factoryPayload: (b: BehaviorEngine<any, any>) => factoryPayload(b, createInitialState, extractConfigPayload)
  });
}

export function toSimulatorFrame<TState>(state: PlatformState<TState>, behavior: BehaviorEngine<TState, unknown>, options: { audioLoad?: { ratio: number; voiceSteal: boolean } } = {}): SimulatorFrame {
  const activePart = clampPartIndex((state.runtimeConfig as any).activePartIndex ?? 0);
  const part = (state.runtimeConfig as any).parts?.[activePart];
  const activeBehaviorId = String(part?.l1?.behaviorId ?? state.runtimeConfig.activeBehavior);
  const engine = (resolveBehavior(activeBehaviorId) as BehaviorEngine<any, unknown>) ?? behavior;
  const partStates = Array.isArray((state as any).partStates) ? ((state as any).partStates as any[]) : [];
  const model = engine.renderModel((partStates[activePart] ?? state.behaviorState) as any);
  const menuView = currentMenuView(state);
  const rawView = locate(menuTree(state), state, state.menu);
  const paramModBinding = state.system.shiftHeld ? paramBindingFromMenuNode(rawView.siblings[state.menu.cursor] as any) : null;
  const scanMode = part?.l2?.scanMode ?? state.runtimeConfig.scanMode;
  const scanAxis = part?.l2?.scanAxis ?? state.runtimeConfig.scanAxis;
  const scanSections = part?.l2?.scanSections ?? state.runtimeConfig.scanSections;
  const scanIndex = ((state as any).partScanIndex?.[activePart] ?? state.scanIndex) as number;
  const scanCursor = scanMode === "scanning" ? { axis: scanAxis, index: scanIndex, sections: scanSections } : null;
  const ghostCells = state.runtimeConfig.ghostCells === true ? ghostCellsForInactiveParts(state, activePart, model.cells.length) : undefined;
  return buildSimulatorFrame({ state, activePart, engine, model, menuView, scanCursor, toOledLines, audioLoad: options.audioLoad, ghostCells, paramModBinding });
}

function menuTree<TState>(state: PlatformState<TState>): MenuNode {
  return buildMenuTree(state, {
    resolveBehavior,
    axisGroup,
    presetListNodes,
    presetRenameNodes,
    midiOutputNodes,
    midiInputNodes,
    sampleBrowserNodes
  });
}

function currentMenuView<TState>(state: PlatformState<TState>): { path: string; lines: string[]; colors: number[]; barValues: (BarValue | null)[] } {
  return renderCurrentMenuView({
    state,
    menuTree,
    resolveBehavior,
    fitOledText: (text: string) => fitOledTextToColumns(text, OLED_TEXT_COLUMNS),
    readAnyValue,
    formatDisplayValue,
    oledTextLines: OLED_TEXT_LINES
  });
}

function openContextHelp<TState>(state: PlatformState<TState>): PlatformState<TState> {
  const view = locate(menuTree(state), state, state.menu);
  const selected = view.siblings[state.menu.cursor];
  if (!selected || selected.kind === "spacer") return state;

  const compactPath = compactMenuPath(view.path, "label" in (selected as any) ? String((selected as any).label ?? "") : "");

  const target = menuHelpTargetFromNode(view.path, selected);
  const help = resolveMenuHelp(target);
  const lines = [compactPath, ...wrapOledText(help.detail, OLED_TEXT_COLUMNS)];
  return {
    ...state,
    system: {
      ...state.system,
      confirm: {
        kind: "help_info",
        action: { kind: "help_info", title: help.title, lines },
        cursor: 0,
        options: ["Close"],
        scroll: 0
      }
    }
  };
}


function backMenu(menu: MenuState): MenuState {
  if (menu.editing) return { ...menu, editing: false };
  if (menu.stack.length === 0) return menu;
  const parentCursor = menu.stack[menu.stack.length - 1];
  return { ...menu, stack: menu.stack.slice(0, -1), cursor: parentCursor };
}

function pressMenu<TState>(state: PlatformState<TState>, effects: PlatformEffect[]): PlatformState<TState> {
  return pressMenuInput(state, effects, {
    menuTree,
    handleAction,
    readAnyValue,
    writeAnyValue,
    reinitBehaviorState: (nextState, key) => reinitBehaviorState(nextState, key, resolveBehavior),
    formatTimestamp,
    extractConfigPayload
  });
}

function turnMenu<TState>(state: PlatformState<TState>, delta: -1 | 1, effects: PlatformEffect[]): PlatformState<TState> {
  return turnMenuInput(state, delta, effects, {
    menuTree,
    readAnyValue,
    writeAnyValue,
    reinitBehaviorState: (nextState, key) => reinitBehaviorState(nextState, key, resolveBehavior),
    autoSaveEffect,
    textEditTurn
  });
}

function handleAction<TState>(state: PlatformState<TState>, action: ActionSpec, effects: PlatformEffect[]): PlatformState<TState> {
  return handleMenuAction(state, action, effects, {
    writeValue,
    extractConfigPayload,
    resolveBehavior
  });
}

function autoSaveEffect<TState>(state: PlatformState<TState>, effects: PlatformEffect[]): void {
  if (state.runtimeConfig.autoSaveDefault) {
    effects.push({ type: "store_save_default", payload: extractConfigPayload(state), mode: "deferred" });
  }
}


const FRAME_SECONDS = 0.15;

export function toOledLines(display: DisplayFrame): { lines: string[]; colors: number[] } {
  const title = fitOledTextToColumns(display.title, OLED_TEXT_COLUMNS);
  const titleColor = getSectionColorFromPath(display.title);
  const body = display.lines
    .slice(0, OLED_TEXT_LINES - 2)
    .map((line, idx) => ({
      line: line.trim().length === 0 ? "" : fitOledMenuLineToColumns(line, OLED_TEXT_COLUMNS),
      color: display.colors?.[idx] ?? 0xffff
    }));
  // Keep empty lines - they render as blank spacer lines
  return {
    lines: [title, ...body.map(b => b.line)].slice(0, OLED_TEXT_LINES - 1),
    colors: [titleColor, ...body.map(b => b.color)].slice(0, OLED_TEXT_LINES - 1)
  };
}

export function enumerateMenuHelpTargets<TState>(state: PlatformState<TState>): HelpTarget[] {
  const out: HelpTarget[] = [];
  function walk(node: MenuNode, s: PlatformState<TState>, path: string): void {
    const kids = visibleChildren(node, s);
    for (const child of kids) {
      if (child.kind === "spacer") continue;
      out.push(menuHelpTargetFromNode(path, child));
      if (child.kind === "group") {
        walk(child, s, `${path} > ${child.label ?? "Group"}`);
      }
    }
  }
  const root = menuTree(state);
  walk(root, state, "Menu");
  return out;
}

export type EnumHelpTarget = {
  path: string;
  key: string;
  kind: "enum";
  options: string[];
};

export function enumerateEnumHelpTargets<TState>(state: PlatformState<TState>): EnumHelpTarget[] {
  const out: EnumHelpTarget[] = [];
  function walk(node: MenuNode, s: PlatformState<TState>, path: string): void {
    const kids = visibleChildren(node, s);
    for (const child of kids) {
      if (child.kind === "group") {
        walk(child, s, `${path} > ${child.label ?? "Group"}`);
        continue;
      }
      if (child.kind !== "enum") continue;
      out.push({
        path: `${path} > ${child.label ?? "Option"}`,
        key: `key:${child.key}`,
        kind: "enum",
        options: child.options.slice()
      });
    }
  }
  walk(menuTree(state), state, "Menu");
  return out;
}
function isMainEncoderInput(id: "main" | "aux1" | "aux2" | "aux3" | "aux4" | undefined): boolean { return id === undefined || id === "main"; }
export function emergencyBrake<TState>(state: PlatformState<TState>): { state: PlatformState<TState>; events: MusicalEvent[] } { return emergencyBrakeState(state); }
