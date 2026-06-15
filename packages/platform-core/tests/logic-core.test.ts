import test from "node:test";
import assert from "node:assert/strict";

import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import type { DeviceInput } from "@cellsymphony/device-contracts";
import { interpretGrid, type GridSnapshot } from "@cellsymphony/interpretation-core";
import { loadDefaultMappingConfig, mapIntentsToMusicalEvents } from "@cellsymphony/mapping-core";
import { createInitialState, OLED_TEXT_COLUMNS, PLATFORM_CAPS, routeInput, tick, toOledLines, toRuntimeSnapshot } from "../src/index";

type MockState = {
  cells: boolean[];
  tickCount: number;
};

const CELL_COUNT = PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight;

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

const staticBehavior: BehaviorEngine<MockState, unknown> = {
  id: "static",
  init: () => ({
    cells: Array.from({ length: CELL_COUNT }, (_, i) => i === 0 || i === PLATFORM_CAPS.gridWidth - 1),
    tickCount: 0
  }),
  onInput: (state) => state,
  onTick: (state) => ({ ...state, tickCount: state.tickCount + 1 }),
  renderModel: (state) => ({
    name: "Static",
    statusLine: "ok",
    cells: state.cells
  }),
  serialize: (state) => state,
  deserialize: (data) => data as MockState
};

test("interpretation supports event and state trigger paths", () => {
  const previous: GridSnapshot = { width: 2, height: 2, cells: [false, false, false, true] };
  const next: GridSnapshot = { width: 2, height: 2, cells: [true, false, false, false] };

  const intents = interpretGrid(previous, next, 0, {
    id: "test",
    event: { enabled: true },
    state: { enabled: true, tick: { mode: "scan_column_active" } },
    x: { mode: "scale_step", step: 1 },
    y: { mode: "scale_step", step: 3 }
  });

  assert.deepEqual(intents.map((i) => i.kind).sort(), ["activate", "deactivate", "scanned", "scanned_empty"]);
});

test("mapping routes trigger kinds to configured targets", () => {
  const mapping = loadDefaultMappingConfig();
  const { events } = mapIntentsToMusicalEvents(
    [
      { x: 0, y: 0, degree: 0, kind: "activate" },
      { x: 1, y: 0, degree: 1, kind: "deactivate" },
      { x: 2, y: 0, degree: 2, kind: "scanned" }
    ],
    mapping
  );

  assert.equal(events.length, 3);
  assert.equal(events[0].type, "note_on");
  assert.equal(events[1].type, "note_off");
  assert.equal(events[2].type, "note_on");
  if (events[0].type === "note_on" && events[1].type === "note_on" && events[2].type === "note_on") {
    assert.equal(events[0].channel, mapping.activate.channel);
    assert.equal(events[1].channel, mapping.deactivate.channel);
    assert.equal(events[2].channel, mapping.scanned.channel);
  }
});

test("menu navigation edits runtime config through hardware-parity inputs", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";

  const turn = (delta: number) => {
    state = routeInput(state, { type: "encoder_turn", delta }, mockBehavior).state;
  };

  const press = () => {
    state = routeInput(state, { type: "encoder_press" }, mockBehavior).state;
  };

  const selectLabel = (label: string) => {
    for (let i = 0; i < 80; i += 1) {
      const frame = toRuntimeSnapshot(state, mockBehavior);
      const selected = frame.display.lines.find((l) => l.startsWith("@@")) ?? "";
      if (selected.includes(label)) return;
      turn(1);
    }
    assert.fail(`failed to select label: ${label}`);
  };

  // System -> Audio -> Master Vol
  selectLabel("System");
  press();
  selectLabel("Sound");
  press();
  press();
  turn(-1);
  press();

  assert.equal(state.runtimeConfig.masterVolume, 72);
  const frame = toRuntimeSnapshot(state, mockBehavior);
  assert.equal(frame.display.editing, false);
});

test("bool menu items edit like 2-option enums", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";

  const turn = (delta: number) => {
    state = routeInput(state, { type: "encoder_turn", delta }, mockBehavior).state;
  };

  const press = () => {
    state = routeInput(state, { type: "encoder_press" }, mockBehavior).state;
  };

  const selectLabel = (label: string) => {
    for (let i = 0; i < 80; i += 1) {
      const frame = toRuntimeSnapshot(state, mockBehavior);
      const selected = frame.display.lines.find((l) => l.startsWith("@@")) ?? "";
      if (selected.includes(label)) return;
      turn(1);
    }
    assert.fail(`failed to select label: ${label}`);
  };

  selectLabel("System");
  press();
  selectLabel("MIDI");
  press();
  selectLabel("Enabled");

  assert.equal(state.runtimeConfig.midi.enabled, false);
  press();
  assert.equal(state.menu.editing, true);
  assert.equal(state.runtimeConfig.midi.enabled, false);

  turn(1);
  assert.equal(state.runtimeConfig.midi.enabled, true);

  turn(-1);
  assert.equal(state.runtimeConfig.midi.enabled, false);

  press();
  assert.equal(state.menu.editing, false);
});

test("scan mode advances cursor using PPQN timing", () => {
  let state = createInitialState(mockBehavior);
  state.transport.playing = true;
  state.runtimeConfig.scanMode = "scanning";
  state.runtimeConfig.scanAxis = "columns";
  state.runtimeConfig.scanDirection = "forward";
  state.runtimeConfig.scanUnit = "1/16";

  state = tick(state, mockBehavior).state;

  assert.equal(state.scanIndex, 1);
});

test("scanning mode emits notes only when scan index advances", () => {
  let state = createInitialState(mockBehavior);
  state.transport.playing = true;
  state.runtimeConfig.scanMode = "scanning";
  state.runtimeConfig.scanAxis = "columns";
  state.runtimeConfig.scanDirection = "forward";
  state.runtimeConfig.scanUnit = "1/1";
  state.runtimeConfig.eventEnabled = false;

  const first = tick(state, mockBehavior);
  assert.equal(first.state.scanIndex, 0);
  assert.equal(first.events.some((e) => e.type === "note_on"), false);

  const second = tick(first.state, mockBehavior);
  assert.equal(second.state.scanIndex, 0);
  assert.equal(second.events.some((e) => e.type === "note_on"), false);
});

test("startup play scans the first forward column on the first step", () => {
  let state = createInitialState(staticBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.scanMode = "scanning";
  state.runtimeConfig.scanAxis = "columns";
  state.runtimeConfig.scanDirection = "forward";
  state.runtimeConfig.scanUnit = "1/16";
  state.runtimeConfig.eventEnabled = false;
  state.mappingConfig.scanned.action = "note_on";
  state.mappingConfig.scanned_empty.action = "none";

  state = routeInput(state, { type: "button_s", pressed: true } as DeviceInput, staticBehavior).state;
  assert.equal(state.scanIndex, 0);
  assert.equal(state.partScanIndex[0], 0);
  const first = tick(state, staticBehavior, 0.2);

  assert.equal(first.state.scanIndex, 1);
  assert.equal(first.events.some((e) => e.type === "note_on"), true);
});

test("stop-to-play rescans the first forward column on restart", () => {
  let state = createInitialState(staticBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.scanMode = "scanning";
  state.runtimeConfig.scanAxis = "columns";
  state.runtimeConfig.scanDirection = "forward";
  state.runtimeConfig.scanUnit = "1/16";
  state.runtimeConfig.eventEnabled = false;
  state.mappingConfig.scanned.action = "note_on";
  state.mappingConfig.scanned_empty.action = "none";

  state = routeInput(state, { type: "button_s", pressed: true } as DeviceInput, staticBehavior).state;
  state = tick(state, staticBehavior, 0.2).state;
  state = routeInput(state, { type: "button_shift", pressed: true } as DeviceInput, staticBehavior).state;
  state = routeInput(state, { type: "button_s", pressed: true } as DeviceInput, staticBehavior).state;
  state = routeInput(state, { type: "button_shift", pressed: false } as DeviceInput, staticBehavior).state;
  state = routeInput(state, { type: "button_s", pressed: true } as DeviceInput, staticBehavior).state;
  assert.equal(state.scanIndex, 0);
  assert.equal(state.partScanIndex[0], 0);
  const restarted = tick(state, staticBehavior, 0.2);

  assert.equal(restarted.state.scanIndex, 1);
  assert.equal(restarted.events.some((e) => e.type === "note_on"), true);
});

test("startup play scans the first reverse column on the first step", () => {
  let state = createInitialState(staticBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.scanMode = "scanning";
  state.runtimeConfig.scanAxis = "columns";
  state.runtimeConfig.scanDirection = "reverse";
  state.runtimeConfig.scanUnit = "1/16";
  state.runtimeConfig.eventEnabled = false;
  state.mappingConfig.scanned.action = "note_on";
  state.mappingConfig.scanned_empty.action = "none";

  state = routeInput(state, { type: "button_s", pressed: true } as DeviceInput, staticBehavior).state;
  assert.equal(state.scanIndex, PLATFORM_CAPS.gridWidth - 1);
  assert.equal(state.partScanIndex[0], PLATFORM_CAPS.gridWidth - 1);
  const first = tick(state, staticBehavior, 0.2);

  assert.equal(first.state.scanIndex, PLATFORM_CAPS.gridWidth - 2);
  assert.equal(first.events.some((e) => e.type === "note_on"), true);
});

test("grid brightness scales rendered LED intensity", () => {
  let state = createInitialState(mockBehavior);
  state.runtimeConfig.gridBrightness = 20;
  const dim = toRuntimeSnapshot(state, mockBehavior);
  state.runtimeConfig.gridBrightness = 100;
  const bright = toRuntimeSnapshot(state, mockBehavior);
  const dimTotal = dim.leds.cells.reduce((sum, c) => sum + c.r + c.g + c.b, 0);
  const brightTotal = bright.leds.cells.reduce((sum, c) => sum + c.r + c.g + c.b, 0);
  assert.ok(brightTotal > dimTotal);
});

test("velocity modulation mode changes output velocity", () => {
  let state = createInitialState(mockBehavior);
  state.transport.playing = true;
  state.runtimeConfig.algorithmStepUnit = "1/16";
  state.mappingConfig.deactivate.action = "note_on";
  state.runtimeConfig.x.velocity.enabled = true;
  state.runtimeConfig.x.velocity.from = 20;
  state.runtimeConfig.x.velocity.to = 100;
  const result = tick(state, mockBehavior);
  const note = result.events.find((e) => e.type === "note_on");
  assert.ok(note && note.type === "note_on");
  if (note && note.type === "note_on") {
    assert.ok(note.velocity >= 20 && note.velocity <= 100);
  }
});

test("filter modulation mode emits cutoff/resonance CC", () => {
  let state = createInitialState(mockBehavior);
  state.transport.playing = true;
  state.runtimeConfig.algorithmStepUnit = "1/16";
  state.mappingConfig.deactivate.action = "note_on";
  state.runtimeConfig.x.filterCutoff.enabled = true;
  state.runtimeConfig.y.filterResonance.enabled = true;
  const result = tick(state, mockBehavior);
  const hasCutoff = result.events.some((e) => e.type === "cc" && e.controller === 74);
  const hasResonance = result.events.some((e) => e.type === "cc" && e.controller === 71);
  assert.equal(hasCutoff, true);
  assert.equal(hasResonance, true);

  const firstNote = result.events.findIndex((e) => e.type === "note_on");
  const firstCutoff = result.events.findIndex((e) => e.type === "cc" && e.controller === 74);
  assert.ok(firstCutoff !== -1 && firstNote !== -1 && firstCutoff < firstNote);

  const note = result.events.find((e) => e.type === "note_on");
  const cc = result.events.find((e) => e.type === "cc" && e.controller === 74);
  if (note && note.type === "note_on" && cc && cc.type === "cc") {
    assert.equal(cc.channel, note.channel);
  }
});

test("aux encoder inputs are reserved and do not navigate menu", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state = routeInput(state, { type: "encoder_turn", delta: 1, id: "aux1" }, mockBehavior).state;
  assert.equal(state.menu.cursor, 0);

  state = routeInput(state, { type: "encoder_press", id: "aux2" }, mockBehavior).state;
  assert.deepEqual(state.menu.stack, []);
});

