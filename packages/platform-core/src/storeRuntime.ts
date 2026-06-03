import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import { getBehavior } from "@cellsymphony/behavior-api";
import type { MappingConfig } from "@cellsymphony/mapping-core";
import type { ConfigPayload, PlatformEffect, PlatformState, RuntimeConfig, StoreResult } from "./index";
import { isBusEffectType, sanitizeFxParams } from "./fxDefaults";
import { clampPanPosition, clampPartIndex, isValidSectionValue, PAN_POSITION_COUNT, PLATFORM_CAPS, scalePanPosition } from "./platformCaps";
import { cutoffHzToDisplay, overrideFromPart, preferMapping } from "./coreUtils";
import { makeToast } from "./toast";
import { DEFAULT_VELOCITY_LEVELS, DEFAULT_MIDI_ENGINE, DEFAULT_PAN_POS, DEFAULT_VELOCITY, DEFAULT_VOLUME } from "./runtimeDefaults";
import { cloneAuxBindings, sanitizeAuxBindings } from "./auxBindingsSerde";
import { normalizeParamMods } from "./paramMod";

type StoreDeps<TState> = {
  resolveBehavior: (id: string) => BehaviorEngine<any, any>;
  factoryPayload: (behavior: BehaviorEngine<TState, unknown>) => ConfigPayload;
};

export function extractConfigPayload<TState>(state: PlatformState<TState>): ConfigPayload {
  const runtimeAny: any = state.runtimeConfig;
  const active = clampPartIndex(runtimeAny.activePartIndex ?? 0);
  const parts = Array.isArray(runtimeAny.parts) ? [...runtimeAny.parts] : [];
  const partStates: unknown[] = Array.isArray((state as any).partStates) ? [...((state as any).partStates as unknown[])] : [];
  for (let i = 0; i < parts.length; i += 1) {
    const part = parts[i];
    const behaviorId = String(part?.l1?.behaviorId ?? runtimeAny.activeBehavior ?? state.activeBehavior);
    const engine = getBehavior(behaviorId);
    const saveGridState = part?.l1?.saveGridState !== false;
    const savedState = saveGridState && engine && partStates[i] !== undefined ? engine.serialize(partStates[i] as any) : undefined;
    parts[i] = {
      ...part,
      l1: {
        ...part.l1,
        saveGridState,
        ...(savedState === undefined ? {} : { savedState })
      }
    };
    if (!saveGridState && parts[i]?.l1 && "savedState" in parts[i].l1) {
      delete parts[i].l1.savedState;
    }
  }
  if (parts[active]) {
    const behaviorId = String(runtimeAny.activeBehavior ?? parts[active].l1?.behaviorId ?? "life");
    parts[active] = {
      ...parts[active],
      l1: {
        ...parts[active].l1,
        stepRate: runtimeAny.algorithmStepUnit,
        behaviorId,
        behaviorConfig: { ...((runtimeAny.behaviorConfig ?? {})[behaviorId] ?? {}) }
      },
      l2: {
        ...parts[active].l2,
        scanMode: runtimeAny.scanMode,
        scanAxis: runtimeAny.scanAxis,
        scanUnit: runtimeAny.scanUnit,
        scanDirection: runtimeAny.scanDirection,
        eventEnabled: runtimeAny.eventEnabled,
        pitch: structuredClone(runtimeAny.pitch),
        x: structuredClone(runtimeAny.x),
        y: structuredClone(runtimeAny.y),
        mapping: (() => {
          const merged = preferMapping(state.mappingConfig as any, parts[active]);
          return {
            activate: { action: merged.activate.action, slot: Number(merged.activate.channel) },
            stable: { action: merged.stable.action, slot: Number(merged.stable.channel) },
            deactivate: { action: merged.deactivate.action, slot: Number(merged.deactivate.channel) },
            scanned: { action: merged.scanned.action, slot: Number(merged.scanned.channel) },
            scanned_empty: { action: merged.scanned_empty.action, slot: Number(merged.scanned_empty.channel) }
          };
        })()
      }
    };
  }
  const runtimeConfig = { ...(state.runtimeConfig as any), parts, auxBindings: cloneAuxBindings((state.system as any).auxBindings) } as RuntimeConfig;
  return {
    activeBehavior: runtimeAny.activeBehavior ?? state.activeBehavior,
    runtimeConfig,
    mappingConfig: state.mappingConfig
  };
}

export function applyConfigPayload<TState>(
  state: PlatformState<TState>,
  payload: ConfigPayload,
  behavior: BehaviorEngine<TState, unknown>,
  deps: StoreDeps<TState>
): PlatformState<TState> {
  const safe = sanitizePayload(payload, behavior, deps);
  const next = { ...state } as any;
  next.runtimeConfig = safe.runtimeConfig;
  next.mappingConfig = safe.mappingConfig;
  const activePartIndex = clampPartIndex((safe.runtimeConfig as any).activePartIndex ?? 0);
  const activePart = (safe.runtimeConfig as any).parts?.[activePartIndex];
  const activePartBehaviorId = String(activePart?.l1?.behaviorId ?? safe.activeBehavior);
  next.activeBehavior = safe.activeBehavior;
  const resolved = deps.resolveBehavior(safe.activeBehavior);
  const behaviorCfgSource = ((safe.runtimeConfig as any).behaviorConfig?.[resolved.id] ?? {}) as Record<string, unknown>;
  const behaviorCfg: Record<string, unknown> = {};
  if (resolved.configMenu) {
    for (const item of resolved.configMenu(resolved.init({}))) {
      const v = behaviorCfgSource[item.key];
      if (v !== undefined) behaviorCfg[item.key] = v;
    }
  }
  next.behaviorState = resolved.init(behaviorCfg);
  const parts: any[] = Array.isArray((safe.runtimeConfig as any).parts) ? (safe.runtimeConfig as any).parts : [];
  next.partStates = parts.map((part) => {
    const engine = deps.resolveBehavior(String(part?.l1?.behaviorId ?? safe.activeBehavior));
    const saveGridState = part?.l1?.saveGridState !== false;
    const savedState = part?.l1?.savedState;
    if (saveGridState && savedState !== undefined) {
      return engine.deserialize(savedState);
    }
    return engine.init({ ...(part?.l1?.behaviorConfig ?? {}) });
  });
  while (next.partStates.length < PLATFORM_CAPS.partCount) next.partStates.push(resolved.init({}));

  next.system = { ...next.system, auxBindings: cloneAuxBindings((safe.runtimeConfig as any).auxBindings) };
  const shouldUseRestoredActiveState = activePartBehaviorId === safe.activeBehavior && activePart?.l1?.saveGridState !== false && activePart?.l1?.savedState !== undefined;
  if (shouldUseRestoredActiveState) {
    next.behaviorState = (next.partStates[activePartIndex] ?? next.behaviorState) as TState;
  }
  next.partScanIndex = Array.from({ length: PLATFORM_CAPS.partCount }, () => 0);
  next.partScanPulseAccumulator = Array.from({ length: PLATFORM_CAPS.partCount }, () => 0);
  next.partAlgorithmPulseAccumulator = Array.from({ length: PLATFORM_CAPS.partCount }, () => 0);
  next.scanPulseAccumulator = 0;
  next.algorithmPulseAccumulator = 0;
  next.ppqnPulseRemainder = 0;
  next.scanIndex = 0;
  next.system = {
    ...next.system,
    heldNotes: [],
    pendingResync: false,
    externalPpqnPulse: 0,
    sampleAssign: null,
    sampleBrowser: null,
    fxAssignMode: null,
    activeFx: [],
    touchMode: "none"
  };
  return next as PlatformState<TState>;
}

function sanitizePanPos(value: unknown, fallback: unknown, incomingPanPositions: unknown): number {
  const raw = value ?? fallback ?? DEFAULT_PAN_POS;
  const sourceCount = Number(incomingPanPositions);
  const oldCenterLeft = Math.floor((PLATFORM_CAPS.gridWidth - 1) / 2);
  const oldCenterRight = Math.floor(PLATFORM_CAPS.gridWidth / 2);
  if ((sourceCount === PLATFORM_CAPS.gridWidth || incomingPanPositions === undefined) && (Number(raw) === oldCenterLeft || Number(raw) === oldCenterRight)) return DEFAULT_PAN_POS;
  if (Number.isInteger(sourceCount) && sourceCount > 1 && sourceCount !== PAN_POSITION_COUNT) return scalePanPosition(raw, sourceCount);
  if (incomingPanPositions === undefined && Number(raw) >= 0 && Number(raw) <= PLATFORM_CAPS.gridWidth - 1) return scalePanPosition(raw, PLATFORM_CAPS.gridWidth);
  return clampPanPosition(raw);
}

function sanitizeInstruments(incoming: any, factory: any, incomingPanPositions: unknown): any[] {
  const factorySlots: any[] = Array.isArray((factory.runtimeConfig as any).instruments)
    ? (factory.runtimeConfig as any).instruments
    : [];
  const baseSlots = factorySlots.length > 0 ? factorySlots : Array.from({ length: PLATFORM_CAPS.instrumentCount }, () => ({ type: "synth", midi: { enabled: false, channel: 0 }, synth: {}, sample: { baseVelocity: DEFAULT_VELOCITY, velocityLevelsEnabled: false, velocityLevels: { ...DEFAULT_VELOCITY_LEVELS }, selectedSlot: 0, slots: Array.from({ length: PLATFORM_CAPS.sampleSlotCount }, () => ({ path: null })), tuneSemis: 0, amp: {}, ampEnv: {}, filter: {}, filterEnv: {}, assignments: [] }, midiEngine: { ...DEFAULT_MIDI_ENGINE }, mixer: { route: "direct", panPos: DEFAULT_PAN_POS, volume: DEFAULT_VOLUME } }));
  const src = Array.isArray(incoming) ? incoming : [];
  const out: any[] = [];
  for (let i = 0; i < PLATFORM_CAPS.instrumentCount; i += 1) {
    const f = baseSlots[i] ?? baseSlots[0] ?? { type: "synth", midi: { enabled: false, channel: i }, synth: {}, sample: { baseVelocity: DEFAULT_VELOCITY, velocityLevelsEnabled: false, velocityLevels: { ...DEFAULT_VELOCITY_LEVELS }, selectedSlot: 0, slots: Array.from({ length: PLATFORM_CAPS.sampleSlotCount }, () => ({ path: null })), tuneSemis: 0, amp: {}, ampEnv: {}, filter: {}, filterEnv: {}, assignments: [] }, midiEngine: { ...DEFAULT_MIDI_ENGINE }, mixer: { route: "direct", panPos: DEFAULT_PAN_POS, volume: DEFAULT_VOLUME } };
    const s = src[i] ?? {};
    const incomingAutoName = typeof (s as any).autoName === "boolean" ? (s as any).autoName : true;
    const incomingName = typeof (s as any).name === "string" && (s as any).name.trim().length > 0 ? (s as any).name.trim() : "";
    const fallbackAutoName = typeof (f as any).autoName === "boolean" ? (f as any).autoName : true;
    const fallbackName = typeof (f as any).name === "string" && (f as any).name.trim().length > 0 ? (f as any).name.trim() : "";
    out.push({
    ...(f as any),
     ...(s as any),
     type: (s as any).type === "sampler" || (s as any).type === "midi" || (s as any).type === "synth" || (s as any).type === "none" ? (s as any).type : (f as any).type,
      autoName: incomingAutoName,
      name: incomingName || fallbackName || (f as any).name || "synth",
      midi: { ...(f as any).midi, ...((s as any).midi ?? {}) },
      synth: {
        ...(f as any).synth,
        ...((s as any).synth ?? {}),
        osc1: { ...(f as any).synth?.osc1, ...((s as any).synth?.osc1 ?? {}) },
        osc2: { ...(f as any).synth?.osc2, ...((s as any).synth?.osc2 ?? {}) },
        amp: { ...(f as any).synth?.amp, ...((s as any).synth?.amp ?? {}) },
        ampEnv: { ...(f as any).synth?.ampEnv, ...((s as any).synth?.ampEnv ?? {}) },
        filter: { ...(f as any).synth?.filter, ...((s as any).synth?.filter ?? {}) },
        filterEnv: { ...(f as any).synth?.filterEnv, ...((s as any).synth?.filterEnv ?? {}) }
      },
      sample: {
        ...(f as any).sample,
        ...((s as any).sample ?? {}),
        velocityLevels: { ...(f as any).sample?.velocityLevels, ...((s as any).sample?.velocityLevels ?? {}) },
        slots: (() => {
          const incomingSlots = Array.isArray((s as any).sample?.slots)
            ? (s as any).sample.slots.slice(0, PLATFORM_CAPS.sampleSlotCount).map((entry: any) => ({ path: typeof entry?.path === "string" ? entry.path : null }))
            : (Array.isArray((f as any).sample?.slots) ? (f as any).sample.slots.slice(0, PLATFORM_CAPS.sampleSlotCount).map((entry: any) => ({ path: typeof entry?.path === "string" ? entry.path : null })) : []);
          while (incomingSlots.length < PLATFORM_CAPS.sampleSlotCount) incomingSlots.push({ path: null });
          return incomingSlots;
        })(),
        assignments: Array.isArray((s as any).sample?.assignments) ? (s as any).sample.assignments : (Array.isArray((f as any).sample?.assignments) ? (f as any).sample.assignments : []),
        amp: { ...(f as any).sample?.amp, ...((s as any).sample?.amp ?? {}) },
        ampEnv: { ...(f as any).sample?.ampEnv, ...((s as any).sample?.ampEnv ?? {}) },
        filter: { ...(f as any).sample?.filter, ...((s as any).sample?.filter ?? {}) },
        filterEnv: { ...(f as any).sample?.filterEnv, ...((s as any).sample?.filterEnv ?? {}) }
      },
      midiEngine: {
        ...(f as any).midiEngine,
        ...((s as any).midiEngine ?? {})
      },
      mixer: {
        route: (() => {
          const raw = String((s as any).mixer?.route ?? (f as any).mixer?.route ?? "direct");
          if (raw === "direct") return "direct";
          const m = /^(?:fx_)?bus_(\d+)$/.exec(raw);
          if (!m) return "direct";
          const idx = Number(m[1]);
          if (!Number.isFinite(idx) || idx < 1 || idx > PLATFORM_CAPS.busCount) return "direct";
          return `fx_bus_${idx}`;
        })(),
        panPos: sanitizePanPos((s as any).mixer?.panPos, (f as any).mixer?.panPos, incomingPanPositions),
        volume: Math.max(0, Math.min(100, Number((s as any).mixer?.volume ?? (f as any).mixer?.volume ?? DEFAULT_VOLUME)))
      }
    });
     const inst = out[i] as Record<string, any>;
     for (const prefix of ["synth", "sampler"]) {
      const section = inst[prefix] as Record<string, any> | undefined;
      const filter = section?.filter;
      if (filter && typeof filter.cutoffHz === "number" && filter.cutoffHz > 255) {
        section.filter = { ...filter, cutoffHz: cutoffHzToDisplay(filter.cutoffHz) };
      }
    }
  }
  return out;
}

const BUS_EFFECT_TYPES = new Set([
  "none", "reverb", "delay", "tremolo", "vibrato", "auto_pan",
  "chorus", "flanger", "wah", "filter_lfo", "duck", "bitcrusher",
  "saturator", "distortion", "glitch", "compressor", "eq"
]);

function normalizeSlot(raw: any): any {
  if (typeof raw === "string") {
    const type = BUS_EFFECT_TYPES.has(raw) ? raw : "none";
    return { type, params: sanitizeFxParams(type, {}) };
  }
  const typeRaw = typeof raw?.type === "string" ? raw.type : "none";
  const type = isBusEffectType(typeRaw) && BUS_EFFECT_TYPES.has(typeRaw) ? typeRaw : "none";
  const params = sanitizeFxParams(type, raw?.params);
  return { type, params };
}

function sanitizeMixer(incoming: any, factory: any, incomingPanPositions: unknown): any {
  const factoryMixer = (factory.runtimeConfig as any).mixer;
  const sourceBuses = Array.isArray(incoming?.buses) ? incoming.buses : (Array.isArray(factoryMixer?.buses) ? factoryMixer.buses : []);
  const buses: any[] = [];
  for (let i = 0; i < PLATFORM_CAPS.busCount; i += 1) {
    const src = sourceBuses[i] ?? {};
    const autoName = typeof src.autoName === "boolean" ? src.autoName : true;
    const srcName = typeof src.name === "string" && src.name.trim().length > 0 ? src.name.trim() : "(none)";
    buses.push({
      slot1: normalizeSlot(src.slot1),
      slot2: normalizeSlot(src.slot2),
      panPos: sanitizePanPos(src.panPos, DEFAULT_PAN_POS, incomingPanPositions),
      autoName,
      name: srcName
    });
  }
  return { buses };
}

function sanitizePayload<TState>(payload: ConfigPayload, behavior: BehaviorEngine<TState, unknown>, deps: StoreDeps<TState>): ConfigPayload {
  const factory = deps.factoryPayload(behavior);
  const p: any = payload ?? {};
  const rt: any = p.runtimeConfig ?? {};

  const mergedRuntime: RuntimeConfig = {
    ...(factory.runtimeConfig as any),
    ...(rt as any),
    midi: { ...(factory.runtimeConfig as any).midi, ...(rt.midi ?? {}) },
    sound: { ...(factory.runtimeConfig as any).sound, ...(rt.sound ?? {}) },
    pitch: { ...(factory.runtimeConfig.pitch as any), ...(rt.pitch ?? {}) },
    x: {
      ...(factory.runtimeConfig.x as any),
      ...(rt.x ?? {}),
      pitch: { ...(factory.runtimeConfig.x.pitch as any), ...(rt.x?.pitch ?? {}) },
      velocity: { ...(factory.runtimeConfig.x.velocity as any), ...(rt.x?.velocity ?? {}) },
      filterCutoff: { ...(factory.runtimeConfig.x.filterCutoff as any), ...(rt.x?.filterCutoff ?? {}) },
      filterResonance: { ...(factory.runtimeConfig.x.filterResonance as any), ...(rt.x?.filterResonance ?? {}) }
    },
    y: {
      ...(factory.runtimeConfig.y as any),
      ...(rt.y ?? {}),
      pitch: { ...(factory.runtimeConfig.y.pitch as any), ...(rt.y?.pitch ?? {}) },
      velocity: { ...(factory.runtimeConfig.y.velocity as any), ...(rt.y?.velocity ?? {}) },
      filterCutoff: { ...(factory.runtimeConfig.y.filterCutoff as any), ...(rt.y?.filterCutoff ?? {}) },
      filterResonance: { ...(factory.runtimeConfig.y.filterResonance as any), ...(rt.y?.filterResonance ?? {}) }
    },
    numericDisplayMode: rt.numericDisplayMode === "bar" || rt.numericDisplayMode === "numbers" || rt.numericDisplayMode === "bar+numbers"
      ? rt.numericDisplayMode
      : typeof rt.showNumericValueWithBars === "boolean" ? (rt.showNumericValueWithBars ? "bar+numbers" : "bar") : "bar+numbers",
    activePartIndex: clampPartIndex(rt.activePartIndex ?? (factory.runtimeConfig as any).activePartIndex ?? 0),
    panPositions: PAN_POSITION_COUNT,
    parts: Array.isArray(rt.parts) ? rt.parts : Array.isArray((factory.runtimeConfig as any).parts) ? (factory.runtimeConfig as any).parts : [],
    instruments: sanitizeInstruments(rt.instruments, factory, rt.panPositions),
    mixer: sanitizeMixer(rt.mixer, factory, rt.panPositions),
    auxBindings: sanitizeAuxBindings(rt.auxBindings)
  };

  const voiceStealingMode = (mergedRuntime as any).sound?.voiceStealingMode;
  if (voiceStealingMode !== "off" && voiceStealingMode !== "lenient" && voiceStealingMode !== "balanced" && voiceStealingMode !== "aggressive") (mergedRuntime as any).sound.voiceStealingMode = (factory.runtimeConfig as any).sound?.voiceStealingMode ?? "balanced";

  if (!Array.isArray((mergedRuntime as any).parts) || (mergedRuntime as any).parts.length === 0) (mergedRuntime as any).parts = Array.isArray((factory.runtimeConfig as any).parts) ? structuredClone((factory.runtimeConfig as any).parts) : [];
  const active = clampPartIndex((mergedRuntime as any).activePartIndex ?? 0);
  const parts = [...((mergedRuntime as any).parts as any[])];
  while (parts.length < PLATFORM_CAPS.partCount) {
    const base = (factory.runtimeConfig as any).parts?.[parts.length] ?? (factory.runtimeConfig as any).parts?.[0];
    if (base) parts.push(structuredClone(base));
    else break;
  }
  if (parts[active]) {
    const p = parts[active];
    mergedRuntime.algorithmStepUnit = p.l1?.stepRate ?? mergedRuntime.algorithmStepUnit;
    mergedRuntime.activeBehavior = p.l1?.behaviorId ?? mergedRuntime.activeBehavior;
    mergedRuntime.scanMode = p.l2?.scanMode ?? mergedRuntime.scanMode;
    mergedRuntime.scanAxis = p.l2?.scanAxis ?? mergedRuntime.scanAxis;
    mergedRuntime.scanUnit = p.l2?.scanUnit ?? mergedRuntime.scanUnit;
    mergedRuntime.scanDirection = p.l2?.scanDirection ?? mergedRuntime.scanDirection;
    mergedRuntime.scanSections = p.l2?.scanSections ?? mergedRuntime.scanSections ?? "1";
    mergedRuntime.eventEnabled = p.l2?.eventEnabled ?? mergedRuntime.eventEnabled;
    mergedRuntime.pitch = p.l2?.pitch ? structuredClone(p.l2.pitch) : mergedRuntime.pitch;
    mergedRuntime.x = p.l2?.x ? structuredClone(p.l2.x) : mergedRuntime.x;
    mergedRuntime.y = p.l2?.y ? structuredClone(p.l2.y) : mergedRuntime.y;
    const behaviorFromPayload = ((rt.behaviorConfig ?? {}) as any)[mergedRuntime.activeBehavior];
    mergedRuntime.behaviorConfig = {
      ...(mergedRuntime.behaviorConfig as any),
      [mergedRuntime.activeBehavior]: { ...(behaviorFromPayload ?? p.l1?.behaviorConfig ?? {}) }
    };
    if (behaviorFromPayload) {
      parts[active] = {
        ...p,
        l1: {
          ...p.l1,
          behaviorId: mergedRuntime.activeBehavior,
          behaviorConfig: { ...behaviorFromPayload }
        }
      };
    }
  }
  for (let i = 0; i < parts.length; i += 1) {
    const part = parts[i];
    if (!part?.l1) continue;
    const basePart = (factory.runtimeConfig as any).parts?.[i] ?? (factory.runtimeConfig as any).parts?.[0] ?? {};
    parts[i] = {
      ...part,
      l1: {
        ...part.l1,
        saveGridState: part.l1.saveGridState !== false,
        triggerGates: Array.isArray(part.l1.triggerGates) ? part.l1.triggerGates : part.l1.triggerGates ?? Array.from({ length: PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight }, () => true)
      },
      l2: sanitizePartL2(part.l2, basePart.l2),
      paramMods: normalizeParamMods((part as any).paramMods),
      autoName: typeof (part as any).autoName === "boolean" ? (part as any).autoName : true,
      name: typeof (part as any).name === "string" && (part as any).name.trim().length > 0 ? (part as any).name.trim() : (part as any).l1?.behaviorId ?? "life"
    };
  }
  (mergedRuntime as any).parts = parts;

  const mappingConfig = p.mappingConfig ? (p.mappingConfig as MappingConfig) : factory.mappingConfig;
  const activePart = (mergedRuntime as any).parts?.[active];
  const mergedMapping = activePart?.l2?.mapping
    ? overrideFromPart(mappingConfig, activePart)
    : mappingConfig;

  return {
    activeBehavior: typeof p.activeBehavior === "string" ? p.activeBehavior : factory.activeBehavior,
    runtimeConfig: mergedRuntime,
    mappingConfig: mergedMapping
  };
}


function sanitizePartL2(partL2: any, baseL2: any): any {
  const base = baseL2 ?? {};
  const l2 = partL2 ?? {};
  return {
    ...base,
    ...l2,
    scanSections: isValidSectionValue(l2.scanSections) ? String(Number(l2.scanSections)) : base.scanSections ?? "1",
    pitch: { ...(base.pitch ?? {}), ...(l2.pitch ?? {}) },
    x: sanitizeAxis(l2.x, base.x),
    y: sanitizeAxis(l2.y, base.y)
  };
}

function sanitizeAxis(axis: any, baseAxis: any): any {
  const base = baseAxis ?? {};
  const src = axis ?? {};
  return { ...base, ...src, pitch: { ...(base.pitch ?? {}), ...(src.pitch ?? {}), restartEachSection: src.pitch?.restartEachSection === true } };
}

import { applyStoreResult } from "./storeResultHandlers";

export { applyStoreResult };
export type { StoreResult } from "./index";
