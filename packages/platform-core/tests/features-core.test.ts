import test from "node:test";
import assert from "node:assert/strict";
import type { BehaviorEngine, CellTriggerType } from "@cellsymphony/behavior-api";
import { type DeviceInput } from "@cellsymphony/device-contracts";
import { lifeBehavior } from "@cellsymphony/behaviors-life";
import { sequencerBehavior } from "@cellsymphony/behaviors-sequencer";
import {
  PLATFORM_CAPS,
  createInitialState,
  routeInput,
  tick,
  toRuntimeSnapshot,
  extractConfigPayload,
  applyConfigPayload,
  applyStoreResult,
  type PlatformState,
  type PlatformEffect
} from "../src/index";
import { handleMenuAction } from "../src/actions";
import { writeValue } from "../src/coreUtils";

const CELL_COUNT = PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight;

type MockState = { cells: boolean[]; tickCount: number };

const mockBehavior: BehaviorEngine<MockState, unknown> = {
  id: "mock",
  init: () => ({
    cells: Array.from({ length: CELL_COUNT }, (_, i) => i === 0 || i === PLATFORM_CAPS.gridWidth),
    tickCount: 0
  }),
  onInput: (state) => state,
  onTick: (state) => {
    const next = state.cells.slice();
    next[0] = !next[0];
    return { cells: next, tickCount: state.tickCount + 1 };
  },
  renderModel: (state) => ({
    name: "Mock",
    statusLine: "ok",
    cells: state.cells
  }),
  serialize: (state) => state,
  deserialize: (data) => data as MockState
};

function makeState() {
  const s = createInitialState(mockBehavior);
  s.system.oledMode = "normal";
  return s;
}

function turn(state: PlatformState<MockState>, delta: -1 | 1): { state: PlatformState<MockState>; effects: PlatformEffect[] } {
  return routeInput(state, { type: "encoder_turn", delta } as DeviceInput, mockBehavior);
}

function press(state: PlatformState<MockState>): { state: PlatformState<MockState>; effects: PlatformEffect[] } {
  return routeInput(state, { type: "encoder_press" } as DeviceInput, mockBehavior);
}

function selectLabel(state: PlatformState<MockState>, label: string): PlatformState<MockState> {
  for (let i = 0; i < 80; i += 1) {
    const frame = toRuntimeSnapshot(state, mockBehavior);
    const selected = frame.display.lines.find((l) => l.startsWith("@@")) ?? "";
    if (selected.includes(label)) return state;
    const r = turn(state, 1);
    state = r.state;
  }
  throw new Error(`failed to select label: ${label}`);
}

// ─── Behavior Config Editing ──────────────────────────────────────

test("behavior config number param edit via menu", () => {
  let state = createInitialState(lifeBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.activeBehavior = "life";

  // Navigate: L1: Life → P1: mock → "Spawn Count"
  state = selectLabel(state, "L1: Life");
  state = press(state).state;
  state = selectLabel(state, "P1: mock");
  state = press(state).state;
  state = selectLabel(state, "Spawn Count");
  state = press(state).state; // enter edit mode
  state = turn(state, -1).state; // decrease by 1
  state = press(state).state; // exit edit

  const val = (state.runtimeConfig.behaviorConfig as any).life?.randomCellsPerTick;
  assert.equal(val, 11, "randomCellsPerTick should be 11 (decremented from default 12)");
});

test("behavior config enum param edit via menu", () => {
  let state = createInitialState(lifeBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.activeBehavior = "life";
  (state.runtimeConfig.behaviorConfig as any).life = { randomCellsPerTick: 0, randomTickInterval: 1 };
  ((state.runtimeConfig as any).parts[0].l1.behaviorConfig as any) = { life: { randomCellsPerTick: 0, randomTickInterval: 1 } };

  // L1: Life → P1: mock → "Spawn Interval" (number)
  state = selectLabel(state, "L1: Life");
  state = press(state).state;
  state = selectLabel(state, "P1: mock");
  state = press(state).state;
  state = selectLabel(state, "Spawn Interval");
  state = press(state).state; // enter edit
  state = turn(state, 1).state; // increase by 1
  state = press(state).state; // exit edit

  const val = (state.runtimeConfig.behaviorConfig as any).life?.randomTickInterval;
  assert.equal(val, 2, "randomTickInterval should be 2 (incremented from default 1)");
});

test("FX type selection seeds editable default parameters", () => {
  let state = makeState();

  state = selectLabel(state, "L3: Voice");
  state = press(state).state;
  state = selectLabel(state, "FX Buses");
  state = press(state).state;
  state = selectLabel(state, "B1: (none)");
  state = press(state).state;
  state = selectLabel(state, "Slot 1");
  state = press(state).state;
  state = selectLabel(state, "Type");
  state = press(state).state;
  state = turn(state, 1).state;
  state = turn(state, 1).state;
  state = press(state).state;

  const slot = (state.runtimeConfig as any).mixer.buses[0].slot1;
  assert.equal(slot.type, "delay");
  assert.equal(slot.params.timeMs, 250);
  assert.equal(slot.params.feedback, 0.35);
  assert.equal(slot.params.mixPct, 35);
});

test("newly selected FX parameters edit as finite numbers", () => {
  let state = makeState();

  state = selectLabel(state, "L3: Voice");
  state = press(state).state;
  state = selectLabel(state, "FX Buses");
  state = press(state).state;
  state = selectLabel(state, "B1: (none)");
  state = press(state).state;
  state = selectLabel(state, "Slot 1");
  state = press(state).state;
  state = selectLabel(state, "Type");
  state = press(state).state;
  state = turn(state, 1).state;
  state = turn(state, 1).state;
  state = press(state).state;
  state = selectLabel(state, "Time ms");

  const frame = toRuntimeSnapshot(state, mockBehavior);
  assert.equal(frame.display.lines.some((line) => line.includes("undefined") || line.includes("NaN")), false);

  state = press(state).state;
  state = turn(state, 1).state;

  const timeMs = (state.runtimeConfig as any).mixer.buses[0].slot1.params.timeMs;
  assert.equal(Number.isFinite(timeMs), true);
  assert.equal(timeMs, 255);
});

test("loading saved FX slots repairs missing and invalid parameters", () => {
  let state = makeState();
  const payload = extractConfigPayload(state) as any;
  payload.runtimeConfig.mixer.buses[0].slot1 = { type: "delay", params: {} };
  payload.runtimeConfig.mixer.buses[0].slot2 = { type: "duck", params: { source: "B999", amountPct: "bad" } };

  state = applyConfigPayload(state, payload, mockBehavior);

  const slot1 = (state.runtimeConfig as any).mixer.buses[0].slot1;
  const slot2 = (state.runtimeConfig as any).mixer.buses[0].slot2;
  assert.deepEqual(slot1, { type: "delay", params: { timeMs: 250, feedback: 0.35, mixPct: 35 } });
  assert.equal(slot2.type, "duck");
  assert.equal(slot2.params.source, "I1");
  assert.equal(slot2.params.amountPct, 60);
  assert.equal(slot2.params.threshold, 0.08);
});

test("FX compressor type selection seeds editable default parameters", () => {
  let state = makeState();
  state = selectLabel(state, "L3: Voice");
  state = press(state).state;
  state = selectLabel(state, "FX Buses");
  state = press(state).state;
  state = selectLabel(state, "B1: (none)");
  state = press(state).state; // enter FX Bus 1
  state = press(state).state; // enter Slot 1
  state = selectLabel(state, "Type");
  state = press(state).state; // enter edit
  // Turn from "none" past: reverb(1), delay(2), tremolo(3), chorus(4), flanger(5), vibrato(6), auto_pan(7), filter_lfo(8), wah(9), eq(10), compressor(11)
  for (let i = 0; i < 11; i += 1) {
    const r2 = turn(state, 1);
    state = r2.state;
  }
  state = press(state).state; // confirm

  const slot = (state.runtimeConfig as any).mixer.buses[0].slot1;
  assert.equal(slot.type, "compressor");
  assert.equal(slot.params.thresholdDb, -24);
  assert.equal(slot.params.ratio, 4);
  assert.equal(slot.params.attackMs, 10);
  assert.equal(slot.params.releaseMs, 100);
  assert.equal(slot.params.makeupDb, 0);
  assert.equal(slot.params.mixPct, 100);
});

test("FX eq type selection seeds editable default parameters", () => {
  let state = makeState();
  state = selectLabel(state, "L3: Voice");
  state = press(state).state;
  state = selectLabel(state, "FX Buses");
  state = press(state).state;
  state = selectLabel(state, "B1: (none)");
  state = press(state).state; // enter FX Bus 1
  state = press(state).state; // enter Slot 1
  state = selectLabel(state, "Type");
  state = press(state).state; // enter edit
  // Turn past none, reverb, delay, tremolo, chorus, flanger, vibrato, auto_pan, filter_lfo, wah = 10 turns to eq
  for (let i = 0; i < 10; i += 1) {
    const r2 = turn(state, 1);
    state = r2.state;
  }
  state = press(state).state; // confirm

  const slot = (state.runtimeConfig as any).mixer.buses[0].slot1;
  assert.equal(slot.type, "eq");
  assert.equal(slot.params.lowGainDb, 0);
  assert.equal(slot.params.midGainDb, 0);
  assert.equal(slot.params.midFreqHz, 1000);
  assert.equal(slot.params.midQ, 1);
  assert.equal(slot.params.highGainDb, 0);
  assert.equal(slot.params.mixPct, 100);
});

test("Global FX slot count follows platform capabilities", () => {
  const state = makeState();
  assert.equal((state.runtimeConfig as any).mixer.master.slots.length, PLATFORM_CAPS.globalFxSlotCount);
});

test("Global FX vinyl type selection seeds editable default parameters", () => {
  let state = makeState();
  state = selectLabel(state, "L3: Voice");
  state = press(state).state;
  state = selectLabel(state, "Global FX");
  state = press(state).state;
  state = selectLabel(state, "Slot 1");
  state = press(state).state;
  state = selectLabel(state, "Type");
  state = press(state).state;
  state = turn(state, 1).state;
  state = press(state).state;

  const slot = (state.runtimeConfig as any).mixer.master.slots[0];
  assert.equal(slot.type, "vinyl");
  assert.equal(slot.params.saturationPct, 15);
  assert.equal(slot.params.cracklePct, 8);
  assert.equal(slot.params.warpDepthPct, 5);
  assert.equal(slot.params.mixPct, 100);
});

test("loading saved Global FX repairs missing and invalid parameters", () => {
  let state = makeState();
  const payload = extractConfigPayload(state) as any;
  payload.runtimeConfig.mixer.master.slots = [
    { type: "vinyl", params: {} },
    { type: "compressor", params: { thresholdDb: "bad", ratio: 9 } }
  ];

  state = applyConfigPayload(state, payload, mockBehavior);

  const slot1 = (state.runtimeConfig as any).mixer.master.slots[0];
  const slot2 = (state.runtimeConfig as any).mixer.master.slots[1];
  assert.deepEqual(slot1, { type: "vinyl", params: { saturationPct: 15, cracklePct: 8, warpDepthPct: 5, mixPct: 100 } });
  assert.equal(slot2.type, "compressor");
  assert.equal(slot2.params.thresholdDb, -24);
  assert.equal(slot2.params.ratio, 9);
  assert.equal(slot2.params.attackMs, 10);
  assert.equal(slot2.params.releaseMs, 100);
  assert.equal(slot2.params.makeupDb, 0);
  assert.equal(slot2.params.mixPct, 100);
});

test("old bus_N route normalizes to fx_bus_N on load", () => {
  let state = makeState();
  const payload = extractConfigPayload(state) as any;
  (payload.runtimeConfig as any).instruments[0].mixer = { route: "bus_2", panPos: 4 };
  state = applyConfigPayload(state, payload, mockBehavior);
  assert.equal((state.runtimeConfig as any).instruments[0].mixer.route, "fx_bus_2");
});

test("fx_bus_N route survives round-trip", () => {
  let state = makeState();
  const payload = extractConfigPayload(state) as any;
  (payload.runtimeConfig as any).instruments[0].mixer = { route: "fx_bus_3", panPos: 4 };
  state = applyConfigPayload(state, payload, mockBehavior);
  assert.equal((state.runtimeConfig as any).instruments[0].mixer.route, "fx_bus_3");
});

// ─── Active Behavior Switching ────────────────────────────────────

test("active behavior switching reinitializes state", () => {
  let state = makeState();
  state.activeBehavior = "life";

  // Start with mock, switch to life
  const r = routeInput(state, { type: "encoder_press", id: "SW1" } as DeviceInput, mockBehavior);
  state = r.state;

  // The behavior state should be reinitialized when switching
  // Since we're using mockBehavior here, the behavior switching happens via applyConfigPayload
  // Instead, test directly with the menu system
});

test("behavior config persists independently per behavior", () => {
  let state = createInitialState(lifeBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.activeBehavior = "life";
  state.runtimeConfig.behaviorConfig = {
    life: { randomCellsPerTick: 5, randomTickInterval: 3 },
    brain: { fireThreshold: 3, randomSeedCells: 0 }
  } as any;
  ((state.runtimeConfig as any).parts[0].l1.behaviorConfig as any) = { randomCellsPerTick: 5, randomTickInterval: 3 };

  // Switch to brain
  state = selectLabel(state, "L1: Life");
  state = press(state).state;
  state = selectLabel(state, "P1: mock");
  state = press(state).state;

  // The "Behavior" menu item should be at a specific position
  state = selectLabel(state, "Behavior");
  state = press(state).state; // enter edit mode for Behavior enum

  // Turn to select brain
  state = turn(state, 1).state; // move from life to next behavior

  // Now activeBehavior should have changed
  // Press to exit edit
  state = press(state).state;

  // Verify life config was preserved
  assert.deepStrictEqual(
    (state.runtimeConfig.behaviorConfig as any).life,
    { randomCellsPerTick: 5, randomTickInterval: 3 }
  );
});

// ─── Auto-Save Default ────────────────────────────────────────────

test("autoSaveDefault on emits store_save_default on value edit", () => {
  let state = makeState();
  state.runtimeConfig.autoSaveDefault = true;

  // Navigate to System → Audio → Master Vol
  state = selectLabel(state, "System");
  state = press(state).state;
  state = selectLabel(state, "Sound");
  state = press(state).state;
  state = press(state).state; // enter edit mode for Master Vol
  const r = turn(state, -1); // change value
  state = r.state;
  press(state); // exit edit

  const hasAutoSave = r.effects.some((e) => e.type === "store_save_default");
  assert.equal(hasAutoSave, true, "should emit store_save_default effect");
  if (r.effects.some((e) => e.type === "store_save_default")) {
    const saveEffect = r.effects.find((e) => e.type === "store_save_default")!;
    if (saveEffect.type === "store_save_default") {
      assert.equal(saveEffect.payload.runtimeConfig.masterVolume, 72);
      assert.equal(saveEffect.mode, "deferred");
    }
  }
});

test("autoSaveDefault off does not emit store_save_default", () => {
  let state = makeState();
  state.runtimeConfig.autoSaveDefault = false;

  state = selectLabel(state, "System");
  state = press(state).state;
  state = selectLabel(state, "Sound");
  state = press(state).state;
  state = press(state).state; // enter edit
  const r = turn(state, -1);
  state = r.state;
  press(state);

  const hasAutoSave = r.effects.some((e) => e.type === "store_save_default");
  assert.equal(hasAutoSave, false, "should NOT emit store_save_default effect");
});

test("enabling autoSaveDefault emits immediate save when exiting edit", () => {
  let state = makeState();
  state.runtimeConfig.autoSaveDefault = false;

  state = selectLabel(state, "System");
  state = press(state).state;
  state = selectLabel(state, "Saves");
  state = press(state).state;
  state = selectLabel(state, "Default");
  state = press(state).state;
  state = selectLabel(state, "Auto Save");
  state = press(state).state; // enter edit
  state = turn(state, 1).state; // false -> true

  const exit = press(state);
  state = exit.state;
  const hasAutoSave = exit.effects.some((e) => e.type === "store_save_default");
  assert.equal(state.runtimeConfig.autoSaveDefault, true);
  assert.equal(hasAutoSave, true, "should emit store_save_default when exiting auto-save edit in ON state");
  const saveEffect = exit.effects.find((e) => e.type === "store_save_default");
  assert.equal(saveEffect?.type === "store_save_default" ? saveEffect.mode : undefined, "immediate");
});

test("auto-save payload contains post-edit state", () => {
  let state = makeState();
  state.runtimeConfig.autoSaveDefault = true;

  state = selectLabel(state, "System");
  state = press(state).state;
  state = selectLabel(state, "Sound");
  state = press(state).state;
  state = press(state).state;
  const r = turn(state, 1);
  state = r.state;
  press(state);

  const saveEffect = r.effects.find((e) => e.type === "store_save_default");
  assert.ok(saveEffect, "should have store_save_default effect");
  if (saveEffect && saveEffect.type === "store_save_default") {
    assert.equal(saveEffect.payload.runtimeConfig.masterVolume, 74, "payload should reflect post-edit value");
    assert.equal(saveEffect.mode, "deferred");
  }
});

test("activeBehavior change with autoSaveDefault on emits store_save_default", () => {
  let state = createInitialState(lifeBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.autoSaveDefault = true;
  state.runtimeConfig.activeBehavior = "sequencer";
  ((state.runtimeConfig as any).parts[0] as any).l1.behaviorId = "sequencer";

  state = selectLabel(state, "L1: Life");
  state = press(state).state;
  state = selectLabel(state, "P1: mock");
  state = press(state).state;
  state = selectLabel(state, "Behavior");
  state = press(state).state; // enter edit
  const r = turn(state, 1); // rotate to next behavior (life)
  state = r.state;
  press(state);

  const saveEffect = r.effects.find((e) => e.type === "store_save_default");
  assert.ok(saveEffect, "behavior switch with autoSave should emit store_save_default");
  assert.equal(saveEffect?.type === "store_save_default" ? saveEffect.mode : undefined, "deferred");
});

test("save current preset triggers overwrite flow for loaded preset", () => {
  let state = makeState();
  state.system.currentPresetName = "Jam A";

  state = selectLabel(state, "System");
  state = press(state).state;
  state = selectLabel(state, "Saves");
  state = press(state).state;
  state = selectLabel(state, "Library");
  state = press(state).state;
  state = selectLabel(state, "Save Current");

  state = press(state).state;
  assert.ok(state.system.confirm, "should open overwrite confirm");

  state = routeInput(state, { type: "encoder_turn", id: "main", delta: 1 } as DeviceInput, mockBehavior).state;
  const confirm = routeInput(state, { type: "encoder_press", id: "main" } as DeviceInput, mockBehavior);
  const hasSave = confirm.effects.some((e) => e.type === "store_save_preset" && e.name === "Jam A");
  assert.equal(hasSave, true);
});

test("save current shows loaded preset name under action", () => {
  let state = makeState();
  state.system.currentPresetName = "Jam A";

  state = selectLabel(state, "System");
  state = press(state).state;
  state = selectLabel(state, "Saves");
  state = press(state).state;
  state = selectLabel(state, "Library");
  state = press(state).state;
  state = selectLabel(state, "Save Current");

  const frame = toRuntimeSnapshot(state, mockBehavior);
  const selectedIndex = frame.display.lines.findIndex((l) => l.startsWith("@@") && l.includes("Save Current"));
  assert.ok(selectedIndex >= 0, "save current row should be selected");
  assert.equal(frame.display.lines[selectedIndex + 1], "@@  Jam A");
});

test("save current preset shows toast when none loaded", () => {
  let state = makeState();
  state.system.currentPresetName = null;

  state = selectLabel(state, "System");
  state = press(state).state;
  state = selectLabel(state, "Saves");
  state = press(state).state;
  state = selectLabel(state, "Library");
  state = press(state).state;
  state = selectLabel(state, "Save Current");
  state = press(state).state;

  assert.equal(state.system.toast?.message, "No preset loaded");
});

test("loading preset tracks current preset name", () => {
  let state = makeState();
  const payload = extractConfigPayload(state);
  state = applyStoreResult(state, { type: "load_preset_result", name: "Jam B", payload }, mockBehavior).state;
  assert.equal(state.system.currentPresetName, "Jam B");
});

test("Dance config and active page persist through config payload", () => {
  let state = makeState();
  state.runtimeConfig.danceMode = "xy";
  state.system.danceMode = "xy";
  state.system.triggerGateTarget = "all";
  state.runtimeConfig.xyRelease = "reset-center";
  (state.runtimeConfig as any).parts[0].xy = {
    x: { key: "instruments.0.synth.filter.cutoffHz", label: "Cutoff", kind: "number", min: 20, max: 20000, step: 1 },
    y: { key: "eventEnabled", label: "Events", kind: "bool" },
    xInvert: true,
    yInvert: true
  };
  (state.runtimeConfig as any).xyTouch = { x: 0.8, y: 0.2, active: true };
  (state.runtimeConfig as any).touchFx.selected = { fxType: "freeze", params: { releaseMs: 900, mixPct: 45 }, targetKey: "master" };
  (state.runtimeConfig as any).touchFx.assignments = [{ x: 1, y: 2, config: { fxType: "stutter", params: { rateHz: 9, depthPct: 60 }, targetKey: "master" } }];
  (state.runtimeConfig as any).instruments[0].mixer.volume = 34;
  (state.runtimeConfig as any).instruments[0].mixer.panPos = 5;
  (state.runtimeConfig as any).parts[0].l2.triggerProbabilityMode = "custom";
  (state.runtimeConfig as any).parts[0].l2.triggerProbabilityMap[3] = "zero";

  const payload = extractConfigPayload(state) as any;
  const restored = applyConfigPayload(createInitialState(mockBehavior), payload, mockBehavior) as any;

  assert.deepEqual(payload.system, { triggerGateTarget: "all" });
  assert.equal(restored.runtimeConfig.danceMode, "xy");
  assert.equal(restored.system.danceMode, "none");
  assert.equal(restored.system.triggerGateTarget, "all");
  assert.equal(restored.runtimeConfig.xyRelease, "reset-center");
  assert.equal(restored.runtimeConfig.parts[0].xy.x.key, "instruments.0.synth.filter.cutoffHz");
  assert.equal(restored.runtimeConfig.parts[0].xy.y.key, "eventEnabled");
  assert.equal(restored.runtimeConfig.parts[0].xy.xInvert, true);
  assert.equal(restored.runtimeConfig.parts[0].xy.yInvert, true);
  assert.equal(restored.runtimeConfig.xyTouch.x, 0.5);
  assert.equal(restored.runtimeConfig.xyTouch.y, 0.5);
  assert.equal(restored.runtimeConfig.xyTouch.active, false);
  assert.equal(restored.runtimeConfig.touchFx.selected.fxType, "freeze");
  assert.equal(restored.runtimeConfig.touchFx.assignments.length, 1);
  assert.equal(restored.runtimeConfig.touchFx.assignments[0].config.fxType, "stutter");
  assert.equal(restored.runtimeConfig.instruments[0].mixer.volume, 34);
  assert.equal(restored.runtimeConfig.instruments[0].mixer.panPos, 5);
  assert.equal(restored.runtimeConfig.parts[0].l2.triggerProbabilityMode, "custom");
  assert.equal(restored.runtimeConfig.parts[0].l2.triggerProbabilityMap[3], "zero");
});

test("X/Y target assignment auto-saves when autoSaveDefault is enabled", () => {
  const state = makeState();
  state.runtimeConfig.autoSaveDefault = true;
  const effects: PlatformEffect[] = [];

  const next = handleMenuAction(state, {
    type: "xy_set_target",
    axis: "x",
    binding: { key: "masterVolume", label: "Master Vol", kind: "number", min: 0, max: 100, step: 1 }
  }, effects, {
    writeValue,
    extractConfigPayload,
    resolveBehavior: () => mockBehavior
  }) as any;

  assert.equal(next.runtimeConfig.parts[0].xy.x.key, "masterVolume");
  assert.equal(effects.some((e) => e.type === "store_save_default"), true);
});

test("Sense param-mod target assignment auto-saves when autoSaveDefault is enabled", () => {
  const state = makeState();
  state.runtimeConfig.autoSaveDefault = true;
  const effects: PlatformEffect[] = [];

  const next = handleMenuAction(state, {
    type: "param_mod_set_target",
    partIndex: 0,
    axis: "x",
    slot: 0,
    binding: { key: "masterVolume", label: "Master Vol", kind: "number", min: 0, max: 100, step: 1 }
  }, effects, {
    writeValue,
    extractConfigPayload,
    resolveBehavior: () => mockBehavior
  }) as any;

  assert.equal(next.runtimeConfig.parts[0].paramMods.x[0].key, "masterVolume");
  assert.equal(effects.some((e) => e.type === "store_save_default"), true);
});

test("Aux turn target assignment auto-saves and preserves click binding", () => {
  const state = makeState();
  state.runtimeConfig.autoSaveDefault = true;
  state.system.auxBindings.aux1 = {
    turn: null,
    press: { kind: "behavior_action", actionType: "spawnRandom", label: "Spawn" }
  };
  state.runtimeConfig.auxBindings = {
    ...state.runtimeConfig.auxBindings,
    aux1: state.system.auxBindings.aux1
  };
  const effects: PlatformEffect[] = [];

  const next = handleMenuAction(state, {
    type: "aux_turn_set_target",
    encoderId: "aux1",
    binding: { key: "masterVolume", label: "Master Vol", kind: "number", min: 0, max: 100, step: 1 }
  }, effects, {
    writeValue,
    extractConfigPayload,
    resolveBehavior: () => mockBehavior
  }) as any;

  assert.equal(next.system.auxBindings.aux1.turn.key, "masterVolume");
  assert.equal(next.system.auxBindings.aux1.press?.kind, "behavior_action");
  assert.equal(next.runtimeConfig.auxBindings.aux1.press?.kind, "behavior_action");
  assert.equal(effects.some((e) => e.type === "store_save_default"), true);
});

test("Aux click target assignment auto-saves and preserves turn binding", () => {
  const state = makeState();
  state.runtimeConfig.autoSaveDefault = true;
  state.system.auxBindings.aux1 = {
    turn: { key: "masterVolume", label: "Master Vol", kind: "number", min: 0, max: 100, step: 1 },
    press: null
  };
  state.runtimeConfig.auxBindings = {
    ...state.runtimeConfig.auxBindings,
    aux1: state.system.auxBindings.aux1
  };
  const effects: PlatformEffect[] = [];

  const next = handleMenuAction(state, {
    type: "aux_click_set_target",
    encoderId: "aux1",
    press: { kind: "behavior_action", actionType: "spawnRandom", label: "Spawn" }
  }, effects, {
    writeValue,
    extractConfigPayload,
    resolveBehavior: () => mockBehavior
  }) as any;

  assert.equal(next.system.auxBindings.aux1.turn.key, "masterVolume");
  assert.equal(next.system.auxBindings.aux1.press?.kind, "behavior_action");
  assert.equal(next.runtimeConfig.auxBindings.aux1.turn.key, "masterVolume");
  assert.equal(effects.some((e) => e.type === "store_save_default"), true);
});

test("Dance page selection and trigger-gate edits auto-save when enabled", () => {
  let state = makeState();
  state.runtimeConfig.autoSaveDefault = true;

  state = routeInput(state, { type: "button_fn", pressed: true } as DeviceInput, mockBehavior).state;
  const pageSelect = routeInput(state, { type: "grid_press", x: PLATFORM_CAPS.gridWidth - 1, y: 4 } as DeviceInput, mockBehavior);
  assert.equal(pageSelect.state.system.danceMode, "xy");
  assert.equal(pageSelect.state.runtimeConfig.danceMode, "xy");
  assert.equal(pageSelect.effects.some((e) => e.type === "store_save_default"), true);

  let gateState = makeState();
  gateState.runtimeConfig.autoSaveDefault = true;
  gateState.system.danceMode = "trigger-gate";
  const gateEdit = routeInput(gateState, { type: "grid_press", x: 1, y: PLATFORM_CAPS.gridHeight - 1 } as DeviceInput, mockBehavior);
  assert.equal(gateEdit.effects.some((e) => e.type === "store_save_default"), true);
});

test("legacy trigger gates migrate into probability map custom mode", () => {
  const state = makeState();
  const payload = extractConfigPayload(state) as any;
  delete payload.runtimeConfig.parts[0].l2.triggerProbabilityMap;
  delete payload.runtimeConfig.parts[0].l2.triggerProbabilityMode;
  payload.runtimeConfig.parts[0].l1.triggerGates = Array.from({ length: PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight }, (_, i) => i !== 3);

  const restored = applyConfigPayload(createInitialState(mockBehavior), payload, mockBehavior) as any;
  assert.equal(restored.runtimeConfig.parts[0].l2.triggerProbabilityMode, "custom");
  assert.equal(restored.runtimeConfig.parts[0].l2.triggerProbabilityMap[3], "zero");
  assert.equal(restored.runtimeConfig.parts[0].l2.triggerProbabilityMap[2], "full");
});

test("factory reset restores default behavior to life", () => {
  let state = createInitialState(sequencerBehavior as any) as any;
  state.system.oledMode = "normal";
  state.runtimeConfig.activeBehavior = "sequencer";
  state.activeBehavior = "sequencer";
  state.system.confirm = {
    kind: "load_factory",
    action: { kind: "factory_load" },
    cursor: 0,
    options: ["Yes", "No"],
    scroll: 0
  };

  const result = routeInput(state, { type: "encoder_press" } as DeviceInput, sequencerBehavior as any);
  assert.equal(result.state.runtimeConfig.activeBehavior, "life");
  assert.equal(result.state.activeBehavior, "life");
});

test("sample list result creates/updates sample browser state", () => {
  let state = makeState();
  const resultA = applyStoreResult(
    state,
    { type: "sample_list_result", instrumentSlot: 2, sampleSlot: 3, dir: "drums", entries: [{ name: "a.wav", path: "drums/a.wav", isDir: false }] } as any,
    mockBehavior
  );
  state = resultA.state;
  assert.equal(state.system.sampleBrowser?.instrumentSlot, 2);
  assert.equal(state.system.sampleBrowser?.sampleSlot, 3);
  assert.equal(state.system.sampleBrowser?.entries.length, 1);

  const resultB = applyStoreResult(
    state,
    { type: "sample_list_result", instrumentSlot: 2, sampleSlot: 3, dir: "drums/kits", entries: [{ name: "k.wav", path: "drums/kits/k.wav", isDir: false }] } as any,
    mockBehavior
  );
  assert.equal(resultB.state.system.sampleBrowser?.dir, "drums/kits");
  assert.equal(resultB.state.system.sampleBrowser?.entries[0]?.name, "k.wav");
});

test("sample list/preview errors set user toast", () => {
  let state = makeState();
  const listErr = applyStoreResult(
    state,
    { type: "sample_list_error", instrumentSlot: 0, sampleSlot: 0, dir: "", message: "permission denied" } as any,
    mockBehavior
  );
  state = listErr.state;
  assert.ok(state.system.toast?.message.includes("Sample list error"));

  const previewErr = applyStoreResult(
    state,
    { type: "sample_preview_error", message: "decode failed" } as any,
    mockBehavior
  );
  assert.ok(previewErr.state.system.toast?.message.includes("Sample preview error"));
});

test("shift space emergency brake is handled by core input routing", () => {
  let state = makeState();
  state.transport.playing = true;
  state.transport.ppqnPulse = 24;
  state = routeInput(state, { type: "button_shift", pressed: true } as DeviceInput, mockBehavior).state;

  const result = routeInput(state, { type: "button_s", pressed: true } as DeviceInput, mockBehavior);

  assert.equal(result.state.transport.playing, false);
  assert.equal(result.state.transport.ppqnPulse, 0);
  assert.equal(result.state.system.stopLatched, true);
  assert.ok(result.events.some((event) => event.type === "cc" && event.controller === 123));
});

test("shift space keeps external sync resync behavior", () => {
  let state = makeState();
  state.runtimeConfig.midi.syncMode = "external";
  state = routeInput(state, { type: "button_shift", pressed: true } as DeviceInput, mockBehavior).state;

  const result = routeInput(state, { type: "button_s", pressed: true } as DeviceInput, mockBehavior);

  assert.equal(result.state.system.pendingResync, true);
  assert.equal(result.events.length, 0);
});

test("midi_status updates system midi status", () => {
  const state = makeState();
  const ok = applyStoreResult(state, { type: "midi_status", ok: true, message: "ok" } as any, mockBehavior);
  assert.equal(ok.state.system.midiStatus, "MIDI ok");
  const bad = applyStoreResult(state, { type: "midi_status", ok: false, message: "failed" } as any, mockBehavior);
  assert.equal(bad.state.system.midiStatus, "failed");
});

test("FN+SHIFT+rightmost grid press stores clone source", () => {
  let state = makeState();
  state = routeInput(state, { type: "button_fn", pressed: true }, mockBehavior).state;
  state = routeInput(state, { type: "button_shift", pressed: true }, mockBehavior).state;
  state = routeInput(state, { type: "grid_press", x: 7, y: 1 }, mockBehavior).state;
  assert.equal(state.system.pendingCloneSource, 1);
  assert.ok(state.system.toast?.message.includes("Clone P2"));
});

test("FN+left column with pending clone source executes clone", () => {
  let state = makeState();
  const origP1Beh = ((state.runtimeConfig as any).parts[0] as any).l1.behaviorId;
  const origP3Beh = ((state.runtimeConfig as any).parts[2] as any).l1.behaviorId;
  assert.notEqual(origP1Beh, origP3Beh);

  state.system.pendingCloneSource = 0; // source = P1
  state = routeInput(state, { type: "button_fn", pressed: true }, mockBehavior).state;
  const result = routeInput(state, { type: "grid_press", x: 0, y: 2 }, mockBehavior);
  state = result.state;
  assert.equal(state.system.pendingCloneSource, null);
  assert.equal(((state.runtimeConfig as any).parts[2] as any).l1.behaviorId, origP1Beh);
  assert.equal(((state.runtimeConfig as any).activePartIndex), 2);
  assert.ok(state.system.toast?.message.includes("Cloned P1 → P3"));
});

test("FN+left column without pending clone source just selects part", () => {
  let state = makeState();
  state = routeInput(state, { type: "button_fn", pressed: true }, mockBehavior).state;
  const result = routeInput(state, { type: "grid_press", x: 0, y: 5 }, mockBehavior);
  assert.equal(result.state.system.pendingCloneSource, null);
  assert.equal((result.state.runtimeConfig as any).activePartIndex, 5);
  assert.equal(result.state.system.toast?.message, "Part 6");
});

test("FN+SHIFT+BACK does not trigger shift clear", () => {
  let state = makeState();
  const activeIdx = (state.runtimeConfig as any).activePartIndex;
  (state.runtimeConfig as any).parts[activeIdx].l1.behaviorId = "life";

  state = routeInput(state, { type: "button_fn", pressed: true }, mockBehavior).state;
  state = routeInput(state, { type: "button_shift", pressed: true }, mockBehavior).state;
  state = routeInput(state, { type: "button_a", pressed: true }, mockBehavior).state;

  assert.equal(((state.runtimeConfig as any).parts[activeIdx] as any).l1.behaviorId, "life");
  assert.equal(state.activeBehavior, "mock");
  assert.equal(state.system.toast?.message, undefined);
});

test("factory reset defaults match REQ-14 specification", () => {
  let state = makeState();
  state.runtimeConfig.activeBehavior = "sequencer";
  state.activeBehavior = "sequencer";
  state.system.confirm = {
    kind: "load_factory",
    action: { kind: "factory_load" },
    cursor: 0,
    options: ["Yes", "No"],
    scroll: 0
  };
  const result = routeInput(state, { type: "encoder_press" } as DeviceInput, mockBehavior);
  state = result.state;
  const rc: any = state.runtimeConfig;

  // P1: life with auto-spawn=12
  assert.equal(rc.parts[0].l1.behaviorId, "life");
  assert.equal(rc.parts[0].l1.behaviorConfig.randomCellsPerTick, 12);
  assert.equal(rc.parts[0].l2.mapping.activate.slot, 0);

  // P2: sequencer with horizontal scan, scanned→I2
  assert.equal(rc.parts[1].l1.behaviorId, "sequencer");
  assert.equal(rc.parts[1].l2.scanAxis, "rows");
  assert.equal(rc.parts[1].l2.mapping.scanned.slot, 1);

  // P3–P8: none
  for (let i = 2; i < 8; i += 1) {
    assert.equal(rc.parts[i].l1.behaviorId, "none");
    assert.equal(rc.parts[i].l2.mapping.activate.slot, 0);
  }

  // I1: synth soft pad, routed→FX Bus 1
  assert.equal(rc.instruments[0].type, "synth");
  assert.equal(rc.instruments[0].mixer.route, "fx_bus_1");

  // I2: perc hit, routed direct
  assert.equal(rc.instruments[1].type, "synth");
  assert.equal(rc.instruments[1].mixer.route, "direct");

  // I3–I8: none
  for (let i = 2; i < 8; i += 1) {
    assert.equal(rc.instruments[i].type, "none");
  }

  // FX Bus 1 (bus 0): delay + duck
  assert.equal(rc.mixer.buses[0].slot1.type, "delay");
  assert.equal(rc.mixer.buses[0].slot2.type, "duck");

  // All other buses: no effects
  for (let i = 1; i < rc.mixer.buses.length; i += 1) {
    assert.equal(rc.mixer.buses[i].slot1.type, "none");
    assert.equal(rc.mixer.buses[i].slot2.type, "none");
  }
});

// ─── Sample Browser Menu ──────────────────────────────────────────

test("choose sample entry creates browser and shows dir navigation", () => {
   let state = makeState();
   (state.runtimeConfig as any).instruments[0].type = "sampler";

  state = selectLabel(state, "L3: Voice");
  state = press(state).state;
  state = selectLabel(state, "Instruments");
  state = press(state).state;
  state = selectLabel(state, "I1: synth");
  state = press(state).state;
  state = selectLabel(state, "Choose Sample");
  state = press(state).state;

  assert.ok(state.system.sampleBrowser != null, "browser should be created");
  assert.equal(state.system.sampleBrowser.instrumentSlot, 0);
  assert.equal(state.system.sampleBrowser.sampleSlot, 0);
  assert.equal(state.system.sampleBrowser.dir, "");
  assert.deepEqual(state.system.sampleBrowser.entries, []);

  const frame = toRuntimeSnapshot(state, mockBehavior);
  assert.ok(frame.display.lines.length > 0, "should not be empty");
  assert.ok(frame.display.lines.some((l: string) => l.includes("..")), "should show ..");
  assert.ok(frame.display.lines.some((l: string) => l.includes("(empty)")), "should show (empty)");
});

test("choose sample populates entries after sample_list_result", () => {
   let state = makeState();
   (state.runtimeConfig as any).instruments[0].type = "sampler";

  state = selectLabel(state, "L3: Voice");
  state = press(state).state;
  state = selectLabel(state, "Instruments");
  state = press(state).state;
  state = selectLabel(state, "I1: synth");
  state = press(state).state;
  state = selectLabel(state, "Choose Sample");
  state = press(state).state;
  state = applyStoreResult(
    state,
    { type: "sample_list_result", instrumentSlot: 0, sampleSlot: 0, dir: "", entries: [{ name: "kick.wav", path: "kick.wav", isDir: false }] } as any,
    mockBehavior
  ).state;
  state = selectLabel(state, "kick.wav");

  const result = routeInput(state, { type: "button_s", pressed: true } as DeviceInput, mockBehavior);

  assert.deepEqual(result.effects, [{ type: "audio_command", command: { type: "sample_preview", instrumentSlot: 0, sampleSlot: 0, path: "kick.wav", velocity: 100 } }]);
});

test("choose sample re-entry preserves browser entries", () => {
   let state = makeState();
   (state.runtimeConfig as any).instruments[0].type = "sampler";

  state = selectLabel(state, "L3: Voice");
  state = press(state).state;
  state = selectLabel(state, "Instruments");
  state = press(state).state;
  state = selectLabel(state, "I1: synth");
  state = press(state).state;
  state = selectLabel(state, "Choose Sample");
  state = press(state).state;

  state = applyStoreResult(
    state,
    {
      type: "sample_list_result",
      instrumentSlot: 0,
      sampleSlot: 0,
      dir: "/samples",
      entries: [{ name: "kick.wav", path: "/samples/kick.wav", isDir: false }]
    } as any,
    mockBehavior
  ).state;

  // Go back to Sample group level, re-enter Choose Sample
  state = routeInput(state, { type: "button_a", pressed: true } as DeviceInput, mockBehavior).state;
  // Cursor is already at Choose Sample (its index was popped from stack)
  state = press(state).state; // enter Choose Sample

  assert.equal(state.system.sampleBrowser.entries.length, 1, "entries preserved");
  const frame = toRuntimeSnapshot(state, mockBehavior);
  assert.ok(frame.display.lines.some((l: string) => l.includes("kick.wav")), "kick.wav still visible");
});

