import type { MusicalEvent } from "@cellsymphony/musical-events";
import type { BehaviorConfigItem } from "@cellsymphony/behavior-api";
import type { PlatformEffect, PlatformState } from "./index";
import { clamp, readValue } from "./coreUtils";
import { compactSourcePathFromKey, fxTypeShort, locate } from "./menuView";
import { defaultFxParam } from "./fxDefaults";
import { MOMENTARY_PREVIEW_ID } from "./momentaryFxPreview";
import { makeToast } from "./toast";

export type AuxSharedDeps<TState> = {
  menuTree: (state: PlatformState<TState>) => any;
  resolveBehavior: (activeId: string) => any;
  readAnyValue: (state: PlatformState<TState>, key: string) => unknown;
  writeAnyValue: (state: PlatformState<TState>, key: string, value: unknown) => PlatformState<TState>;
  reinitBehaviorState: (state: PlatformState<TState>, key: string) => PlatformState<TState>;
  autoSaveEffect: (state: PlatformState<TState>, effects: PlatformEffect[]) => void;
  formatDisplayValue: (key: string, value: unknown, runtimeConfig?: any) => string;
  isSpawnActionType: (actionType: string) => boolean;
  spawnActionTypeForBehavior: (behaviorId: string) => string | null;
};

export function pressAuxEncoder<TState>(state: PlatformState<TState>, encoderId: string, _effects: PlatformEffect[], emit: (event: MusicalEvent) => void, deps: AuxSharedDeps<TState>): PlatformState<TState> {
  const binding = state.system.auxBindings[encoderId];
  if (!binding?.press) return setAuxToast(state, `${auxInputPrefix("press", encoderId)} No binding`);
  return executePressAction(state, encoderId, binding.press, _effects, emit, deps);
}

function executePressAction<TState>(
  state: PlatformState<TState>,
  encoderId: string,
  bindingPress: any,
  _effects: PlatformEffect[],
  emit: (event: MusicalEvent) => void,
  deps: AuxSharedDeps<TState>
): PlatformState<TState> {
  const inactiveMsg = inactivePressMessage(state, bindingPress, deps);
  if (inactiveMsg) return setAuxToast(state, `${auxInputPrefix("press", encoderId)} ${inactiveMsg}`);
  if (bindingPress.kind === "menu_action") {
    return pressMenuAction(state, encoderId, bindingPress.action, bindingPress.label ?? "Action");
  }
  let actionType = bindingPress.actionType;
  let label = bindingPress.label ?? bindingPress.actionType;
  const pfx = auxToastPrefix(state, "press", encoderId, bindingPress);
  if (bindingPress.routeKey === "trigger.life.spawn_now") {
    label = bindingPress.label ?? "Spawn Now";
    const resolvedAction = deps.spawnActionTypeForBehavior(state.runtimeConfig.activeBehavior);
    if (!resolvedAction) return setAuxToast(state, `${auxInputPrefix("press", encoderId)} N/A (${label})`);
    actionType = resolvedAction;
  }
  const behavior = deps.resolveBehavior(state.runtimeConfig.activeBehavior);
  const newBehaviorState = behavior.onInput(state.behaviorState, { type: "behavior_action", actionType }, { bpm: state.transport.bpm, emit });
  return setAuxToast({ ...state, behaviorState: newBehaviorState }, `${pfx} ${label}`);
}

export function pressAuxEncoderMapped<TState>(
  state: PlatformState<TState>,
  encoderId: string,
  bindingPress: any,
  _effects: PlatformEffect[],
  emit: (event: MusicalEvent) => void,
  deps: AuxSharedDeps<TState>
): PlatformState<TState> {
  return executePressAction(state, encoderId, bindingPress, _effects, emit, deps);
}

function executeTurnAction<TState>(
  state: PlatformState<TState>,
  encoderId: string,
  tKey: string,
  t: any,
  delta: -1 | 1,
  effects: PlatformEffect[],
  deps: AuxSharedDeps<TState>
): PlatformState<TState> {
  const label = t.label ?? tKey;
  const pfx = auxToastPrefix(state, "turn", encoderId, tKey);
  const inactiveMsg = inactiveTurnMessage(state, tKey, label, deps);
  if (inactiveMsg) return setAuxToast(state, `${auxInputPrefix("turn", encoderId)} ${inactiveMsg}`);
  if (t.kind === "number") {
    const current = deps.readAnyValue(state, tKey);
    const nextValue = clamp(Number(current) + delta * (t.step ?? 1), t.min ?? 0, t.max ?? 127);
    let nextState: PlatformState<TState> = deps.writeAnyValue(state, tKey, nextValue);
    if (tKey.startsWith("behaviorConfig.")) {
      const finalState = deps.reinitBehaviorState(nextState, tKey);
      deps.autoSaveEffect(finalState, effects);
      const v = deps.formatDisplayValue(tKey, deps.readAnyValue(finalState, tKey), finalState.runtimeConfig as any);
      return setAuxToast(finalState, `${pfx} ${label}: ${v}`);
    }
    if (tKey.includes(".l1.behaviorConfig.")) {
      nextState = applyBehaviorConfigField(nextState, tKey, nextValue);
    }
    deps.autoSaveEffect(nextState, effects);
    maybeUpdateMomentaryPreview(nextState, tKey, effects);
    const v = deps.formatDisplayValue(tKey, deps.readAnyValue(nextState, tKey), nextState.runtimeConfig as any);
    return setAuxToast(nextState, `${pfx} ${label}: ${v}`);
  }
  if (t.kind === "enum" && t.options) {
    const current = deps.readAnyValue(state, tKey);
    const idx = t.options.indexOf(String(current));
    const nextIdx = clamp(idx + delta, 0, t.options.length - 1);
    const raw = t.options[nextIdx];
    if (tKey === "transport.playing") {
      const nextState = { ...state, transport: { ...state.transport, playing: raw === "true" } };
      const v = deps.formatDisplayValue(tKey, deps.readAnyValue(nextState, tKey), nextState.runtimeConfig as any);
      return setAuxToast(nextState, `${pfx} ${label}: ${v}`);
    }
    if (tKey === "activeBehavior" || tKey.startsWith("behaviorConfig.")) {
      const nextState = deps.writeAnyValue(state, tKey, raw);
      const finalState = deps.reinitBehaviorState(nextState, tKey);
      deps.autoSaveEffect(finalState, effects);
      const v = deps.formatDisplayValue(tKey, deps.readAnyValue(finalState, tKey), finalState.runtimeConfig as any);
      return setAuxToast(finalState, `${pfx} ${label}: ${v}`);
    }
    let nextState = deps.writeAnyValue(state, tKey, raw);
    if (tKey.includes(".l1.behaviorConfig.")) {
      nextState = applyBehaviorConfigField(nextState, tKey, raw);
    }
    deps.autoSaveEffect(nextState, effects);
    maybeUpdateMomentaryPreview(nextState, tKey, effects);
    const v = deps.formatDisplayValue(tKey, deps.readAnyValue(nextState, tKey), nextState.runtimeConfig as any);
    return setAuxToast(nextState, `${pfx} ${label}: ${v}`);
  }
  if (t.kind === "bool") {
    const current = deps.readAnyValue(state, tKey);
    const clamped = current === true ? (delta > 0 ? true : false) : (delta < 0 ? false : true);
    let nextState = deps.writeAnyValue(state, tKey, clamped);
    if (tKey.includes(".l1.behaviorConfig.")) {
      nextState = applyBehaviorConfigField(nextState, tKey, clamped);
    }
    deps.autoSaveEffect(nextState, effects);
    maybeUpdateMomentaryPreview(nextState, tKey, effects);
    const v = deps.formatDisplayValue(tKey, deps.readAnyValue(nextState, tKey), nextState.runtimeConfig as any);
    return setAuxToast(nextState, `${pfx} ${label}: ${v}`);
  }
  return state;
}

export function turnAuxEncoder<TState>(state: PlatformState<TState>, encoderId: string, delta: -1 | 1, effects: PlatformEffect[], deps: AuxSharedDeps<TState>): PlatformState<TState> {
  const binding = state.system.auxBindings[encoderId];
  if (!binding?.turn) return setAuxToast(state, `${auxInputPrefix("turn", encoderId)} No binding`);
  return executeTurnAction(state, encoderId, binding.turn.key, binding.turn, delta, effects, deps);
}

export function turnAuxEncoderMapped<TState>(
  state: PlatformState<TState>,
  encoderId: string,
  t: any,
  delta: -1 | 1,
  effects: PlatformEffect[],
  deps: AuxSharedDeps<TState>
): PlatformState<TState> {
  const label = t.label ?? t.key;
  const pfx = auxToastPrefix(state, "turn", encoderId, t.key);
  const inactiveMsg = inactiveTurnMessage(state, t.key, label, deps);
  if (inactiveMsg) return setAuxToast(state, `${auxInputPrefix("turn", encoderId)} ${inactiveMsg}`);
  if (t.kind === "number") {
    const current = deps.readAnyValue(state, t.key);
    const nextValue = clamp(Number(current) + delta * (t.step ?? 1), t.min ?? 0, t.max ?? 127);
    let nextState: PlatformState<TState> = deps.writeAnyValue(state, t.key, nextValue);
    if (t.key.startsWith("behaviorConfig.")) {
      const finalState = deps.reinitBehaviorState(nextState, t.key);
      deps.autoSaveEffect(finalState, effects);
      const v = deps.formatDisplayValue(t.key, deps.readAnyValue(finalState, t.key), finalState.runtimeConfig as any);
      return setAuxToast(finalState, `${pfx} ${label}: ${v}`);
    }
    if (t.key.includes(".l1.behaviorConfig.")) {
      nextState = applyBehaviorConfigField(nextState, t.key, nextValue);
    }
    deps.autoSaveEffect(nextState, effects);
    maybeUpdateMomentaryPreview(nextState, t.key, effects);
    const v = deps.formatDisplayValue(t.key, deps.readAnyValue(nextState, t.key), nextState.runtimeConfig as any);
    return setAuxToast(nextState, `${pfx} ${label}: ${v}`);
  }
  if (t.kind === "enum" && t.options) {
    const current = deps.readAnyValue(state, t.key);
    const idx = t.options.indexOf(String(current));
    const nextIdx = clamp(idx + delta, 0, t.options.length - 1);
    const raw = t.options[nextIdx];
    if (t.key === "transport.playing") {
      const nextState = { ...state, transport: { ...state.transport, playing: raw === "true" } };
      const v = deps.formatDisplayValue(t.key, deps.readAnyValue(nextState, t.key), nextState.runtimeConfig as any);
      return setAuxToast(nextState, `${pfx} ${label}: ${v}`);
    }
    if (t.key === "activeBehavior" || t.key.startsWith("behaviorConfig.")) {
      const nextState = deps.writeAnyValue(state, t.key, raw);
      const finalState = deps.reinitBehaviorState(nextState, t.key);
      deps.autoSaveEffect(finalState, effects);
      const v = deps.formatDisplayValue(t.key, deps.readAnyValue(finalState, t.key), finalState.runtimeConfig as any);
      return setAuxToast(finalState, `${pfx} ${label}: ${v}`);
    }
    let nextState = deps.writeAnyValue(state, t.key, raw);
    if (t.key.includes(".l1.behaviorConfig.")) {
      nextState = applyBehaviorConfigField(nextState, t.key, raw);
    }
    deps.autoSaveEffect(nextState, effects);
    maybeUpdateMomentaryPreview(nextState, t.key, effects);
    const v = deps.formatDisplayValue(t.key, deps.readAnyValue(nextState, t.key), nextState.runtimeConfig as any);
    return setAuxToast(nextState, `${pfx} ${label}: ${v}`);
  }
  if (t.kind === "bool") {
    const current = deps.readAnyValue(state, t.key);
    const clamped = current === true ? (delta > 0 ? true : false) : (delta < 0 ? false : true);
    let nextState = deps.writeAnyValue(state, t.key, clamped);
    if (t.key.includes(".l1.behaviorConfig.")) {
      nextState = applyBehaviorConfigField(nextState, t.key, clamped);
    }
    deps.autoSaveEffect(nextState, effects);
    maybeUpdateMomentaryPreview(nextState, t.key, effects);
    const v = deps.formatDisplayValue(t.key, deps.readAnyValue(nextState, t.key), nextState.runtimeConfig as any);
    return setAuxToast(nextState, `${pfx} ${label}: ${v}`);
  }
  return state;
}

function maybeUpdateMomentaryPreview<TState>(state: PlatformState<TState>, key: string, effects: PlatformEffect[]): void {
  if (!key.startsWith("touchFx.selected.params.")) return;
  const activeFx = state.system.activeFx;
  if (!Array.isArray(activeFx) || !activeFx.some((fx) => fx.cellX === -1 && fx.cellY === -1)) return;
  const selected = state.runtimeConfig.touchFx?.selected as any;
  const fxType = selected?.fxType;
  if (!fxType || fxType === "none") return;
  effects.push({ type: "audio_command", command: { type: "momentary_fx_update", id: MOMENTARY_PREVIEW_ID, params: structuredClone(selected.params ?? {}) } } as any);
}

function setAuxToast<TState>(state: PlatformState<TState>, message: string): PlatformState<TState> {
  return {
    ...state,
    system: {
      ...state.system,
      toast: makeToast(message, { current: state.system.toast, extend: true })
    }
  };
}

function auxInputPrefix(kind: "press" | "turn", encoderId: string): string {
  const index = encoderId.startsWith("aux") ? encoderId.slice(3) : encoderId;
  const lead = kind === "press" ? "S" : "T";
  return `${lead}${index}:`;
}

function auxPathFromKey<TState>(state: PlatformState<TState>, key: string): string | null {
  return compactSourcePathFromKey(state, key);
}

function auxPathFromPress<TState>(state: PlatformState<TState>, press: any): string | null {
  if (!press) return null;
  if (press.kind === "behavior_action" || press.actionType) {
    const activePart = Number((state.runtimeConfig as any).activePartIndex ?? 0) + 1;
    return `L1>P${activePart}`;
  }
  if (press.kind === "menu_action") {
    if (press.action?.type === "sample_assign_enter") {
      return `L3>I${Number(press.action.instrumentSlot) + 1}>Sample`;
    }
    if (press.action?.type === "fx_assign_enter") {
      return `L4>FX>${fxTypeShort(press.action.config?.fxType ?? "none")}`;
    }
  }
  return null;
}

function auxToastPrefix<TState>(state: PlatformState<TState>, kind: "press" | "turn", encoderId: string, keyOrPress?: string | any): string {
  const index = encoderId.startsWith("aux") ? encoderId.slice(3) : encoderId;
  const lead = kind === "press" ? "S" : "T";
  const base = `${lead}${index}`;
  let path: string | null = null;
  if (typeof keyOrPress === "string") {
    path = auxPathFromKey(state, keyOrPress);
  } else if (keyOrPress) {
    path = auxPathFromPress(state, keyOrPress);
  }
  return path ? `${base} ${path}` : `${base}:`;
}

function applyBehaviorConfigField<TState>(state: PlatformState<TState>, key: string, value: unknown): PlatformState<TState> {
  const match = /^parts\.(\d+)\.l1\.behaviorConfig\.(.+)$/.exec(key);
  if (!match) return state;
  const partIdx = parseInt(match[1], 10);
  const fieldName = match[2];
  const activePart = (state.runtimeConfig as any)?.activePartIndex ?? 0;
  const nextPartStates = [...state.partStates] as any[];
  if (nextPartStates[partIdx]) {
    nextPartStates[partIdx] = { ...nextPartStates[partIdx], [fieldName]: value };
  }
  return {
    ...state,
    partStates: nextPartStates as TState[],
    behaviorState: partIdx === activePart ? nextPartStates[partIdx] as TState : state.behaviorState
  };
}

function bindingScope(key: string, activePart: number): string {
  const busMatch = /^mixer\.buses\.(\d+)/.exec(key);
  if (busMatch) return `B${Number(busMatch[1]) + 1}`;
  const instMatch = /^instruments\.(\d+)/.exec(key);
  if (instMatch) return `I${Number(instMatch[1]) + 1}`;
  const partMatch = /^parts\.(\d+)/.exec(key);
  if (partMatch) return `P${Number(partMatch[1]) + 1}`;
  return `P${activePart + 1}`;
}

function inactiveTurnMessage<TState>(state: PlatformState<TState>, key: string, label: string, deps: AuxSharedDeps<TState>): string | null {
  const activePart = state.runtimeConfig.activePartIndex ?? 0;
  const scope = bindingScope(key, activePart);

  const fxMatch = /^mixer\.buses\.(\d+)\.(slot[12])\.params\.([^.]+)$/.exec(key);
  if (fxMatch) {
    const type = readValue(state.runtimeConfig, `mixer.buses.${fxMatch[1]}.${fxMatch[2]}.type`);
    if (defaultFxParam(type, fxMatch[3]) === undefined) {
      return `${scope} ${label} not active`;
    }
    return null;
  }

  const instMatch = /^instruments\.(\d+)\.(synth|sample|midiEngine)\./.exec(key);
  if (instMatch) {
    const instType = readValue(state.runtimeConfig, `instruments.${instMatch[1]}.type`);
    const typeMap: Record<string, string> = { synth: "synth", sample: "sampler", midiEngine: "midi" };
    if (instType !== typeMap[instMatch[2]]) {
      return `${scope} ${label} not active`;
    }
    return null;
  }

  const scanMatch = /^parts\.(\d+)\.l2\.(scanAxis|scanUnit|scanDirection)$/.exec(key);
  if (scanMatch) {
    const scanMode = readValue(state.runtimeConfig, `parts.${scanMatch[1]}.l2.scanMode`);
    if (scanMode !== "scanning") {
      return `${scope} ${label} not active`;
    }
    return null;
  }

  const partBehMatch = /^parts\.(\d+)\.l1\.behaviorConfig\.(.+)$/.exec(key);
  if (partBehMatch) {
    const behaviorId = readValue(state.runtimeConfig, `parts.${partBehMatch[1]}.l1.behaviorId`);
    const behavior = deps.resolveBehavior(String(behaviorId));
    if (!behavior.configMenu) return null;
    const configItems: BehaviorConfigItem[] = behavior.configMenu(behavior.init({}));
    if (!configItems.some((item: BehaviorConfigItem) => item.key === partBehMatch[2])) {
      return `${scope} ${label} not active`;
    }
    return null;
  }

  const globalBehMatch = /^behaviorConfig\.(\w+)\.(.+)$/.exec(key);
  if (globalBehMatch) {
    const behaviorId = String(state.runtimeConfig.activeBehavior ?? "");
    const itemKey = globalBehMatch[2];
    const behavior = deps.resolveBehavior(behaviorId);
    if (!behavior.configMenu) return null;
    const configItems: BehaviorConfigItem[] = behavior.configMenu(behavior.init({}));
    if (!configItems.some((item: BehaviorConfigItem) => item.key === itemKey)) {
      return `${scope} ${label} not active`;
    }
    return null;
  }

  return null;
}

function inactivePressMessage<TState>(state: PlatformState<TState>, bindingPress: any, deps: AuxSharedDeps<TState>): string | null {
  const activePart = state.runtimeConfig.activePartIndex ?? 0;
  const behaviorId = String(state.runtimeConfig.activeBehavior ?? "");
  const behavior = deps.resolveBehavior(behaviorId);
  const scope = `P${activePart + 1}`;

  if (bindingPress.kind === "menu_action") {
   if (bindingPress.action?.type === "sample_assign_enter") {
       const inst = (state.runtimeConfig as any).instruments?.[bindingPress.action.instrumentSlot];
       if (!inst || inst.type !== "sampler") return `${scope} ${bindingPress.label ?? "Assign"} not active`;
    }
    return null;
  }

  if (bindingPress.routeKey === "trigger.life.spawn_now") {
    const resolvedAction = deps.spawnActionTypeForBehavior(behaviorId);
    if (!resolvedAction) {
      return `${scope} ${bindingPress.label ?? "Spawn Now"} not active`;
    }
    return null;
  }

  if (behavior.configMenu) {
    const configItems: BehaviorConfigItem[] = behavior.configMenu(behavior.init({}));
    if (!configItems.some((item: BehaviorConfigItem) => item.type === "action" && item.key === bindingPress.actionType)) {
      return `${scope} ${bindingPress.label ?? bindingPress.actionType} not active`;
    }
  }
  return null;
}

function pressMenuAction<TState>(state: PlatformState<TState>, encoderId: string, action: any, label: string): PlatformState<TState> {
  if (action?.type === "sample_assign_enter") {
    const next = {
      ...state,
      system: {
        ...state.system,
        sampleAssign: { instrumentSlot: action.instrumentSlot, sampleSlot: action.sampleSlot },
      }
    };
    const pfx = auxToastPrefix(state, "press", encoderId, { kind: "menu_action", action });
    return setAuxToast(next, `${pfx} ${label} I${action.instrumentSlot + 1}/S${action.sampleSlot + 1}`);
  }
  if (action?.type === "fx_assign_enter") {
    const next = {
      ...state,
      system: {
        ...state.system,
        fxAssignMode: { config: structuredClone(action.config) }
      }
    };
    const pfx = auxToastPrefix(state, "press", encoderId, { kind: "menu_action", action });
    return setAuxToast(next, `${pfx} ${label} (${action.config.fxType})`);
  }
  return setAuxToast(state, `${auxInputPrefix("press", encoderId)} N/A (${label})`);
}
