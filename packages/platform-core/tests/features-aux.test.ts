import test from "node:test";
import assert from "node:assert/strict";
import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import { type DeviceInput } from "@cellsymphony/device-contracts";
import { lifeBehavior } from "@cellsymphony/behaviors-life";
import {
  applyConfigPayload,
  createInitialState,
  extractConfigPayload,
  OLED_TEXT_LINES,
  PLATFORM_CAPS,
  routeInput,
  tick,
  toSimulatorFrame,
  type PlatformEffect,
  type PlatformState
} from "../src/index";

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
  s.system.auxAutoMapEnabled = false;
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
    const frame = toSimulatorFrame(state, mockBehavior);
    const selected = frame.display.lines.find((l) => l.startsWith("@@")) ?? "";
    if (selected.includes(label)) return state;
    const r = turn(state, 1);
    state = r.state;
  }
  throw new Error(`failed to select label: ${label}`);
}

// ─── Aux Encoder Binding ──────────────────────────────────────────

test("aux encoder bind while editing param", () => {
  let state = makeState();

  // Navigate to Master Vol and start editing
  state = selectLabel(state, "System");
  state = press(state).state;
  state = selectLabel(state, "Sound");
  state = press(state).state;
  state = press(state).state; // enter edit mode for Master Vol

  // Now shift+press an aux encoder to bind
  state.system.shiftHeld = true;
  const r = routeInput(state, { type: "encoder_press", id: "aux1" } as DeviceInput, mockBehavior);
  state = r.state;

  assert.ok(state.system.auxBindings["aux1"], "aux1 should be bound");
  assert.equal(state.system.auxBindings["aux1"]!.turn!.key, "masterVolume");
  assert.equal(state.system.auxBindings["aux1"]!.press, null);
});

test("aux bindings persist through config payload", () => {
  let state = makeState();
  state.runtimeConfig.autoSaveDefault = true;
  state.system.shiftHeld = true;

  // Bind Master Vol on aux1
  state = selectLabel(state, "System");
  state = press(state).state;
  state = selectLabel(state, "Sound");
  state = press(state).state;
  state = selectLabel(state, "Master Vol");
  state = routeInput(state, { type: "encoder_press", id: "aux1" } as DeviceInput, mockBehavior).state;

  const payload = extractConfigPayload(state);
  const restored = applyConfigPayload(createInitialState(mockBehavior), payload, mockBehavior);
  assert.ok(restored.system.auxBindings.aux1, "aux1 binding should restore");
  assert.equal(restored.system.auxBindings.aux1!.turn!.key, "masterVolume");
});

test("aux encoder bind while highlighting param (not editing)", () => {
  let state = makeState();

  // Navigate to Master Vol but do not enter edit mode
  state = selectLabel(state, "System");
  state = press(state).state;
  state = selectLabel(state, "Sound");
  state = press(state).state;

  // Shift+press aux to bind highlighted param
  state.system.shiftHeld = true;
  const r = routeInput(state, { type: "encoder_press", id: "aux1" } as DeviceInput, mockBehavior);
  state = r.state;

  assert.ok(state.system.auxBindings["aux1"], "aux1 should be bound");
  assert.equal(state.system.auxBindings["aux1"]!.turn!.key, "masterVolume");
});

test("shift-hold shows current mapping overlay (custom)", () => {
  let state = makeState();
  state.system.auxBindings["aux1"] = { turn: { key: "masterVolume", label: "Master Vol", kind: "number", min: 0, max: 100, step: 1 }, press: null };
  state.system.shiftHeld = true;
  state.system.shiftHeldSinceMs = Date.now() - 2500;

  const frame = toSimulatorFrame(state, mockBehavior);
  assert.equal(frame.display.title, "CUSTOM MAP");
  assert.ok(frame.display.lines.some((l) => l.startsWith("A1")), "overlay should list aux slots");
});

test("shift-hold mapping overlay waits for delay", () => {
  let state = makeState();
  state.system.auxBindings["aux1"] = { turn: { key: "masterVolume", label: "Master Vol", kind: "number", min: 0, max: 100, step: 1 }, press: null };
  state.system.shiftHeld = true;
  state.system.shiftHeldSinceMs = Date.now() - 1400;

  const frame = toSimulatorFrame(state, mockBehavior);
  assert.notEqual(frame.display.title, "CUSTOM MAP");
});

test("shift-hold shows current mapping overlay (auto)", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.auxAutoMapEnabled = true;
  state.system.shiftHeld = true;
  state.system.shiftHeldSinceMs = Date.now() - 2500;
  state.system.activeFx = [
    { cellX: 0, cellY: 0, fxType: "stutter", config: { fxType: "stutter", params: { rateHz: 8, depthPct: 100 }, targetKey: "master" }, activatedAtMs: 0 }
  ] as any;

  const frame = toSimulatorFrame(state, mockBehavior);
  assert.equal(frame.display.title, "AUTO MAP");
  assert.ok(frame.display.lines.join("\n").includes("Rate"));
});

test("aux encoder unbind when pressing same param again", () => {
  let state = makeState();

  state = selectLabel(state, "System");
  state = press(state).state;
  state = selectLabel(state, "Sound");
  state = press(state).state;
  state = press(state).state;

  // Bind with shift+press
  state.system.shiftHeld = true;
  state = routeInput(state, { type: "encoder_press", id: "aux1" } as DeviceInput, mockBehavior).state;
  assert.ok(state.system.auxBindings["aux1"], "should be bound");

  // Unbind (same param, same aux encoder, shift+press again)
  state.system.shiftHeld = true;
  state = routeInput(state, { type: "encoder_press", id: "aux1" } as DeviceInput, mockBehavior).state;
  assert.ok(state.system.confirm, "should open unbind confirm");
  state = routeInput(state, { type: "encoder_press", id: "main" } as DeviceInput, mockBehavior).state;
  assert.equal(state.system.auxBindings["aux1"], null, "should be unbound");
});

test("aux encoder shift+press unbinds bound param when nothing bindable", () => {
  let state = makeState();
  state.system.auxBindings["aux1"] = { turn: { key: "masterVolume", label: "Master Vol", kind: "number", min: 0, max: 100, step: 1 }, press: null };
  state.system.shiftHeld = true;

  const r = routeInput(state, { type: "encoder_press", id: "aux1" } as DeviceInput, mockBehavior);
  state = r.state;

  assert.ok(state.system.confirm, "should open unbind confirm");
  state = routeInput(state, { type: "encoder_press", id: "main" } as DeviceInput, mockBehavior).state;
  assert.equal(state.system.auxBindings["aux1"], null, "should be unbound");
});

test("aux encoder shift+press on unbound does nothing else harmful", () => {
  let state = makeState();
  state.system.shiftHeld = true;

  const r = routeInput(state, { type: "encoder_press", id: "aux1" } as DeviceInput, mockBehavior);
  state = r.state;

  assert.equal(state.system.auxBindings["aux1"] ?? null, null, "should remain unbound");
  assert.equal(state.system.toast?.message, "S1: No binding");
});

test("aux encoder turn adjusts bound param", () => {
  let state = makeState();
  state.system.auxBindings["aux1"] = { turn: { key: "masterVolume", label: "Master Vol", kind: "number", min: 0, max: 100, step: 1 }, press: null };
  state.runtimeConfig.masterVolume = 50;

  const r = routeInput(state, { type: "encoder_turn", id: "aux1", delta: 1 } as DeviceInput, mockBehavior);
  state = r.state;

  assert.equal(state.runtimeConfig.masterVolume, 51);
  assert.equal(state.system.toast?.message, "T1: Master Vol: Vol: 51%");
});

test("aux encoder turn adjusts bound behaviorConfig param", () => {
  let state = createInitialState(lifeBehavior);
  state.system.oledMode = "normal";
  state.system.auxAutoMapEnabled = false;
  state.runtimeConfig.activeBehavior = "life";
  state.runtimeConfig.behaviorConfig = { life: { randomCellsPerTick: 5, randomTickInterval: 2 } } as any;
  state.system.auxBindings["aux1"] = { turn: { key: "behaviorConfig.life.randomCellsPerTick", label: "Spawn Count", kind: "number", min: 0, max: 20, step: 1 }, press: null };

  const r = routeInput(state, { type: "encoder_turn", id: "aux1", delta: 1 } as DeviceInput, mockBehavior);
  state = r.state;

  assert.equal((state.runtimeConfig.behaviorConfig as any).life?.randomCellsPerTick, 6);
  assert.equal(state.system.toast?.message, "T1 L1 Spawn Count: 6");
});

test("aux encoder turn updates per-part behaviorConfig in partStates live state", () => {
  let state = createInitialState(lifeBehavior);
  state.system.oledMode = "normal";
  state.system.auxAutoMapEnabled = false;
  state.runtimeConfig.activeBehavior = "life";
  state.runtimeConfig.activePartIndex = 0;
  // Use the full part structure from createInitialState, just set behaviorConfig
  state.runtimeConfig.parts[0].l1.behaviorConfig = { randomCellsPerTick: 5, randomTickInterval: 2 };
  state.runtimeConfig.parts[0].l1.behaviorId = "life";
  // Populate some grid cells to verify they survive
  const cellCount = PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight;
  const cells = new Array(cellCount).fill(false);
  cells[0] = true;
  cells[1] = true;
  state.behaviorState = { ...state.behaviorState, cells, tickCounter: 10, randomCellsPerTick: 5 };
  state.partStates[0] = state.behaviorState;
  // Bind to per-part key (the format that was broken)
  state.system.auxBindings["aux1"] = { turn: { key: "parts.0.l1.behaviorConfig.randomCellsPerTick", label: "Spawn Count", kind: "number", min: 0, max: 20, step: 1 }, press: null };

  const r = routeInput(state, { type: "encoder_turn", id: "aux1", delta: 1 } as DeviceInput, lifeBehavior);
  state = r.state;

  // Config is updated
  assert.equal(state.runtimeConfig.parts[0]?.l1?.behaviorConfig?.randomCellsPerTick, 6);
  // Behavior live state is updated
  assert.equal((state.behaviorState as any).randomCellsPerTick, 6);
  assert.equal((state.partStates[0] as any).randomCellsPerTick, 6);
  // Grid cells are preserved (no reinit)
  assert.equal(state.behaviorState.cells[0], true);
  assert.equal(state.behaviorState.cells[1], true);
  // tickCounter is preserved
  assert.equal((state.behaviorState as any).tickCounter, 10);
  assert.equal(state.system.toast?.message, "T1 L1>P1 Spawn Count: 6");
});

test("aux encoder press triggers bound behavior action", () => {
  let state = createInitialState(lifeBehavior);
  state.system.oledMode = "normal";
  state.system.auxAutoMapEnabled = false;
  state.runtimeConfig.activeBehavior = "life";
  state.system.auxBindings["aux1"] = { turn: null, press: { kind: "behavior_action", actionType: "spawnRandom", label: "Spawn Random" } };

  const before = state.behaviorState.cells.filter(Boolean).length;
  const r = routeInput(state, { type: "encoder_press", id: "aux1" } as DeviceInput, lifeBehavior);
  state = r.state;
  const after = state.behaviorState.cells.filter(Boolean).length;

  assert.ok(after > before, "bound behavior action should change behavior state");
  assert.equal(state.system.toast?.message, "S1 L1>P1 Spawn Random");
});

test("spawn action label shows auto-map prefix and shared marker in menu", () => {
  let state = createInitialState(lifeBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.activeBehavior = "life";

  state = selectLabel(state, "L1: Life");
  state = press(state).state;
  state = selectLabel(state, "P1: mock");
  state = press(state).state;
  state = selectLabel(state, "Spawn Random");

  const frame = toSimulatorFrame(state, mockBehavior);
  const selected = frame.display.lines.find((l) => l.startsWith("@@")) ?? "";
  assert.ok(selected.includes("!1-Spawn Random [S]"));
});

test("binding spawn action stores shared route", () => {
  let state = createInitialState(lifeBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.activeBehavior = "life";

  state = selectLabel(state, "L1: Life");
  state = press(state).state;
  state = selectLabel(state, "P1: mock");
  state = press(state).state;
  state = selectLabel(state, "Spawn Random");
  state.system.shiftHeld = true;
  state = routeInput(state, { type: "encoder_press", id: "aux1" } as DeviceInput, mockBehavior).state;

  assert.equal((state.system.auxBindings["aux1"]?.press as any)?.routeKey, "trigger.life.spawn_now");
  assert.equal((state.system.auxBindings["aux1"]?.press as any)?.label, "Spawn Now");
});

test("menu view reserves bottom line", () => {
  const state = makeState();
  const frame = toSimulatorFrame(state, mockBehavior);
  assert.ok(frame.display.lines.length <= OLED_TEXT_LINES - 2);
});

test("auto-map works in synth filter", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.auxAutoMapEnabled = true;
  (state.runtimeConfig as any).instruments[0].type = "synth";
  (state.runtimeConfig as any).instruments[0].synth.filter.cutoffHz = 100;

  state = selectLabel(state, "L3: Voice");
  state = press(state).state;
  state = selectLabel(state, "Instruments");
  state = press(state).state;
  state = selectLabel(state, "I1:");
  state = press(state).state;
  state = selectLabel(state, "Synth");
  state = press(state).state;
  state = selectLabel(state, "Filter");
  state = press(state).state;
  state = selectLabel(state, "Cutoff");

  const frame = toSimulatorFrame(state, mockBehavior);
  assert.ok(frame.display.lines.join("\n").includes("1-Cutoff"));

  state = routeInput(state, { type: "encoder_turn", id: "aux1", delta: 1 } as DeviceInput, mockBehavior).state;
  assert.equal((state.runtimeConfig as any).instruments[0].synth.filter.cutoffHz, 101);
});

test("auto-map A1 press enters sample assign mode", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.auxAutoMapEnabled = true;
  (state.runtimeConfig as any).instruments[0].type = "sample";
  (state.runtimeConfig as any).instruments[0].sample.selectedSlot = 0;

  state = selectLabel(state, "L3: Voice");
  state = press(state).state;
  state = selectLabel(state, "Instruments");
  state = press(state).state;
  state = selectLabel(state, "I1:");
  state = press(state).state;
  state = selectLabel(state, "Sample");
  state = press(state).state;
  state = selectLabel(state, "Assign");

  const frame = toSimulatorFrame(state, mockBehavior);
  const selected = frame.display.lines.find((l) => l.startsWith("@@")) ?? "";
  assert.ok(selected.includes("!1-Assign"), "action row should render as !1-Assign");

  state = routeInput(state, { type: "encoder_press", id: "aux1" } as DeviceInput, mockBehavior).state;
  assert.deepEqual(state.system.sampleAssign, { instrumentSlot: 0, sampleSlot: 0 });
});

test("shared spawn route shows N/A toast in sequencer", () => {
  let state = makeState();
  state.runtimeConfig.activeBehavior = "sequencer";
  state.system.auxBindings["aux1"] = {
    turn: null,
    press: { kind: "behavior_action", actionType: "spawnRandom", routeKey: "trigger.life.spawn_now", label: "Spawn Now" }
  };

  state = routeInput(state, { type: "encoder_press", id: "aux1" } as DeviceInput, mockBehavior).state;
  assert.equal(state.system.toast?.message, "S1: P1 Spawn Now not active");
});

test("aux encoder turn bool binding clamps in both directions", () => {
  let state = makeState();
  state.runtimeConfig.midi.enabled = false;
  state.system.auxBindings["aux1"] = { turn: { key: "midi.enabled", label: "MIDI Enabled", kind: "bool" }, press: null };

  state = routeInput(state, { type: "encoder_turn", id: "aux1", delta: -1 } as DeviceInput, mockBehavior).state;
  assert.equal(state.runtimeConfig.midi.enabled, false);
  assert.equal(state.system.toast?.message, "T1: MIDI Enabled: Off");

  state = routeInput(state, { type: "encoder_turn", id: "aux1", delta: 1 } as DeviceInput, mockBehavior).state;
  assert.equal(state.runtimeConfig.midi.enabled, true);
  assert.equal(state.system.toast?.message, "T1: MIDI Enabled: On");

  state = routeInput(state, { type: "encoder_turn", id: "aux1", delta: 1 } as DeviceInput, mockBehavior).state;
  assert.equal(state.runtimeConfig.midi.enabled, true);
});

test("aux encoder turn enum binding clamps and does not wrap", () => {
  let state = makeState();
  state.runtimeConfig.scanMode = "immediate";
  state.system.auxBindings["aux1"] = { turn: { key: "scanMode", label: "Scan Mode", kind: "enum", options: ["immediate", "scanning"] }, press: null };

  state = routeInput(state, { type: "encoder_turn", id: "aux1", delta: -1 } as DeviceInput, mockBehavior).state;
  assert.equal(state.runtimeConfig.scanMode, "immediate");

  state = routeInput(state, { type: "encoder_turn", id: "aux1", delta: 1 } as DeviceInput, mockBehavior).state;
  assert.equal(state.runtimeConfig.scanMode, "scanning");

  state = routeInput(state, { type: "encoder_turn", id: "aux1", delta: 1 } as DeviceInput, mockBehavior).state;
  assert.equal(state.runtimeConfig.scanMode, "scanning");
});

test("aux press spawn action remaps on behavior switch", () => {
  let state = createInitialState(lifeBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.activeBehavior = "life";
  state.activeBehavior = "life";
  state.system.auxBindings["aux1"] = { turn: null, press: { kind: "behavior_action", actionType: "spawnRandom", label: "Spawn Random" } };

  state = selectLabel(state, "L1: Life");
  state = press(state).state;
  state = selectLabel(state, "P1: mock");
  state = press(state).state;
  state = selectLabel(state, "Behavior");
  state = press(state).state;
  state = turn(state, 3).state;

  assert.equal(state.runtimeConfig.activeBehavior, "brain");
  assert.equal((state.system.auxBindings["aux1"]?.press as any)?.actionType, "seedRandom");
  assert.equal((state.system.auxBindings["aux1"]?.press as any)?.label, "Seed Random");
});

test("aux press spawn action clears on switch to sequencer", () => {
  let state = createInitialState(lifeBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.activeBehavior = "life";
  state.activeBehavior = "life";
  state.system.auxBindings["aux1"] = { turn: null, press: { kind: "behavior_action", actionType: "spawnRandom", label: "Spawn Random" } };

  state = selectLabel(state, "L1: Life");
  state = press(state).state;
  state = selectLabel(state, "P1: mock");
  state = press(state).state;
  state = selectLabel(state, "Behavior");
  state = press(state).state;
  state = turn(state, 1).state;

  assert.equal(state.runtimeConfig.activeBehavior, "sequencer");
  assert.equal(state.system.auxBindings["aux1"], null);
});

test("aux encoder press shows no binding toast when unbound", () => {
  let state = makeState();
  state = routeInput(state, { type: "encoder_press", id: "aux1" } as DeviceInput, mockBehavior).state;
  assert.equal(state.system.toast?.message, "S1: No binding");
});

test("aux encoder turn shows no binding toast when unbound", () => {
  let state = makeState();
  state = routeInput(state, { type: "encoder_turn", id: "aux1", delta: 1 } as DeviceInput, mockBehavior).state;
  assert.equal(state.system.toast?.message, "T1: No binding");
});

test("aux toast updates and extends while already visible", () => {
  let state = makeState();
  state.runtimeConfig.masterVolume = 50;
  state.system.auxBindings["aux1"] = { turn: { key: "masterVolume", label: "Master Vol", kind: "number", min: 0, max: 100, step: 1 }, press: null };

  state = routeInput(state, { type: "encoder_turn", id: "aux1", delta: 1 } as DeviceInput, mockBehavior).state;
  const firstUntil = state.system.toast!.untilMs;

  state = routeInput(state, { type: "encoder_turn", id: "aux1", delta: 1 } as DeviceInput, mockBehavior).state;
  const secondUntil = state.system.toast!.untilMs;

  assert.equal(state.system.toast?.message, "T1: Master Vol: Vol: 52%");
  assert.ok(secondUntil > firstUntil, "toast lifetime should extend on update");
});

test("aux unbind confirm supports Click and Turn choices", () => {
  let state = makeState();
  state.system.shiftHeld = true;
  state.system.auxBindings["aux1"] = {
    turn: { key: "masterVolume", label: "Master Vol", kind: "number", min: 0, max: 100, step: 1 },
    press: { actionType: "spawnRandom", label: "Spawn Random" }
  };

  state = routeInput(state, { type: "encoder_press", id: "aux1" } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "encoder_turn", id: "main", delta: 1 } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "encoder_press", id: "main" } as DeviceInput, mockBehavior).state;
  assert.equal(state.system.auxBindings["aux1"]?.press, null);
  assert.ok(state.system.auxBindings["aux1"]?.turn);

  state.system.shiftHeld = true;
  state = routeInput(state, { type: "encoder_press", id: "aux1" } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "encoder_turn", id: "main", delta: 1 } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "encoder_turn", id: "main", delta: 1 } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "encoder_press", id: "main" } as DeviceInput, mockBehavior).state;
  assert.equal(state.system.auxBindings["aux1"], null);
});

test("aux unbind confirm cancel keeps binding", () => {
  let state = makeState();
  state.system.shiftHeld = true;
  state.system.auxBindings["aux1"] = { turn: { key: "masterVolume", label: "Master Vol", kind: "number", min: 0, max: 100, step: 1 }, press: null };

  state = routeInput(state, { type: "encoder_press", id: "aux1" } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "encoder_turn", id: "main", delta: 1 } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "encoder_turn", id: "main", delta: 1 } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "encoder_turn", id: "main", delta: 1 } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "encoder_press", id: "main" } as DeviceInput, mockBehavior).state;
  assert.ok(state.system.auxBindings["aux1"]);
});

// ─── Stale Aux Binding Detection ──────────────────────────────────

test("stale FX param turn shows not active toast", () => {
  let state = makeState();
  // Set slot1 to delay with params
  state.runtimeConfig.mixer = {
    buses: [{ slot1: { type: "delay", params: { timeMs: 200 } as any }, slot2: { type: "none", params: {} as any } }]
  } as any;
  state.system.auxBindings["aux1"] = {
    turn: { key: "mixer.buses.0.slot1.params.timeMs", label: "Time ms", kind: "number", min: 1, max: 2000, step: 1 },
    press: null
  };

  // Change slot type to reverb — timeMs no longer valid
  state.runtimeConfig.mixer.buses[0].slot1.type = "reverb";

  const r = routeInput(state, { type: "encoder_turn", id: "aux1", delta: 1 } as DeviceInput, mockBehavior);
  state = r.state;

  assert.equal(state.system.toast?.message, "T1: B1 Time ms not active");
});

test("stale instrument type turn shows not active toast", () => {
  let state = makeState();
  state.runtimeConfig.instruments = [
    { type: "synth", autoName: true, name: "synth", synth: { filterCutoff: 0.5 } as any }
  ] as any;
  state.system.auxBindings["aux1"] = {
    turn: { key: "instruments.0.synth.filterCutoff", label: "Filter cutoff", kind: "number", min: 0, max: 1, step: 0.01 },
    press: null
  };

  // Change instrument type to sample — synth subtree inactive
  state.runtimeConfig.instruments[0].type = "sample";

  const r = routeInput(state, { type: "encoder_turn", id: "aux1", delta: 1 } as DeviceInput, mockBehavior);
  state = r.state;

  assert.equal(state.system.toast?.message, "T1: I1 Filter cutoff not active");
});

test("stale part scan turn shows not active toast", () => {
  let state = makeState();
  state.runtimeConfig.parts = [
    { l1: { behaviorId: "life", autoName: true, name: "life", behaviorConfig: {} }, l2: { scanMode: "scanning", scanAxis: "rows", scanUnit: 1, scanDirection: "forward" } as any }
  ] as any;
  state.system.auxBindings["aux1"] = {
    turn: { key: "parts.0.l2.scanDirection", label: "Scan Direction", kind: "enum", options: ["forward", "reverse"] },
    press: null
  };

  // Change scan mode to immediate — scan direction inactive
  state.runtimeConfig.parts[0].l2.scanMode = "immediate";

  const r = routeInput(state, { type: "encoder_turn", id: "aux1", delta: 1 } as DeviceInput, mockBehavior);
  state = r.state;

  assert.equal(state.system.toast?.message, "T1: P1 Scan Direction not active");
});

test("stale concrete behavior action press shows not active toast", () => {
  let state = createInitialState(lifeBehavior);
  state.system.oledMode = "normal";
  state.system.auxAutoMapEnabled = false;
  state.runtimeConfig.activeBehavior = "life";
  state.activeBehavior = "life";
  state.system.auxBindings["aux1"] = {
    turn: null,
    press: { actionType: "spawnRandom", label: "Spawn Random" }
  };

  // Switch active behavior to brain — spawnRandom not in brain's config menu
  state.runtimeConfig.activeBehavior = "brain";
  state.activeBehavior = "brain";

  const r = routeInput(state, { type: "encoder_press", id: "aux1" } as DeviceInput, lifeBehavior);
  state = r.state;

  assert.equal(state.system.toast?.message, "S1: P1 Spawn Random not active");
});

test("stale spawn route press shows not active toast", () => {
  let state = createInitialState(lifeBehavior);
  state.system.oledMode = "normal";
  state.system.auxAutoMapEnabled = false;
  state.runtimeConfig.activeBehavior = "life";
  state.activeBehavior = "life";
  state.system.auxBindings["aux1"] = {
    turn: null,
    press: { actionType: "spawnRandom", routeKey: "trigger.life.spawn_now", label: "Spawn Now" }
  };

  // Switch to sequencer — no spawn action
  state.runtimeConfig.activeBehavior = "sequencer";
  state.activeBehavior = "sequencer";

  const r = routeInput(state, { type: "encoder_press", id: "aux1" } as DeviceInput, lifeBehavior);
  state = r.state;

  assert.equal(state.system.toast?.message, "S1: P1 Spawn Now not active");
});

// ─── Shift+Back Grid Clear ────────────────────────────────────────

test("shift+back clears grid and shows toast", () => {
  let state = makeState();
  state.system.shiftHeld = true;
  state.runtimeConfig.activeBehavior = "life";

  const r = routeInput(state, { type: "button_a", pressed: true } as DeviceInput, mockBehavior);
  state = r.state;

  assert.ok(state.system.toast, "should show toast");
  assert.equal(state.system.toast!.message, "Grid cleared");
  // Behavior state should be re-initialized (all cells false for life behavior)
  // Since we're using mockBehavior here, just verify no error
});

test("shift+back with life behavior reinitializes cells", () => {
  let state = createInitialState(lifeBehavior);
  state.system.oledMode = "normal";
  state.system.shiftHeld = true;
  state.runtimeConfig.activeBehavior = "life";

  // Set some cells on
  const bs = state.behaviorState;
  bs.cells[0] = true;
  bs.cells[10] = true;
  bs.cells[20] = true;

  const r = routeInput(state, { type: "button_a", pressed: true } as DeviceInput, lifeBehavior);
  state = r.state;

  const aliveCount = state.behaviorState.cells.filter(Boolean).length;
  assert.equal(aliveCount, 0, "all cells should be cleared");
  assert.ok(state.system.toast, "should show toast");
});
