import { getBehavior, type BehaviorEngine } from "@cellsymphony/behavior-api";
import type { MappingConfig } from "@cellsymphony/mapping-core";
import type { TransportFrame } from "@cellsymphony/device-contracts";
import { clamp, mod, readNestedValue, readValue, writeNestedValue, writeValue, deriveBusAutoName, derivePartAutoName, deriveInstAutoName, overrideFromPart, preferMapping } from "./coreUtils";
import { defaultFxParam, defaultFxParams, isBusEffectType } from "./fxDefaults";
import { defaultMomentaryFxParams, isMomentaryFxType } from "./momentaryFx";
import type { ConfigPayload, MenuNode, PlatformState, SystemState } from "./platformTypes";
import { clampPartIndex, PLATFORM_CAPS } from "./platformCaps";
import { SYNTH_PRESETS } from "./synthPresets";

type AnyState = PlatformState<any>;

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

export function textEditTurn<TState>(state: PlatformState<TState>, node: Extract<MenuNode, { kind: "text" }>, delta: -1 | 1): PlatformState<TState> {
  const raw = String(readAnyValue(state, node.key) ?? "");
  const cursor = clamp(state.system.nameCursor, 0, Math.max(0, node.maxLen));
  const safe = raw.slice(0, node.maxLen);
  const curPos = clamp(cursor, 0, safe.length);
  const charset = " ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";
  const chars = safe.split("");
  while (chars.length <= curPos) chars.push(" ");
  const current = chars[curPos] ?? " ";
  const idx = Math.max(0, charset.indexOf(current));
  const nextIdx = mod(idx + delta, charset.length);
  chars[curPos] = charset[nextIdx] ?? " ";
  const next = chars.join("").replace(/\s+$/g, "");
  return {
    ...state,
    system: { ...state.system, draftName: next, nameCursor: curPos }
  };
}

export function formatTimestamp(nowMs: number): string {
  const d = new Date(nowMs);
  const yyyy = d.getFullYear();
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  const hh = String(d.getHours()).padStart(2, "0");
  const min = String(d.getMinutes()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd} ${hh}${min}`;
}

export function factoryPayload<TState>(behavior: BehaviorEngine<TState, unknown>, createInitialState: (b: BehaviorEngine<TState, unknown>) => PlatformState<TState>, extractConfigPayload: (s: PlatformState<TState>) => ConfigPayload): ConfigPayload {
  const s = createInitialState(behavior);
  const rc: any = s.runtimeConfig;
  const parts: any[] = rc.parts;
  const instruments: any[] = rc.instruments;

  // P1: life with auto-spawn=12, activate→I1, routed→FX Bus 1
  parts[0].l1.behaviorId = "life";
  parts[0].l1.behaviorConfig = { ...((rc.behaviorConfig ?? {}).life ?? {}), randomCellsPerTick: 12, randomTickInterval: 1 };
  parts[0].l1.stepRate = "1/8";
  parts[0].l2.mapping = {
    activate: { action: "note_on", slot: 0 },
    stable: { action: "none", slot: 0 },
    deactivate: { action: "note_off", slot: 0 },
    scanned: { action: "none", slot: 0 },
    scanned_empty: { action: "note_off", slot: 0 }
  };
  parts[0].l2.scanAxis = "columns";
  parts[0].l2.eventEnabled = true;
  parts[0].name = "life";
  parts[0].autoName = true;
  parts[0].l1.triggerGates = Array.from({ length: PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight }, () => true);

  // P2: sequencer with horizontal scan, scanned→I2, routed direct
  parts[1].l1.behaviorId = "sequencer";
  parts[1].l1.behaviorConfig = {};
  parts[1].l1.stepRate = "1/4";
  parts[1].l2.mapping = {
    activate: { action: "none", slot: 0 },
    stable: { action: "none", slot: 0 },
    deactivate: { action: "none", slot: 0 },
    scanned: { action: "note_on", slot: 1 },
    scanned_empty: { action: "note_off", slot: 1 }
  };
  parts[1].l2.scanAxis = "rows";
  parts[1].l2.eventEnabled = true;
  parts[1].name = "sequencer";
  parts[1].autoName = true;
  parts[1].l1.triggerGates = Array.from({ length: PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight }, () => true);

  // P3–P8: "none", no triggers
  for (let i = 2; i < PLATFORM_CAPS.partCount; i += 1) {
    parts[i].l1.behaviorId = "none";
    parts[i].l1.behaviorConfig = {};
    parts[i].l2.mapping = {
      activate: { action: "none", slot: 0 },
      stable: { action: "none", slot: 0 },
      deactivate: { action: "none", slot: 0 },
      scanned: { action: "none", slot: 0 },
      scanned_empty: { action: "none", slot: 0 }
    };
    parts[i].l2.eventEnabled = false;
    parts[i].name = "(none)";
    parts[i].autoName = true;
    parts[i].l1.triggerGates = Array.from({ length: PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight }, () => true);
  }

  // I1: synth with soft pad preset, routed→FX Bus 1 (fx_bus_1 → bus index 0)
  instruments[0].type = "synth";
  instruments[0].synth = structuredClone(SYNTH_PRESETS[1]!.synth);
  instruments[0].mixer = { route: "fx_bus_1", panPos: 0, volume: 100 };
  instruments[0].name = "synth";
  instruments[0].autoName = true;
  instruments[0].noteBehavior = "oneshot";

  // I2: drum kit (perc hit), routed direct
  instruments[1].type = "synth";
  instruments[1].synth = structuredClone(SYNTH_PRESETS[7]!.synth);
  instruments[1].mixer = { route: "direct", panPos: 0, volume: 100 };
  instruments[1].name = "drums";
  instruments[1].autoName = true;
  instruments[1].noteBehavior = "oneshot";

  // I3–I8: "none"
  for (let i = 2; i < PLATFORM_CAPS.instrumentCount; i += 1) {
    instruments[i].type = "none";
    instruments[i].mixer = { route: "direct", panPos: 0, volume: 100 };
    instruments[i].name = "(none)";
    instruments[i].autoName = true;
  }

  // FX Bus 1 (bus 0): delay + duck sourcing I2
  rc.mixer.buses[0] = {
    slot1: { type: "delay", params: { timeMs: 280, feedbackPct: 38, mixPct: 45 } },
    slot2: { type: "duck", params: { source: "I2", threshold: 0.08, amountPct: 60, attackMs: 8, releaseMs: 160 } },
    panPos: 0,
    autoName: true,
    name: "Bus 1"
  };

  // All other FX buses: no effects
  for (let i = 1; i < PLATFORM_CAPS.busCount; i += 1) {
    rc.mixer.buses[i] = {
      slot1: { type: "none", params: {} },
      slot2: { type: "none", params: {} },
      panPos: 0,
      autoName: true,
      name: "(none)"
    };
  }

  // Align active part and behavior
  rc.activePartIndex = 0;
  rc.activeBehavior = "life";

  // Reinitialize part states to match new config
  const partStates: any[] = [];
  for (const p of parts) {
    const engine = getBehavior(p.l1.behaviorId) ?? behavior;
    partStates.push(engine.init({ ...(p.l1.behaviorConfig ?? {}) }));
  }
  s.partStates = partStates;
  s.behaviorState = partStates[0];
  s.activeBehavior = "life";

  return extractConfigPayload({ ...s, runtimeConfig: rc });
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
  return readValue(state.runtimeConfig, key);
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

export function writeAnyValue<TState>(state: PlatformState<TState>, key: string, value: unknown): PlatformState<TState> {
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

export function reinitBehaviorState<TState>(
  state: PlatformState<TState>,
  key: string,
  resolveBehavior: (id: string) => BehaviorEngine<any, any>
): PlatformState<TState> {
  const previousBehaviorId = state.activeBehavior;
  if (key === "activePartIndex") {
    const targetPartIndex = clampPartIndex((state.runtimeConfig as any).activePartIndex ?? 0);
    const partBehaviorId = String((state.runtimeConfig as any).parts?.[targetPartIndex]?.l1?.behaviorId ?? state.runtimeConfig.activeBehavior);
    const partStates = Array.isArray((state as any).partStates) ? ((state as any).partStates as any[]) : [];
    const partState = partStates[targetPartIndex];
    return {
      ...(state as any),
      activeBehavior: partBehaviorId,
      behaviorState: (partState ?? state.behaviorState) as TState
    } as PlatformState<TState>;
  }
  const parts = key.split(".");
  let behaviorId = parts[1] ?? state.runtimeConfig.activeBehavior;
  let ns = state.runtimeConfig.behaviorConfig?.[behaviorId] as Record<string, unknown> | undefined;
  let targetPartIndex = clampPartIndex((state.runtimeConfig as any).activePartIndex ?? 0);
  if (key.startsWith("parts.")) {
    targetPartIndex = clampPartIndex(parts[1] ?? 0);
    behaviorId = String((state.runtimeConfig as any).parts?.[targetPartIndex]?.l1?.behaviorId ?? state.runtimeConfig.activeBehavior);
    ns = ((state.runtimeConfig as any).parts?.[targetPartIndex]?.l1?.behaviorConfig ?? {}) as Record<string, unknown>;
  }
  const behavior = resolveBehavior(behaviorId);
  const cfg: any = {};
  if (behavior.configMenu) {
    for (const item of behavior.configMenu(behavior.init({}))) {
      const val = ns?.[item.key];
      if (val !== undefined) cfg[item.key] = val;
    }
  }
  const next = { ...state } as any;
  next.behaviorState = behavior.init(cfg);
  next.activeBehavior = behaviorId;
  if (key.endsWith(".l1.behaviorId")) {
    next.runtimeConfig = {
      ...next.runtimeConfig,
      activeBehavior: behaviorId,
      behaviorConfig: {
        ...(next.runtimeConfig.behaviorConfig ?? {}),
        [behaviorId]: { ...((next.runtimeConfig.behaviorConfig ?? {})[behaviorId] ?? {}) }
      }
    };
    const part = next.runtimeConfig.parts?.[targetPartIndex];
    if (part) {
      const seeded = { ...((next.runtimeConfig.behaviorConfig ?? {})[behaviorId] ?? {}) };
      part.l1 = { ...part.l1, behaviorId, behaviorConfig: seeded };
    }
  }
  if (Array.isArray(next.partStates) && next.partStates.length > targetPartIndex) {
    next.partStates[targetPartIndex] = next.behaviorState;
  }
  if (key === "activeBehavior" || key.endsWith(".l1.behaviorId")) {
    if (key.endsWith(".l1.behaviorId")) {
      next.runtimeConfig = {
        ...next.runtimeConfig,
        parts: remapPartParamModsForBehavior(next.runtimeConfig.parts ?? [], targetPartIndex, behaviorId, resolveBehavior)
      };
    }
    next.system = {
      ...next.system,
      auxBindings: remapAuxBindingsForBehavior(next.system.auxBindings, previousBehaviorId, behaviorId, resolveBehavior)
    };
  }
  return next as PlatformState<TState>;
}

const BEHAVIOR_PARAM_ANALOGUES = [
  ["randomTickInterval", "seedInterval", "autoSpawnInterval", "spawnInterval", "autoPulseInterval", "autoDropInterval"],
  ["randomCellsPerTick", "randomSeedCells", "maxAnts", "maxBalls"]
];

function remapPartParamModsForBehavior(
  parts: any[],
  partIndex: number,
  toBehaviorId: string,
  resolveBehavior: (id: string) => BehaviorEngine<any, any>
): any[] {
  if (!Array.isArray(parts) || !parts[partIndex]?.paramMods) return parts;
  const next = [...parts];
  const part = { ...next[partIndex] };
  const paramMods = structuredClone(part.paramMods);
  for (const axis of ["x", "y"] as const) {
    for (let slotIdx = 0; slotIdx < 2; slotIdx += 1) {
      const slot = paramMods?.[axis]?.[slotIdx];
      if (!slot || typeof slot.key !== "string") continue;
      const match = new RegExp(`^parts\\.${partIndex}\\.l1\\.behaviorConfig\\.([^.]+)$`).exec(slot.key);
      if (!match) continue;
      paramMods[axis][slotIdx] = remapBehaviorTurnBindingForBehavior(slot, toBehaviorId, resolveBehavior, partIndex);
    }
  }
  next[partIndex] = { ...part, paramMods };
  return next;
}

function behaviorParamAnalogue(paramKey: string, behaviorId: string, resolveBehavior: (id: string) => BehaviorEngine<any, any>): any | null {
  const behavior = resolveBehavior(behaviorId);
  if (!behavior.configMenu) return null;
  const items = behavior.configMenu(behavior.init({}));
  const keys = new Set<string>();
  for (const group of BEHAVIOR_PARAM_ANALOGUES) {
    if (group.includes(paramKey)) for (const k of group) keys.add(k);
  }
  if (keys.size === 0) keys.add(paramKey);
  for (const item of items) {
    if (!keys.has(item.key)) continue;
    if (item.type === "number") return { key: item.key, label: item.label, kind: "number", min: item.min ?? 0, max: item.max ?? 127, step: item.step ?? 1 };
    if (item.type === "enum") return { key: item.key, label: item.label, kind: "enum", options: item.options ?? [] };
    if (item.type === "bool") return { key: item.key, label: item.label, kind: "bool" };
  }
  return null;
}

function remapBehaviorTurnBindingForBehavior(binding: any, toBehaviorId: string, resolveBehavior: (id: string) => BehaviorEngine<any, any>, partIndex?: number): any {
  if (!binding?.key || typeof binding.key !== "string") return binding;
  const partMatch = /^parts\.(\d+)\.l1\.behaviorConfig\.([^.]+)$/.exec(binding.key);
  const rootMatch = /^behaviorConfig\.([^.]+)\.([^.]+)$/.exec(binding.key);
  const paramKey = partMatch?.[2] ?? rootMatch?.[2];
  if (!paramKey) return binding;
  const analogue = behaviorParamAnalogue(paramKey, toBehaviorId, resolveBehavior);
  if (!analogue) return binding;
  if (partMatch) {
    const idx = partIndex ?? Number(partMatch[1]);
    return { ...binding, ...analogue, key: `parts.${idx}.l1.behaviorConfig.${analogue.key}` };
  }
  return { ...binding, ...analogue, key: `behaviorConfig.${toBehaviorId}.${analogue.key}` };
}

function primaryBehaviorAction(behaviorId: string, resolveBehavior: (id: string) => BehaviorEngine<any, any>): { actionType: string; label: string } | null {
  const behavior = resolveBehavior(behaviorId);
  if (!behavior.configMenu) return null;
  const items = behavior.configMenu(behavior.init({}));
  for (const item of items) {
    if (item.type === "action") return { actionType: item.key, label: item.label };
  }
  return null;
}

function remapAuxBindingsForBehavior(
  bindings: Record<string, any>,
  fromBehaviorId: string,
  toBehaviorId: string,
  resolveBehavior: (id: string) => BehaviorEngine<any, any>
): Record<string, any> {
  if (fromBehaviorId === toBehaviorId) return bindings;
  const fromAction = primaryBehaviorAction(fromBehaviorId, resolveBehavior);
  const toAction = primaryBehaviorAction(toBehaviorId, resolveBehavior);

  const next: Record<string, any> = { ...bindings };
  for (const id of Object.keys(next)) {
    const binding = next[id];
    if (!binding) continue;
    let nextBinding = binding.turn ? { ...binding, turn: remapBehaviorTurnBindingForBehavior(binding.turn, toBehaviorId, resolveBehavior) } : binding;
    if (fromAction && nextBinding?.press?.kind === "behavior_action" && !nextBinding.press.routeKey && nextBinding.press.actionType === fromAction.actionType) {
      if (!toAction) nextBinding = nextBinding.turn ? { turn: nextBinding.turn, press: null } : null;
      else nextBinding = { ...nextBinding, press: { kind: "behavior_action", actionType: toAction.actionType, label: toAction.label } };
    }
    next[id] = nextBinding;
  }
  return next;
}
