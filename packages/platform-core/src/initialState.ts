import { type BehaviorEngine, getBehavior, listBehaviorIds } from "@cellsymphony/behavior-api";
import { loadDefaultMappingConfig } from "@cellsymphony/mapping-core";
import type { PartConfig, PlatformState, RuntimeConfig } from "./platformTypes";
import { SYNTH_PRESETS } from "./synthPresets";
import { PLATFORM_CAPS } from "./platformCaps";
import {
  DEFAULT_VELOCITY_LEVELS,
  DEFAULT_MIDI_ENGINE,
  DEFAULT_NOTE_LENGTH_MS,
  DEFAULT_VELOCITY,
  DEFAULT_MASTER_VOLUME,
  DEFAULT_DISPLAY_BRIGHTNESS,
  DEFAULT_GRID_BRIGHTNESS,
  DEFAULT_BUTTON_BRIGHTNESS,
  DEFAULT_SCREEN_SLEEP_SECONDS,
  DEFAULT_PITCH_STARTING_NOTE,
  DEFAULT_PITCH_LOWEST_NOTE,
  DEFAULT_PITCH_HIGHEST_NOTE,
  DEFAULT_BPM,
  DEFAULT_PAN_POS,
  DEFAULT_VOLUME
} from "./runtimeDefaults";
import { defaultMomentaryFxParams } from "./momentaryFx";
import { deadlineMs, nowMs, STARTUP_SPLASH_MS } from "./timing";

export function createInitialPlatformState<TState>(behavior: BehaviorEngine<TState, unknown>): PlatformState<TState> {
  const now = nowMs();
  const defaultMapping = loadDefaultMappingConfig();
  const instruments = Array.from({ length: PLATFORM_CAPS.instrumentCount }, (_, idx) => ({
    type: "synth" as const,
    autoName: true,
    name: "synth",
    noteBehavior: "oneshot" as const,
    midi: { enabled: false, channel: idx },
    synth: structuredClone(SYNTH_PRESETS[idx % 8]!.synth as RuntimeConfig["instruments"][number]["synth"]),
    sample: {
      baseVelocity: DEFAULT_VELOCITY,
      velocityLevelsEnabled: false,
      velocityLevels: { ...DEFAULT_VELOCITY_LEVELS },
      selectedSlot: 0,
      slots: Array.from({ length: PLATFORM_CAPS.sampleSlotCount }, () => ({ path: null })),
      tuneSemis: 0,
      amp: structuredClone((SYNTH_PRESETS[idx % 8]!.synth as any).amp),
      ampEnv: structuredClone((SYNTH_PRESETS[idx % 8]!.synth as any).ampEnv),
      filter: structuredClone((SYNTH_PRESETS[idx % 8]!.synth as any).filter),
      filterEnv: structuredClone((SYNTH_PRESETS[idx % 8]!.synth as any).filterEnv),
      assignments: []
    },
    midiEngine: { ...DEFAULT_MIDI_ENGINE },
    mixer: { route: "direct" as const, panPos: DEFAULT_PAN_POS, volume: DEFAULT_VOLUME }
  }));

  const runtimeConfig: RuntimeConfig = {
    masterVolume: DEFAULT_MASTER_VOLUME,
    displayBrightness: DEFAULT_DISPLAY_BRIGHTNESS,
    gridBrightness: DEFAULT_GRID_BRIGHTNESS,
    buttonBrightness: DEFAULT_BUTTON_BRIGHTNESS,
    screenSleepSeconds: DEFAULT_SCREEN_SLEEP_SECONDS,
    midi: { enabled: false, outId: null, clockOutEnabled: false, inId: null, clockInEnabled: false, syncMode: "internal", respondToStartStop: true },
    sound: { noteLengthMs: DEFAULT_NOTE_LENGTH_MS, velocityScalePct: 100, velocityCurve: "linear", voiceStealingMode: "balanced" },
    scanMode: "immediate",
    scanAxis: "columns",
    scanUnit: "1/8",
    scanDirection: "forward",
    scanSections: "1",
    algorithmStepUnit: "1/8",
    activeBehavior: behavior.id,
    autoSaveDefault: false,
    behaviorConfig: Object.fromEntries(listBehaviorIds().map(id => {
      const b = getBehavior(id);
      const defaults: Record<string, unknown> = {};
      if (b?.configMenu) {
        const s = b.init({});
        for (const item of b.configMenu(s)) defaults[item.key] = (s as any)[item.key];
      }
      return [id, defaults];
    })),
    eventEnabled: true,
    inputEventsWhilePaused: true,
    numericDisplayMode: "bar+numbers",
    pitch: { startingNote: DEFAULT_PITCH_STARTING_NOTE, lowestNote: DEFAULT_PITCH_LOWEST_NOTE, highestNote: DEFAULT_PITCH_HIGHEST_NOTE, outOfRange: "clamp", scale: "major_pentatonic", root: "D" },
    x: {
      pitch: { enabled: true, steps: 0, restartEachSection: false },
      velocity: { enabled: false, from: 20, to: 100, gridOffset: 0, curve: "linear" },
      filterCutoff: { enabled: false, from: 20, to: 127, gridOffset: 0, curve: "linear" },
      filterResonance: { enabled: false, from: 10, to: 90, gridOffset: 0, curve: "linear" }
    },
    y: {
      pitch: { enabled: true, steps: 1, restartEachSection: false },
      velocity: { enabled: false, from: 20, to: 100, gridOffset: 0, curve: "linear" },
      filterCutoff: { enabled: false, from: 20, to: 127, gridOffset: 0, curve: "linear" },
      filterResonance: { enabled: false, from: 10, to: 90, gridOffset: 0, curve: "linear" }
    },
    activePartIndex: 0,
    ghostCells: false,
    parts: [],
    instruments,
    mixer: {
      buses: Array.from({ length: PLATFORM_CAPS.busCount }, () => ({
        slot1: { type: "none" as const, params: {} },
        slot2: { type: "none" as const, params: {} },
        panPos: DEFAULT_PAN_POS,
        autoName: true,
        name: "(none)"
      }))
    },
    touchFx: {
      selected: { fxType: "stutter", params: defaultMomentaryFxParams("stutter"), targetKey: "master" },
      assignments: []
    }
  };
  const makePart = (idx: number): PartConfig => ({
    l1: {
      stepRate: idx === 0 ? "1/8" : "1/4",
      behaviorId: idx === 0 ? behavior.id : "life",
      behaviorConfig: { ...((runtimeConfig.behaviorConfig as any)[idx === 0 ? behavior.id : "life"] ?? {}) },
      saveGridState: true,
      triggerGates: Array.from({ length: PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight }, () => true)
    },
    l2: {
      scanMode: "immediate",
      scanAxis: "columns",
      scanUnit: "1/8",
      scanDirection: "forward",
      scanSections: "1",
      eventEnabled: idx === 0,
      pitch: structuredClone(runtimeConfig.pitch),
      x: structuredClone(runtimeConfig.x),
      y: structuredClone(runtimeConfig.y),
      mapping: {
        activate: { action: defaultMapping.activate.action, slot: defaultMapping.activate.channel },
        stable: { action: defaultMapping.stable.action, slot: defaultMapping.stable.channel },
        deactivate: { action: defaultMapping.deactivate.action, slot: defaultMapping.deactivate.channel },
        scanned: { action: defaultMapping.scanned.action, slot: defaultMapping.scanned.channel },
        scanned_empty: { action: defaultMapping.scanned_empty.action, slot: defaultMapping.scanned_empty.channel }
      }
    },
    autoName: true,
    name: idx === 0 ? "mock" : "life"
  });
  runtimeConfig.parts = Array.from({ length: PLATFORM_CAPS.partCount }, (_, idx) => makePart(idx));
  const behaviorCfg = runtimeConfig.behaviorConfig as Record<string, Record<string, unknown> | undefined>;
  behaviorCfg.life = { ...(behaviorCfg.life ?? {}), randomCellsPerTick: 12, randomTickInterval: 1 };
  for (let i = 0; i < runtimeConfig.parts.length; i += 1) {
    const p = runtimeConfig.parts[i];
    if (p?.l1?.behaviorId === "life") {
      p.l1.behaviorConfig = { ...(behaviorCfg.life ?? {}) };
    }
  }
  const partStates = runtimeConfig.parts.map((part) => {
    const engine = getBehavior(part.l1.behaviorId) ?? behavior;
    return engine.init({ ...(part.l1.behaviorConfig ?? {}) });
  });
  return {
    transport: { playing: false, bpm: DEFAULT_BPM, tick: 0, ppqnPulse: 0 },
    behaviorState: behavior.init({}),
    activeBehavior: behavior.id,
    mappingConfig: loadDefaultMappingConfig(),
    runtimeConfig,
    menu: { stack: [], cursor: 0, editing: false },
    system: {
      shiftHeld: false,
      fnHeld: false,
      physicalShiftHeld: false,
      physicalFnHeld: false,
      combinedModifierHeld: false,
      presetNames: [],
      selectedPreset: null,
      currentPresetName: null,
      draftName: "",
      nameCursor: 0,
      pendingRename: null,
      confirm: null,
      toast: null,
      eventBlipUntilMs: 0,
      stopLatched: true,
      transportFlash: "none",
      transportFlashUntilMs: 0,
      autoSaveFlash: "none",
      autoSaveFlashUntilMs: 0,
      textEdit: null,
      midiOutputs: [],
      midiInputs: [],
      midiStatus: null,
      externalPpqnPulse: 0,
      pendingResync: false,
      pausedByUser: false,
      oledMode: "splash",
      oledSplashText: "Starting up",
      oledSplashUntilMs: deadlineMs(now, STARTUP_SPLASH_MS),
      lastInteractionMs: now,
      auxBindings: structuredClone((runtimeConfig as any).auxBindings ?? {}),
      shiftHeldSinceMs: null,
      auxOverlayScroll: 0,
      auxAutoMapEnabled: true,
      heldNotes: [],
      sampleAssign: null,
      fxAssignMode: null,
      activeFx: [],
      pendingCloneSource: null,
            sampleBrowser: null,
      touchMode: "none",
      triggerGateTarget: "active"
    },
    scanIndex: 0,
    scanPulseAccumulator: 0,
    algorithmPulseAccumulator: 0,
    ppqnPulseRemainder: 0,
    partStates,
    partScanIndex: Array.from({ length: PLATFORM_CAPS.partCount }, () => 0),
    partScanPulseAccumulator: Array.from({ length: PLATFORM_CAPS.partCount }, () => 0),
    partAlgorithmPulseAccumulator: Array.from({ length: PLATFORM_CAPS.partCount }, () => 0),
    partPaused: Array.from({ length: PLATFORM_CAPS.partCount }, () => false)
  };
}
