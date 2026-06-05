import type { MappingConfig } from "@cellsymphony/mapping-core";
import type { TransportFrame } from "@cellsymphony/device-contracts";
import { readNestedValue, readValue, writeNestedValue, writeValue, deriveBusAutoName, derivePartAutoName, deriveInstAutoName, overrideFromPart, preferMapping } from "./coreUtils";
import { defaultFxParam, defaultFxParams, defaultGlobalFxParam, defaultGlobalFxParams, isBusEffectType, isGlobalFxEffectType } from "./fxDefaults";
import { defaultMomentaryFxParams, isMomentaryFxType } from "./momentaryFx";
import type { PlatformState, SystemState } from "./platformTypes";
import { clampPartIndex } from "./platformCaps";

function syncLegacyFromActivePart<TState>(state: PlatformState<TState>): PlatformState<TState> {
  const active = clampPartIndex((state.runtimeConfig as any).activePartIndex ?? 0);
  const part = (state.runtimeConfig as any).parts?.[active];
  if (!part) return state;
  const partCfg = part.l1.behaviorConfig ?? {};
  const hasPartCfg = Object.keys(partCfg).length > 0;
  const nextRuntime: any = {
    ...(state.runtimeConfig as any),
    algorithmStepUnit: part.l1.stepRate,
    activeBehavior: part.l1.behaviorId,
    scanMode: part.l2.scanMode,
    scanAxis: part.l2.scanAxis,
    scanUnit: part.l2.scanUnit,
    scanDirection: part.l2.scanDirection,
    scanSections: part.l2.scanSections ?? "1",
    eventEnabled: part.l2.eventEnabled,
    pitch: structuredClone(part.l2.pitch),
    x: structuredClone(part.l2.x),
    y: structuredClone(part.l2.y),
    behaviorConfig: {
      ...(state.runtimeConfig as any).behaviorConfig,
      ...(hasPartCfg ? { [part.l1.behaviorId]: { ...partCfg } } : {})
    }
  };
  return { ...state, runtimeConfig: nextRuntime, mappingConfig: overrideFromPart(state.mappingConfig, part) as any };
}

function syncActivePartFromLegacy<TState>(state: PlatformState<TState>): PlatformState<TState> {
  const active = clampPartIndex((state.runtimeConfig as any).activePartIndex ?? 0);
  const parts = Array.isArray((state.runtimeConfig as any).parts) ? [...((state.runtimeConfig as any).parts as any[])] : [];
  const current = parts[active];
  if (!current) return state;
  const behaviorId = String((state.runtimeConfig as any).activeBehavior ?? current.l1.behaviorId);
  const merged = preferMapping(state.mappingConfig, current);
  const nextPart = {
    ...current,
    l1: {
      ...current.l1,
      stepRate: (state.runtimeConfig as any).algorithmStepUnit,
      behaviorId,
      behaviorConfig: { ...((((state.runtimeConfig as any).behaviorConfig ?? {})[behaviorId] ?? {})) }
    },
    l2: {
      ...current.l2,
      scanMode: (state.runtimeConfig as any).scanMode,
      scanAxis: (state.runtimeConfig as any).scanAxis,
      scanUnit: (state.runtimeConfig as any).scanUnit,
      scanDirection: (state.runtimeConfig as any).scanDirection,
      scanSections: (state.runtimeConfig as any).scanSections ?? "1",
      eventEnabled: Boolean((state.runtimeConfig as any).eventEnabled),
      pitch: structuredClone((state.runtimeConfig as any).pitch),
      x: structuredClone((state.runtimeConfig as any).x),
      y: structuredClone((state.runtimeConfig as any).y),
      mapping: {
        activate: { action: merged.activate.action, slot: Number(merged.activate.channel) },
        stable: { action: merged.stable.action, slot: Number(merged.stable.channel) },
        deactivate: { action: merged.deactivate.action, slot: Number(merged.deactivate.channel) },
        scanned: { action: merged.scanned.action, slot: Number(merged.scanned.channel) },
        scanned_empty: { action: merged.scanned_empty.action, slot: Number(merged.scanned_empty.channel) }
      }
    }
  };
  parts[active] = nextPart;
  return { ...state, runtimeConfig: { ...(state.runtimeConfig as any), parts } as any };
}

type AutoNameRule = {
  match: RegExp;
  onSet: (rc: any, idx: number) => any;
};

type AutoNameToggle = {
  match: RegExp;
  deriveName: (rc: any, idx: number) => string;
};

const AUTO_NAME_RULES: AutoNameRule[] = [
  { match: /^instruments\.(\d+)\.type$/, onSet: (rc, idx) => { const inst = rc.instruments?.[idx]; return inst && inst.autoName === true ? writeValue(rc, `instruments.${idx}.name`, deriveInstAutoName(inst)) : rc; } },
  { match: /^parts\.(\d+)\.l1\.behaviorId$/, onSet: (rc, idx) => { const part = rc.parts?.[idx]; return part && part.autoName === true ? writeValue(rc, `parts.${idx}.name`, derivePartAutoName(part)) : rc; } },
  { match: /^mixer\.buses\.(\d+)\.(slot[12])\.type$/, onSet: (rc, idx) => { const bus = rc.mixer?.buses?.[idx]; return bus && bus.autoName === true ? writeValue(rc, `mixer.buses.${idx}.name`, deriveBusAutoName(bus)) : rc; } }
];

const AUTO_NAME_CLEAR: AutoNameRule[] = [
  { match: /^instruments\.(\d+)\.name$/, onSet: (rc, idx) => writeValue(rc, `instruments.${idx}.autoName`, false) },
  { match: /^parts\.(\d+)\.name$/, onSet: (rc, idx) => writeValue(rc, `parts.${idx}.autoName`, false) },
  { match: /^mixer\.buses\.(\d+)\.name$/, onSet: (rc, idx) => writeValue(rc, `mixer.buses.${idx}.autoName`, false) }
];

const AUTO_NAME_TOGGLE: AutoNameToggle[] = [
  { match: /^instruments\.(\d+)\.autoName$/, deriveName: (rc, idx) => { const inst = rc.instruments?.[idx]; return inst ? deriveInstAutoName(inst) : ""; } },
  { match: /^parts\.(\d+)\.autoName$/, deriveName: (rc, idx) => { const part = rc.parts?.[idx]; return part ? derivePartAutoName(part) : ""; } },
  { match: /^mixer\.buses\.(\d+)\.autoName$/, deriveName: (rc, idx) => { const bus = rc.mixer?.buses?.[idx]; return bus ? deriveBusAutoName(bus) : ""; } }
];

function applyAutoName<TState>(state: PlatformState<TState>, rc: any, key: string, setVal: unknown): PlatformState<TState> {
  for (const rule of AUTO_NAME_RULES) {
    const m = rule.match.exec(key);
    if (m) return { ...state, runtimeConfig: rule.onSet(rc, Number(m[1])) };
  }
  for (const rule of AUTO_NAME_CLEAR) {
    const m = rule.match.exec(key);
    if (m) return { ...state, runtimeConfig: rule.onSet(rc, Number(m[1])) };
  }
  for (const rule of AUTO_NAME_TOGGLE) {
    const m = rule.match.exec(key);
    if (m) {
      if (setVal === true) {
        const name = rule.deriveName(rc, Number(m[1]));
        if (name) rc = writeValue(rc, key.replace(/\.autoName$/, ".name"), name);
      }
      return { ...state, runtimeConfig: rc };
    }
  }
  return { ...state, runtimeConfig: rc };
}

export function readAnyValue<TState>(state: PlatformState<TState>, key: string): unknown {
  if (key.startsWith("transport.")) return readNestedValue(state.transport, key.slice("transport.".length));
  if (key.startsWith("mapping.")) return readNestedValue(state.mappingConfig, key.slice("mapping.".length));
  if (key.startsWith("system.")) return readNestedValue(state.system, key.slice("system.".length));
  const fxParamMatch = /^mixer\.buses\.(\d+)\.(slot[12])\.params\.([^.]+)$/.exec(key);
  if (fxParamMatch) {
    const value = readValue(state.runtimeConfig, key);
    const type = readValue(state.runtimeConfig, `mixer.buses.${fxParamMatch[1]}.${fxParamMatch[2]}.type`);
    const fallback = defaultFxParam(type, fxParamMatch[3]);
    if (fallback !== undefined && (value === undefined || (typeof fallback === "number" && !Number.isFinite(Number(value))))) return fallback;
    return value;
  }
  const globalFxParamMatch = /^mixer\.master\.slots\.(\d+)\.params\.([^.]+)$/.exec(key);
  if (globalFxParamMatch) {
    const value = readValue(state.runtimeConfig, key);
    const type = readValue(state.runtimeConfig, `mixer.master.slots.${globalFxParamMatch[1]}.type`);
    const fallback = defaultGlobalFxParam(type, globalFxParamMatch[2]);
    if (fallback !== undefined && (value === undefined || (typeof fallback === "number" && !Number.isFinite(Number(value))))) return fallback;
    return value;
  }
  return readValue(state.runtimeConfig, key);
}

export function writeAnyValue<TState>(state: PlatformState<TState>, key: string, value: unknown): PlatformState<TState> {
  if (key === "danceMode") {
    const danceMode = value === "none" || value === "mix" || value === "pan" || value === "fx" || value === "trigger-gate" || value === "xy"
      ? value
      : "none";
    return {
      ...state,
      runtimeConfig: writeValue(state.runtimeConfig, "danceMode", danceMode),
      system: { ...state.system, danceMode }
    };
  }
  if (key === "touchFx.selected.fxType") {
    const fxType = isMomentaryFxType(value) ? value : "none";
    const prevTargetKey = (state.runtimeConfig as any).touchFx?.selected?.targetKey ?? "master";
    return { ...state, runtimeConfig: writeValue(state.runtimeConfig, "touchFx.selected", { fxType, params: defaultMomentaryFxParams(fxType), targetKey: prevTargetKey }) };
  }
  const fxTypeMatch = /^mixer\.buses\.(\d+)\.(slot[12])\.type$/.exec(key);
  if (fxTypeMatch) {
    const type = isBusEffectType(value) ? value : "none";
    const busIdx = Number(fxTypeMatch[1]);
    let nextState = { ...state, runtimeConfig: writeValue(state.runtimeConfig, `mixer.buses.${busIdx}.${fxTypeMatch[2]}`, { type, params: defaultFxParams(type) }) };
    nextState = syncActivePartFromLegacy(nextState);
    return applyAutoName(nextState, nextState.runtimeConfig as any, key, value);
  }
  const globalFxTypeMatch = /^mixer\.master\.slots\.(\d+)\.type$/.exec(key);
  if (globalFxTypeMatch) {
    const type = isGlobalFxEffectType(value) ? value : "none";
    return syncActivePartFromLegacy({
      ...state,
      runtimeConfig: writeValue(state.runtimeConfig, `mixer.master.slots.${globalFxTypeMatch[1]}`, { type, params: defaultGlobalFxParams(type) })
    });
  }
  if (key.startsWith("transport.")) {
    const transport = writeNestedValue(state.transport, key.slice("transport.".length), value) as TransportFrame;
    return { ...state, transport };
  }
  if (key.startsWith("mapping.")) {
    const mappingConfig = writeNestedValue(state.mappingConfig, key.slice("mapping.".length), value) as MappingConfig;
    return syncActivePartFromLegacy({ ...state, mappingConfig });
  }
  if (key.startsWith("system.")) {
    const system = writeNestedValue(state.system, key.slice("system.".length), value) as SystemState;
    return { ...state, system };
  }
  if (key.startsWith("parts.")) {
    const normalized = key.endsWith(".slot") || key.endsWith(".sample.selectedSlot") ? Number(value) : value;
    let nextState = { ...state, runtimeConfig: writeValue(state.runtimeConfig, key, normalized) };
    const behMatch = key.endsWith(".l1.behaviorId");
    const autoNameMatch = /^parts\.\d+\.(name|autoName)$/.test(key);
    const active = clampPartIndex((nextState.runtimeConfig as any).activePartIndex ?? 0);
    if (behMatch || autoNameMatch) {
      nextState = applyAutoName(nextState, nextState.runtimeConfig as any, key, value);
    }
    if (key.startsWith(`parts.${active}.`)) {
      nextState = syncLegacyFromActivePart(nextState);
    }
    return nextState;
  }
  if (/^instruments\.\d+\.(name|autoName|type)$/.test(key)) {
    const normalized = key.endsWith(".sample.selectedSlot") ? Number(value) : value;
    let nextState = { ...state, runtimeConfig: writeValue(state.runtimeConfig, key, normalized) };
    nextState = applyAutoName(nextState, nextState.runtimeConfig as any, key, value);
    return syncActivePartFromLegacy(nextState);
  }
  if (/^mixer\.buses\.\d+\.(name|autoName)$/.test(key)) {
    const normalized = key.endsWith(".sample.selectedSlot") ? Number(value) : value;
    let nextState = { ...state, runtimeConfig: writeValue(state.runtimeConfig, key, normalized) };
    nextState = applyAutoName(nextState, nextState.runtimeConfig as any, key, value);
    return syncActivePartFromLegacy(nextState);
  }
  const normalized = key === "activePartIndex" || key.endsWith(".sample.selectedSlot") ? Number(value) : value;
  const nextState = { ...state, runtimeConfig: writeValue(state.runtimeConfig, key, normalized) };
  if (key === "activePartIndex") {
    return syncLegacyFromActivePart(nextState);
  }
  if (key.startsWith("mapping.")) {
    return syncActivePartFromLegacy(nextState);
  }
  return syncActivePartFromLegacy(nextState);
}
