import test from "node:test";
import assert from "node:assert/strict";
import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import { type DeviceInput } from "@cellsymphony/device-contracts";
import { lifeBehavior } from "@cellsymphony/behaviors-life";
import {
  applyConfigPayload,
  createInitialState,
  GRID_DOMAIN,
  PLATFORM_CAPS,
  emergencyBrake,
  extractConfigPayload,
  routeInput,
  tick,
  toSimulatorFrame,
  type PlatformEffect,
  type PlatformState
} from "../src/index";
import { validatePlatformCapabilities } from "../src/platformCaps";
import { writeAnyValue } from "../src/stateHelpers";
import { filterTriggerGatedIntents } from "../src/runtimeHelpers";

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
    const frame = toSimulatorFrame(state, mockBehavior);
    const selected = frame.display.lines.find((l) => l.startsWith("@@")) ?? "";
    if (selected.includes(label)) return state;
    const r = turn(state, 1);
    state = r.state;
  }
  throw new Error(`failed to select label: ${label}`);
}

// ─── Aux Encoder Binding ──────────────────────────────────────────

// ─── Config Payload Round-Trip ────────────────────────────────────

test("extractConfigPayload and applyConfigPayload round-trip preserves state", () => {
  let state = makeState();
  state.runtimeConfig.masterVolume = 42;
  state.runtimeConfig.algorithmStepUnit = "1/4";
  state.runtimeConfig.activeBehavior = "sequencer";

  const payload = extractConfigPayload(state);
  assert.equal(payload.runtimeConfig.masterVolume, 42);
  assert.equal(payload.runtimeConfig.algorithmStepUnit, "1/4");
  assert.equal(payload.activeBehavior, "sequencer");

  const restored = applyConfigPayload(state, payload, mockBehavior);
  assert.equal(restored.runtimeConfig.masterVolume, 42);
  assert.equal(restored.runtimeConfig.algorithmStepUnit, "1/4");
});

test("applyConfigPayload reinitializes behavior state when behavior changes", () => {
  let state = createInitialState(lifeBehavior);
  const payload = extractConfigPayload(state);
  payload.activeBehavior = "sequencer";

  const restored = applyConfigPayload(state, payload, lifeBehavior);
  assert.equal(restored.activeBehavior, "sequencer");
});

test("platform capabilities validator rejects invalid values", () => {
  assert.throws(() => validatePlatformCapabilities({ gridWidth: 0, gridHeight: 8, partCount: 8, instrumentCount: 16, sampleSlotCount: 8, busCount: 4, touchFxMaxConcurrent: 4 }));
  assert.throws(() => validatePlatformCapabilities({ gridWidth: 8, gridHeight: 8, partCount: -1, instrumentCount: 16, sampleSlotCount: 8, busCount: 4, touchFxMaxConcurrent: 4 }));
  assert.throws(() => validatePlatformCapabilities({ gridWidth: 8, gridHeight: 8, partCount: 8, instrumentCount: 16.5, sampleSlotCount: 8, busCount: 4, touchFxMaxConcurrent: 4 }));
  assert.throws(() => validatePlatformCapabilities({ gridWidth: 8, gridHeight: 8, partCount: 8, instrumentCount: 16, sampleSlotCount: 8, busCount: 4, touchFxMaxConcurrent: 0 }));
});

test("applyConfigPayload reinitializes behavior state for same behavior id using saved behaviorConfig", () => {
  let state = createInitialState(lifeBehavior);
  (state.runtimeConfig as any).parts[0].l1.saveGridState = false;
  const payload = extractConfigPayload(state);
  payload.activeBehavior = "life";
  (payload.runtimeConfig.behaviorConfig as any).life = { randomCellsPerTick: 11, randomTickInterval: 2 };

  const restored = applyConfigPayload(state, payload, lifeBehavior);
  assert.equal(restored.activeBehavior, "life");
  assert.equal((restored.behaviorState as any).randomCellsPerTick, 11);
  assert.equal((restored.behaviorState as any).randomTickInterval, 2);
});

test("applyConfigPayload clears transient runtime state on load", () => {
  let state = makeState();
  state.scanPulseAccumulator = 99;
  state.algorithmPulseAccumulator = 77;
  state.ppqnPulseRemainder = 0.5;
  state.scanIndex = 12;
  state.system.heldNotes = ["0:60"];
  state.system.pendingResync = true;
  state.system.externalPpqnPulse = 42;
  const payload = extractConfigPayload(state);

  const restored = applyConfigPayload(state, payload, mockBehavior);
  assert.equal(restored.scanPulseAccumulator, 0);
  assert.equal(restored.algorithmPulseAccumulator, 0);
  assert.equal(restored.ppqnPulseRemainder, 0);
  assert.equal(restored.scanIndex, 0);
  assert.deepEqual(restored.system.heldNotes, []);
  assert.equal(restored.system.pendingResync, false);
  assert.equal(restored.system.externalPpqnPulse, 0);
});

test("applyConfigPayload keeps active behavior state aligned to restored active part state", () => {
  let state = createInitialState(lifeBehavior) as any;
  state.runtimeConfig.activePartIndex = 2;
  state.runtimeConfig.parts[2].l1.saveGridState = true;
  state.partStates[2] = { ...state.partStates[2], tick: 17 };

  const restored = applyConfigPayload(state, extractConfigPayload(state), lifeBehavior) as any;
  assert.equal(restored.runtimeConfig.activePartIndex, 2);
  assert.equal(restored.partStates[2].tick, 17);
  assert.equal(restored.behaviorState.tick, 17);
});

test("algorithmStepUnit is included in config payload", () => {
  let state = makeState();
  state.runtimeConfig.algorithmStepUnit = "1/2";
  const payload = extractConfigPayload(state);
  assert.equal(payload.runtimeConfig.algorithmStepUnit, "1/2");
});

// ─── Algorithm Step Rate ──────────────────────────────────────────

test("algorithm pulse accumulator advances during tick", () => {
  let state = createInitialState(mockBehavior);
  state.transport.playing = true;
  state.runtimeConfig.algorithmStepUnit = "1/16";

  const result = tick(state, mockBehavior);
  assert.ok(result.state.algorithmPulseAccumulator > 0, "accumulator should advance");
  assert.ok(result.state.transport.ppqnPulse > 0, "ppqn should advance");
});

test("behavior onTick is called when accumulator reaches step threshold", () => {
  let state = createInitialState(mockBehavior);
  state.transport.playing = true;
  state.runtimeConfig.algorithmStepUnit = "1/16";

  const result = tick(state, mockBehavior);
  assert.equal(result.state.behaviorState.tickCount, 1, "onTick should be called once per step");
});

test("X/Y param modulation updates runtime mixer config on transport tick", () => {
  let state = createInitialState(mockBehavior) as any;
  state.transport.playing = true;
  state.runtimeConfig.algorithmStepUnit = "1/16";
  state.runtimeConfig.mixer.buses[0].slot1 = { type: "reverb", params: { decay: 0.5, damp: 0.35, mixPct: 30 } };
  state.runtimeConfig.parts[0].paramMods = {
    x: [{ key: "mixer.buses.0.slot1.params.decay", label: "Decay", kind: "number", min: 0, max: 0.995, step: 0.005, invert: false }, null],
    y: [null, null]
  };

  const result = tick(state, mockBehavior);

  assert.notEqual(result.state.runtimeConfig.mixer.buses[0].slot1.params.decay, 0.5);
});

// ─── Sense Menu Instrument Targets ────────────────────────────────

test("L2: Sense has event instrument targets accessible via menu", () => {
  let state = makeState();

  // Enable event triggers so instrument targets become visible
  state.runtimeConfig.parts[0].l2.eventEnabled = true;

  // Verify event targets (Activate, Stable, Deactivate) are now directly under Events (no nested Instrument Targets)
  state = selectLabel(state, "L2: Sense");
  state = press(state).state;
  state = selectLabel(state, "P1: mock");
  state = press(state).state;
  state = selectLabel(state, "Events");
  state = press(state).state;
  state = selectLabel(state, "Activate Instrument");
  state = press(state).state; // enter edit
  state = turn(state, 1).state; // 0 → "1"
  state = press(state).state; // exit edit
  assert.equal(String(state.mappingConfig.activate.channel), "1");

  state = selectLabel(state, "Stable Instrument");
  state = press(state).state;
  state = turn(state, 1).state;
  state = press(state).state;

  state = selectLabel(state, "Deactivate Instrument");
  state = press(state).state;
  state = turn(state, 1).state;
  state = press(state).state;

  assert.ok("activate" in state.mappingConfig, "activate target exists");
  assert.ok("stable" in state.mappingConfig, "stable target exists");
  assert.ok("deactivate" in state.mappingConfig, "deactivate target exists");
});

test("L2: Sense has scanning instrument targets accessible via menu", () => {
  let state = makeState();

  // Enable scanning mode so scanning instrument targets become visible
  state.runtimeConfig.parts[0].l2.scanMode = "scanning";

  state = selectLabel(state, "L2: Sense");
  state = press(state).state;
  state = selectLabel(state, "P1: mock");
  state = press(state).state;
  state = selectLabel(state, "Scanning");
  state = press(state).state;

  state = selectLabel(state, "Instrument");
  state = press(state).state;
  state = turn(state, 1).state;
  state = press(state).state;

  assert.ok("scanned" in state.mappingConfig, "scanned target exists");
});

test("stable target is separate from activate and deactivate", () => {
  let state = makeState();

  // Enable event triggers so instrument targets become visible
  state.runtimeConfig.parts[0].l2.eventEnabled = true;

  state = selectLabel(state, "L2: Sense");
  state = press(state).state;
  state = selectLabel(state, "P1: mock");
  state = press(state).state;
  state = selectLabel(state, "Events");
  state = press(state).state;

  // Set activate to channel 0
  state = selectLabel(state, "Activate Instrument");
  state = press(state).state;
  // Ensure we're at 0 by turning down a couple times.
  state = turn(state, -1).state;
  state = turn(state, -1).state;
  state = press(state).state;

  // Set stable to channel 2
  state = selectLabel(state, "Stable Instrument");
  state = press(state).state;
  state = turn(state, 1).state;
  state = turn(state, 1).state;
  state = press(state).state;

  assert.equal(state.mappingConfig.activate.channel, 0);
  assert.equal(state.mappingConfig.stable.channel, 2);
  assert.notEqual(state.mappingConfig.activate.channel, state.mappingConfig.stable.channel);
});

test("loading synth preset from Voice menu requires confirm and applies to target slot", () => {
  let state = makeState();
  const beforeGain = (state.runtimeConfig as any).instruments?.[0]?.synth?.amp?.gainPct;

  state = selectLabel(state, "L3: Voice");
  state = press(state).state;
  state = selectLabel(state, "Instruments");
  state = press(state).state;
  state = selectLabel(state, "I1: synth");
  state = press(state).state;
  state = selectLabel(state, "Synth");
  state = press(state).state;
  state = selectLabel(state, "Preset");
  state = press(state).state;
  state = selectLabel(state, "Load");
  state = press(state).state;
  state = selectLabel(state, "soft pad");
  state = press(state).state;

  assert.equal(state.system.confirm?.kind, "load_synth_preset");

  state = routeInput(state, { type: "encoder_turn", delta: 1 } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "encoder_press" } as DeviceInput, mockBehavior).state;

  assert.equal(state.system.confirm, null);
  const afterGain = (state.runtimeConfig as any).instruments?.[0]?.synth?.amp?.gainPct;
  assert.notEqual(afterGain, beforeGain);
  assert.equal(afterGain, 72);
});

// ─── Spacer skipping ──────────────────────────────────────────────

test("menu navigation skips spacers when turning", () => {
  let state = makeState();
  const initialCursor = state.menu.cursor;
  // Navigate past any spacers
  for (let i = 0; i < 10; i++) {
    const r = turn(state, 1);
    state = r.state;
  }
  // Should never land on a spacer
  const frame = toSimulatorFrame(state, mockBehavior);
  const selected = frame.display.lines.find((l) => l.startsWith("@@")) ?? "";
  assert.ok(!selected.includes("─"), "should not select spacer");
});

// ─── Shift+Back in text editing (backspace) ───────────────────────

test("switching parts restores stored part state immediately", () => {
  let state = makeState() as any;
  state.runtimeConfig.parts[1].l1.behaviorId = "mock";
  state.runtimeConfig.parts[1].l1.behaviorConfig = {};
  state.partStates[0] = { cells: Array.from({ length: CELL_COUNT }, (_, i) => i === 0), tickCount: 11 };
  state.partStates[1] = { cells: Array.from({ length: CELL_COUNT }, (_, i) => i === 1), tickCount: 22 };
  state.behaviorState = state.partStates[0];

  state = routeInput(state, { type: "button_fn", pressed: true } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "grid_press", x: 0, y: 1 } as DeviceInput, mockBehavior).state;

  assert.equal(state.runtimeConfig.activePartIndex, 1);
  assert.equal(state.behaviorState.tickCount, 22);
});

test("switching active part does not overwrite playback timing accumulators", () => {
  let state = createInitialState(lifeBehavior) as any;
  state.system.oledMode = "normal";
  state.transport.playing = true;
  state.partScanPulseAccumulator = [1, 2, 3, 4, 5, 6, 7, 8];
  state.partAlgorithmPulseAccumulator = [2, 3, 4, 5, 6, 7, 8, 9];
  state.partScanIndex = [0, 1, 2, 3, 4, 5, 6, 7];
  state.scanPulseAccumulator = 99;
  state.algorithmPulseAccumulator = 88;
  state.scanIndex = 6;

  state.system.fnHeld = true;
  state = routeInput(state, { type: "grid_press", x: 0, y: 1 } as DeviceInput, lifeBehavior).state as any;

  assert.deepEqual(state.partScanPulseAccumulator, [1, 2, 3, 4, 5, 6, 7, 8]);
  assert.deepEqual(state.partAlgorithmPulseAccumulator, [2, 3, 4, 5, 6, 7, 8, 9]);
  assert.deepEqual(state.partScanIndex, [0, 1, 2, 3, 4, 5, 6, 7]);

  state = tick(state, lifeBehavior, 0.01).state as any;
  const expectedPulseDelta = 0.48;
  assert.equal(state.partScanPulseAccumulator[0], 1 + expectedPulseDelta);
  assert.equal(state.partScanPulseAccumulator[1], 2 + expectedPulseDelta);
  assert.equal(state.partAlgorithmPulseAccumulator[0], 2 + expectedPulseDelta);
  assert.equal(state.partAlgorithmPulseAccumulator[1], 3 + expectedPulseDelta);
});

test("L2 Sense lists parts", () => {
  let state = makeState();
  state = selectLabel(state, "L2: Sense");
  state = press(state).state;
  const frame = toSimulatorFrame(state, mockBehavior);
  assert.ok(frame.display.lines.some((line) => line.includes("P1:")));
});

test("L1 Life always exposes part Auto Name within part config", () => {
  for (const behaviorId of ["life", "none", "sequencer", "keys"]) {
    let state = makeState() as any;
    state.runtimeConfig.parts[0].l1.behaviorId = behaviorId;
    state.runtimeConfig.activeBehavior = behaviorId;
    state = selectLabel(state, "L1: Life");
    state = press(state).state;
    state = selectLabel(state, "P1: mock");
    state = press(state).state;
    state = selectLabel(state, "Auto Name");
    assert.ok(true, `${behaviorId} shows Auto Name`);
  }
});

test("instrument list shows compact name labels", () => {
  let state = makeState();
  state = selectLabel(state, "L3: Voice");
  state = press(state).state;
  state = selectLabel(state, "Instruments");
  state = press(state).state;
  const frame = toSimulatorFrame(state, mockBehavior);
  assert.ok(frame.display.lines.some((line) => line.includes("I1: synth")));
});

test("MIDI instruments do not expose the audio Mixer group", () => {
  let state = makeState() as any;
  state.runtimeConfig.instruments[0].type = "midi";
  state = selectLabel(state, "L3: Voice");
  state = press(state).state;
  state = selectLabel(state, "Instruments");
  state = press(state).state;
  state = press(state).state;
  const seen = new Set<string>();
  for (let i = 0; i < 20; i += 1) {
    const frame = toSimulatorFrame(state, mockBehavior);
    const selected = frame.display.lines.find((line) => line.startsWith("@@")) ?? "";
    seen.add(selected);
    state = turn(state, 1).state;
  }
  assert.ok(![...seen].some((line) => line.includes("Mixer")));
});

test("instrument auto name follows type, manual name sets autoName false", () => {
   let state = makeState() as any;
   state.runtimeConfig.instruments[0].type = "sampler";
   state.runtimeConfig.instruments[0].autoName = true;
   state.runtimeConfig.instruments[0].name = "sampler";
  state = selectLabel(state, "L3: Voice");
  state = press(state).state;
  state = selectLabel(state, "Instruments");
  state = press(state).state;
  let frame = toSimulatorFrame(state, mockBehavior);
  assert.ok(frame.display.lines.some((line) => line.includes("I1: sample")));

  state.runtimeConfig.instruments[0].autoName = false;
  state.runtimeConfig.instruments[0].name = "Drums";
  frame = toSimulatorFrame(state, mockBehavior);
  assert.ok(frame.display.lines.some((line) => line.includes("I1: Drums")));

  state.runtimeConfig.instruments[0].name = "MyKick";
  frame = toSimulatorFrame(state, mockBehavior);
  assert.ok(frame.display.lines.some((line) => line.includes("I1: MyKick")));
});

test("part auto name follows behavior and manual name disables auto", () => {
  let state = createInitialState(lifeBehavior) as any;
  state.runtimeConfig.parts[1].autoName = true;
  state.runtimeConfig.parts[1].name = "life";

  state = writeAnyValue(state, "parts.1.l1.behaviorId", "none") as any;
  assert.equal(state.runtimeConfig.parts[1].name, "none");
  assert.equal(state.runtimeConfig.parts[1].autoName, true);

  state = writeAnyValue(state, "parts.1.name", "Manual") as any;
  assert.equal(state.runtimeConfig.parts[1].name, "Manual");
  assert.equal(state.runtimeConfig.parts[1].autoName, false);

  state = writeAnyValue(state, "parts.1.autoName", true) as any;
  assert.equal(state.runtimeConfig.parts[1].name, "none");
  assert.equal(state.runtimeConfig.parts[1].autoName, true);
});

test("extract/apply payload preserves part state when save grid state is on", () => {
  let state = createInitialState(lifeBehavior) as any;
  state.runtimeConfig.parts[0].l1.saveGridState = true;
  state.partStates[0] = { ...state.partStates[0], tick: 33 };
  state.behaviorState = state.partStates[0];

  const payload = extractConfigPayload(state);
  const restored = applyConfigPayload(state, payload, lifeBehavior) as any;
  assert.equal(restored.partStates[0].tick, 33);
});

test("extract/apply payload does not preserve part state when save grid state is off", () => {
  let state = createInitialState(lifeBehavior) as any;
  state.runtimeConfig.parts[0].l1.saveGridState = false;
  state.partStates[0] = { ...state.partStates[0], tick: 44 };
  state.behaviorState = state.partStates[0];

  const payload = extractConfigPayload(state);
  assert.equal(payload.runtimeConfig.parts[0]?.l1?.savedState, undefined);
  const restored = applyConfigPayload(state, payload, lifeBehavior) as any;
  assert.ok(restored.partStates[0] != null);
  assert.notEqual(restored.partStates[0].tick, 44);
});

test("stop toggle does not clear active part grid state", () => {
  let state = makeState() as any;
  state.transport.playing = true;
  state.behaviorState = { ...state.behaviorState, tickCount: 12 };
  state.partStates[0] = state.behaviorState;
  state = routeInput(state, { type: "button_s", pressed: true } as DeviceInput, mockBehavior).state;
  assert.equal(state.transport.playing, false);
  assert.equal(state.partStates[0].tickCount, 12);
});

test("midi_stop does not clear active part grid state", () => {
  let state = makeState() as any;
  state.runtimeConfig.midi.syncMode = "external";
  state.runtimeConfig.midi.clockInEnabled = true;
  state.runtimeConfig.midi.respondToStartStop = true;
  state.transport.playing = true;
  state.behaviorState = { ...state.behaviorState, tickCount: 21 };
  state.partStates[0] = state.behaviorState;
  state = routeInput(state, { type: "midi_stop" } as DeviceInput, mockBehavior).state;
  assert.equal(state.transport.playing, false);
  assert.equal(state.partStates[0].tickCount, 21);
});

test("emergency brake preserves grids and resets timing accumulators", () => {
  let state = makeState() as any;
  state.transport.playing = true;
  state.transport.ppqnPulse = 65;
  state.scanIndex = 4;
  state.scanPulseAccumulator = 2.5;
  state.algorithmPulseAccumulator = 3.5;
  state.ppqnPulseRemainder = 0.7;
  state.partScanIndex = [1, 2, 3, 4, 5, 6, 7, 0];
  state.partScanPulseAccumulator = [1, 1, 1, 1, 1, 1, 1, 1];
  state.partAlgorithmPulseAccumulator = [2, 2, 2, 2, 2, 2, 2, 2];
  state.partStates[0] = { ...state.partStates[0], tickCount: 99 };

  const result = emergencyBrake(state as any);
  const next = result.state as any;
  assert.equal(next.transport.playing, false);
  assert.equal(next.transport.ppqnPulse, 0);
  assert.equal(next.scanPulseAccumulator, 0);
  assert.equal(next.algorithmPulseAccumulator, 0);
  assert.equal(next.ppqnPulseRemainder, 0);
  assert.deepEqual(next.partScanIndex, [0, 0, 0, 0, 0, 0, 0, 0]);
  assert.deepEqual(next.partScanPulseAccumulator, [0, 0, 0, 0, 0, 0, 0, 0]);
  assert.deepEqual(next.partAlgorithmPulseAccumulator, [0, 0, 0, 0, 0, 0, 0, 0]);
  assert.equal(next.partStates[0].tickCount, 99);
});

test("ghost cells show inactive part cells only when enabled", () => {
  const state = createInitialState(lifeBehavior) as any;
  const ghostIndex = GRID_DOMAIN.indexOf({ x: 2, y: 3 });
  const screenIndex = GRID_DOMAIN.toDisplayIndex({ x: 2, y: 3 });
  state.partStates[0].cells = Array.from({ length: CELL_COUNT }, () => false);
  state.partStates[1].cells = Array.from({ length: CELL_COUNT }, (_, i) => i === ghostIndex);

  state.runtimeConfig.ghostCells = false;
  const off = toSimulatorFrame(state, lifeBehavior).leds.cells[screenIndex];
  state.runtimeConfig.ghostCells = true;
  const ghost = toSimulatorFrame(state, lifeBehavior).leds.cells[screenIndex];
  state.partStates[0].cells[ghostIndex] = true;
  const active = toSimulatorFrame(state, lifeBehavior).leds.cells[screenIndex];

  assert.ok(ghost.g > off.g);
  assert.ok(active.g > ghost.g);
});

test("sectioned scan wraps and reverses across lane steps", () => {
  let state = createInitialState(lifeBehavior) as any;
  state.transport.playing = true;
  state.runtimeConfig.scanMode = "scanning";
  state.runtimeConfig.scanAxis = "rows";
  state.runtimeConfig.scanUnit = "1/16";
  state.runtimeConfig.scanSections = "2";
  state.runtimeConfig.parts[0].l2.scanMode = "scanning";
  state.runtimeConfig.parts[0].l2.scanAxis = "rows";
  state.runtimeConfig.parts[0].l2.scanUnit = "1/16";
  state.runtimeConfig.parts[0].l2.scanSections = "2";

  state.partScanIndex[0] = 15;
  state.scanIndex = 15;
  state = tick(state, lifeBehavior).state as any;
  assert.equal(state.scanIndex, 0);

  state.runtimeConfig.scanDirection = "reverse";
  state.runtimeConfig.parts[0].l2.scanDirection = "reverse";
  state = tick(state, lifeBehavior).state as any;
  assert.equal(state.scanIndex, 15);
});

// ─── Global Trigger Mute ──────────────────────────────────────────

test("Fn+Play toggles triggerMuted flag", () => {
  let state = makeState();

  assert.equal(state.system.triggerMuted, false);

  state = routeInput(state, { type: "button_fn", pressed: true } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "button_s", pressed: true } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "button_fn", pressed: false } as DeviceInput, mockBehavior).state;
  assert.equal(state.system.triggerMuted, true);

  state = routeInput(state, { type: "button_fn", pressed: true } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "button_s", pressed: true } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "button_fn", pressed: false } as DeviceInput, mockBehavior).state;
  assert.equal(state.system.triggerMuted, false);
});

test("filterTriggerGatedIntents returns empty when triggerMuted is true for active part", () => {
  const state = makeState();
  state.system.triggerMuted = true;
  const intents = [{ x: 0, y: 0, kind: "activate" as const, degree: 60 }];
  const activeIdx = (state.runtimeConfig as any).activePartIndex ?? 0;
  const filtered = filterTriggerGatedIntents(intents, state, activeIdx);
  assert.equal(filtered.length, 0);
});

test("filterTriggerGatedIntents does not gate non-active parts when triggerMuted is true", () => {
  const state = makeState();
  state.system.triggerMuted = true;
  state.runtimeConfig.activePartIndex = 0;
  const intents = [{ x: 0, y: 0, kind: "activate" as const, degree: 60 }];
  const filtered = filterTriggerGatedIntents(intents, state, 1);
  assert.equal(filtered.length, 1);
});

test("filterTriggerGatedIntents respects per-cell trigger gates", () => {
  const state = makeState();
  state.system.triggerMuted = false;
  const width = PLATFORM_CAPS.gridWidth;
  state.runtimeConfig.parts[0].l1.triggerGates = Array.from({ length: width * PLATFORM_CAPS.gridHeight }, (_, i) => i !== 1);

  const intents = [
    { x: 0, y: 0, kind: "activate" as const, degree: 60 },
    { x: 1, y: 0, kind: "activate" as const, degree: 64 },
  ];
  const filtered = filterTriggerGatedIntents(intents, state, 0);
  assert.equal(filtered.length, 1);
  assert.equal(filtered[0].x, 0);
  assert.equal(filtered[0].y, 0);
});

test("global trigger mute suppresses tick events", () => {
  let state = makeState();
  state.transport.playing = true;
  state.runtimeConfig.algorithmStepUnit = "1/16";

  const r1 = tick(state, mockBehavior);
  assert.ok(r1.events.length > 0, "should produce events when not muted");

  r1.state.system.triggerMuted = true;
  const r2 = tick(r1.state, mockBehavior);
  assert.equal(r2.events.length, 0, "should produce no events when muted");
});
