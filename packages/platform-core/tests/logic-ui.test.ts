import test from "node:test";
import assert from "node:assert/strict";

import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import type { DeviceInput } from "@cellsymphony/device-contracts";
import { keysBehavior } from "@cellsymphony/behaviors-keys";
import { createInitialState, GRID_DOMAIN, OLED_TEXT_COLUMNS, PAN_CENTER_POS, PAN_POSITION_COUNT, PAN_POSITION_MAX, PLATFORM_CAPS, routeInput, tick, toOledLines, toSimulatorFrame } from "../src/index";
import { fitOledText, formatDisplayValue } from "../src/coreUtils";
import { shouldUseNumberBar } from "../src/menuPresentation";
import { currentMenuView } from "../src/menuView";
import { axisGroup } from "../src/menuNodes";
import { buildMenuTree } from "../src/menuTree";
import { readAnyValue } from "../src/paramAccess";
import { renderOledFrame } from "../src/oledRender";
import { pitchFromIntent } from "../src/musicTransforms";
import { cellsToLeds, resolveDancePanTarget, DANCE_PAN_COLORS } from "../src/runtimeHelpers";

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

test("OLED formatter clamps display lines and width", () => {
  const result = toOledLines({
    page: "Transport",
    title: "Cell Symphony Super Long Header",
    editing: false,
    lines: ["line one", "line two", "line three", "line four"]
  });

  assert.equal(result.lines.length, 5);
  assert.equal(result.lines[0].length, OLED_TEXT_COLUMNS);
  assert.equal(result.lines[result.lines.length - 1], "line four");
});

test("OLED formatter does not truncate selected marker lines that fit visually", () => {
  const result = toOledLines({
    page: "System/MIDI",
    title: "System/MIDI",
    editing: false,
    lines: ["@@> Respond Start/Stop", "@@> !Spawn Random"]
  });

  assert.equal(result.lines[1], "@@> Respond Start/Stop");
  assert.equal(result.lines[2], "@@> !Spawn Random");
});

test("OLED formatter still truncates truly long selected lines", () => {
  const result = toOledLines({
    page: "System/MIDI",
    title: "System/MIDI",
    editing: false,
    lines: ["@@> This selection label is definitely too long"]
  });

  assert.equal(result.lines[1].startsWith("@@"), true);
  assert.equal(result.lines[1].slice(2).length, OLED_TEXT_COLUMNS);
  assert.equal(result.lines[1].slice(2).endsWith("..."), true);
});

test("edit marker uses compact star prefix", () => {
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
      const frame = toSimulatorFrame(state, mockBehavior);
      const selected = frame.display.lines.find((l) => l.startsWith("@@")) ?? "";
      if (selected.includes(label)) return;
      turn(1);
    }
    assert.fail(`failed to select label: ${label}`);
  };

  selectLabel("System");
  press();
  selectLabel("Sound");
  press();
  // Master Vol
  press();
  const frame = toSimulatorFrame(state, mockBehavior);
  const hasStarEdit = frame.display.lines.some((line) => /\*\s*Vol:/.test(line));
  assert.equal(hasStarEdit, true);
});

test("pan menu display shows direct distance from center", () => {
  const key = "instruments.0.mixer.panPos";

  assert.equal(formatDisplayValue(key, 0), "L16");
  assert.equal(formatDisplayValue(key, 1), "L15");
  assert.equal(formatDisplayValue(key, 15), "L1");
  assert.equal(formatDisplayValue(key, PAN_CENTER_POS), "C");
  assert.equal(formatDisplayValue(key, 17), "R1");
  assert.equal(formatDisplayValue(key, 31), "R15");
  assert.equal(formatDisplayValue(key, PAN_POSITION_MAX), "R16");
});

test("marker display style still opts into number bar rendering", () => {
  assert.equal(shouldUseNumberBar({ kind: "number", label: "Pan Pos", key: "instruments.0.mixer.panPos", min: 0, max: PAN_POSITION_MAX, step: 1, displayStyle: "marker" }), true);
});

test("current menu view propagates marker and fill bar styles", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";

  const turn = (delta: number) => {
    state = routeInput(state, { type: "encoder_turn", delta }, mockBehavior).state;
  };

  const press = () => {
    state = routeInput(state, { type: "encoder_press" }, mockBehavior).state;
  };

  const selectLabel = (label: string) => {
    for (let i = 0; i < 120; i += 1) {
      const frame = toSimulatorFrame(state, mockBehavior);
      const selected = frame.display.lines.find((l) => l.startsWith("@@")) ?? "";
      if (selected.includes(label)) return;
      turn(1);
    }
    assert.fail(`failed to select label: ${label}`);
  };

  selectLabel("L3: Voice");
  press();
  selectLabel("Instruments");
  press();
  selectLabel("I1:");
  press();
  selectLabel("Mixer");
  press();
  selectLabel("Pan Pos");

  const view = currentMenuView({
    state,
    menuTree: (nextState) => buildMenuTree(nextState, {
      resolveBehavior: () => mockBehavior,
      axisGroup,
      presetListNodes: () => [],
      presetRenameNodes: () => [],
      midiOutputNodes: () => [],
      midiInputNodes: () => [],
      sampleBrowserNodes: () => []
    }),
    resolveBehavior: () => mockBehavior,
    fitOledText: (text: string) => fitOledText(text, OLED_TEXT_COLUMNS),
    readAnyValue,
    formatDisplayValue,
    oledTextLines: 8
  });

  assert.ok(view.barValues.some((bar) => bar?.style === "marker"), "Pan Pos should use marker style");
});

test("X/Y Axis menu item shows mapped parameter path", () => {
  const state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.danceMode = "xy";
  (state.runtimeConfig as any).parts[0].xy.x = {
    key: "instruments.0.synth.filter.cutoffHz",
    label: "Cutoff",
    kind: "number",
    min: 20,
    max: 20000,
    step: 1
  };
  state.menu = { stack: [3], cursor: 2, editing: false };

  const view = currentMenuView({
    state,
    menuTree: (nextState) => buildMenuTree(nextState, {
      resolveBehavior: () => mockBehavior,
      axisGroup,
      presetListNodes: () => [],
      presetRenameNodes: () => [],
      midiOutputNodes: () => [],
      midiInputNodes: () => [],
      sampleBrowserNodes: () => []
    }),
    resolveBehavior: () => mockBehavior,
    fitOledText: (text: string) => fitOledText(text, OLED_TEXT_COLUMNS),
    readAnyValue,
    formatDisplayValue,
    oledTextLines: 8
  });

  assert.ok(view.lines.some((line) => line.includes("> X Axis")));
  assert.ok(view.lines.some((line) => line.includes("L3>I1>Synth>Filter")));
});

test("X/Y Axis opens directly to none and parameter tree", () => {
  const state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.danceMode = "xy";
  state.menu = { stack: [3, 2], cursor: 0, editing: false };

  const view = currentMenuView({
    state,
    menuTree: (nextState) => buildMenuTree(nextState, {
      resolveBehavior: () => mockBehavior,
      axisGroup,
      presetListNodes: () => [],
      presetRenameNodes: () => [],
      midiOutputNodes: () => [],
      midiInputNodes: () => [],
      sampleBrowserNodes: () => []
    }),
    resolveBehavior: () => mockBehavior,
    fitOledText: (text: string) => fitOledText(text, OLED_TEXT_COLUMNS),
    readAnyValue,
    formatDisplayValue,
    oledTextLines: 8
  });

  assert.equal(view.path, "X/Y:X Axis");
  assert.ok(view.lines.some((line) => line.includes("!(none)")));
  assert.ok(view.lines.some((line) => line.includes("L1: Life")));
});

test("X/Y Axis parameter browser keeps Dance color", () => {
  const state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.danceMode = "xy";
  state.menu = { stack: [3, 2, 1], cursor: 0, editing: false };

  const view = currentMenuView({
    state,
    menuTree: (nextState) => buildMenuTree(nextState, {
      resolveBehavior: () => mockBehavior,
      axisGroup,
      presetListNodes: () => [],
      presetRenameNodes: () => [],
      midiOutputNodes: () => [],
      midiInputNodes: () => [],
      sampleBrowserNodes: () => []
    }),
    resolveBehavior: () => mockBehavior,
    fitOledText: (text: string) => fitOledText(text, OLED_TEXT_COLUMNS),
    readAnyValue,
    formatDisplayValue,
    oledTextLines: 8
  });

  assert.equal(view.path, "X/Y:X Axis/L1: Life");
  assert.ok(view.colors.length > 0);
  assert.ok(view.colors.every((color) => color === 0xffff));
});

test("Sense mappings slot picker opens to none and parameter tree", () => {
  const state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.menu = { stack: [1, 0, 3, 0, 0], cursor: 0, editing: false };

  const view = currentMenuView({
    state,
    menuTree: (nextState) => buildMenuTree(nextState, {
      resolveBehavior: () => mockBehavior,
      axisGroup,
      presetListNodes: () => [],
      presetRenameNodes: () => [],
      midiOutputNodes: () => [],
      midiInputNodes: () => [],
      sampleBrowserNodes: () => []
    }),
    resolveBehavior: () => mockBehavior,
    fitOledText: (text: string) => fitOledText(text, OLED_TEXT_COLUMNS),
    readAnyValue,
    formatDisplayValue,
    oledTextLines: 8
  });

  assert.ok(view.lines.some((line) => line.includes("!(none)")));
  assert.ok(view.lines.some((line) => line.includes("L3: Voice")));
});

test("Sense Aux Turn menu item shows mapped parameter path", () => {
  const state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.auxBindings.aux1 = {
    turn: {
      key: "instruments.0.synth.filter.cutoffHz",
      label: "Cutoff",
      kind: "number",
      min: 20,
      max: 20000,
      step: 1
    },
    press: null
  };
  state.menu = { stack: [1, 0, 0], cursor: 0, editing: false };

  const view = currentMenuView({
    state,
    menuTree: (nextState) => buildMenuTree(nextState, {
      resolveBehavior: () => mockBehavior,
      axisGroup,
      presetListNodes: () => [],
      presetRenameNodes: () => [],
      midiOutputNodes: () => [],
      midiInputNodes: () => [],
      sampleBrowserNodes: () => []
    }),
    resolveBehavior: () => mockBehavior,
    fitOledText: (text: string) => fitOledText(text, OLED_TEXT_COLUMNS),
    readAnyValue,
    formatDisplayValue,
    oledTextLines: 8
  });

  assert.ok(view.lines.some((line) => line.includes("> Turn")));
  assert.ok(view.lines.some((line) => line.includes("L3>I1>Synth>Filter")));
});

test("Sense Aux Click menu opens to none and click action groups", () => {
  const state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.menu = { stack: [1, 0, 0, 1], cursor: 0, editing: false };

  const view = currentMenuView({
    state,
    menuTree: (nextState) => buildMenuTree(nextState, {
      resolveBehavior: () => mockBehavior,
      axisGroup,
      presetListNodes: () => [],
      presetRenameNodes: () => [],
      midiOutputNodes: () => [],
      midiInputNodes: () => [],
      sampleBrowserNodes: () => []
    }),
    resolveBehavior: () => mockBehavior,
    fitOledText: (text: string) => fitOledText(text, OLED_TEXT_COLUMNS),
    readAnyValue,
    formatDisplayValue,
    oledTextLines: 8
  });

  assert.ok(view.lines.some((line) => line.includes("!(none)")));
});

test("OLED renders audio load indicator colors", () => {
  const state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";

  const yellow = toSimulatorFrame(state, mockBehavior, { audioLoad: { ratio: 0.7, voiceSteal: false } }).oled!;
  const red = toSimulatorFrame(state, mockBehavior, { audioLoad: { ratio: 0.9, voiceSteal: false } }).oled!;
  const idle = toSimulatorFrame(state, mockBehavior, { audioLoad: { ratio: 0.1, voiceSteal: false } }).oled!;

  assert.equal(pixel565(yellow.pixels, 123, 5), 0xffe0);
  assert.equal(pixel565(red.pixels, 123, 5), 0xf800);
  assert.notEqual(pixel565(idle.pixels, 123, 5), 0xffe0);
  assert.notEqual(pixel565(idle.pixels, 123, 5), 0xf800);
});

test("OLED marker bars render a position marker instead of a filled region", () => {
  const marker = renderOledFrame({
    lines: ["@@  Pan: C"],
    barValues: [{ frac: 0.5, numChars: 0, style: "marker" }]
  });
  const fill = renderOledFrame({
    lines: ["@@  Pan: C"],
    barValues: [{ frac: 0.5, numChars: 0 }]
  });

  assert.equal(pixel565(marker.pixels, 30, 5), 0xffff);
  assert.equal(pixel565(fill.pixels, 30, 5), 0x39c7);
  assert.equal(pixel565(marker.pixels, 57, 5), 0x39c7);
});

test("simulator frame exposes behavior grid interaction semantics", () => {
  const paintState = createInitialState(mockBehavior);
  const keysState = createInitialState(keysBehavior);

  assert.equal(toSimulatorFrame(paintState, mockBehavior).gridInteraction, "paint");
  assert.equal(toSimulatorFrame(keysState, keysBehavior).gridInteraction, "momentary");
});

test("Fn+rightmost grid column selects Dance pages", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";

  state = routeInput(state, { type: "button_fn", pressed: true }, mockBehavior).state;
  state = routeInput(state, { type: "grid_press", x: PLATFORM_CAPS.gridWidth - 1, y: 0 }, mockBehavior).state;

 assert.equal(state.system.danceMode, "mix");
  assert.deepEqual(state.menu.stack, [3]);
  assert.equal(toSimulatorFrame(state, mockBehavior).display.page, "L4: Dance");

  state = routeInput(state, { type: "grid_press", x: PLATFORM_CAPS.gridWidth - 1, y: 1 }, mockBehavior).state;
  assert.equal(state.system.danceMode, "pan");

  state = routeInput(state, { type: "grid_press", x: PLATFORM_CAPS.gridWidth - 1, y: 3 }, mockBehavior).state;
  assert.equal(state.system.danceMode, "trigger-gate");

  state = routeInput(state, { type: "grid_press", x: PLATFORM_CAPS.gridWidth - 1, y: PLATFORM_CAPS.gridHeight - 1 }, mockBehavior).state;
  assert.equal(state.system.danceMode, "trigger-gate");
});

test("entering L1: Life selects the active part", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  (state.runtimeConfig as any).activePartIndex = 2;

  state = routeInput(state, { type: "encoder_press" }, mockBehavior).state;

  assert.deepEqual(state.menu.stack, [0]);
  assert.equal(state.menu.cursor, 2);
  assert.equal(toSimulatorFrame(state, mockBehavior).display.page, "L1: Life");
});

test("entering L2: Sense selects the active part", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  (state.runtimeConfig as any).activePartIndex = 2;

  state = routeInput(state, { type: "encoder_turn", delta: 1 }, mockBehavior).state;
  state = routeInput(state, { type: "encoder_press" }, mockBehavior).state;

  assert.deepEqual(state.menu.stack, [1]);
  assert.equal(state.menu.cursor, 3);
  assert.equal(toSimulatorFrame(state, mockBehavior).display.page, "L2: Sense");
});

test("entering L1 or L2 clears the active Dance overlay", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.danceMode = "pan";
  state.system.danceMode = "pan";

  state = routeInput(state, { type: "encoder_press" }, mockBehavior).state;
  assert.equal(state.system.danceMode, "none");
  assert.equal((state.runtimeConfig as any).danceMode, "pan");

  state.system.danceMode = "pan";
  state.menu = { stack: [], cursor: 1, editing: false };
  state = routeInput(state, { type: "encoder_press" }, mockBehavior).state;
  assert.equal(state.system.danceMode, "none");
  assert.equal((state.runtimeConfig as any).danceMode, "pan");
});

test("Dance Page menu can activate trigger-gate", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";

  state = routeInput(state, { type: "encoder_turn", delta: 1 }, mockBehavior).state;
  state = routeInput(state, { type: "encoder_turn", delta: 1 }, mockBehavior).state;
  state = routeInput(state, { type: "encoder_turn", delta: 1 }, mockBehavior).state;
  state = routeInput(state, { type: "encoder_press" }, mockBehavior).state;
  state = routeInput(state, { type: "encoder_press" }, mockBehavior).state;
  state = routeInput(state, { type: "encoder_turn", delta: 1 }, mockBehavior).state;
  state = routeInput(state, { type: "encoder_turn", delta: 1 }, mockBehavior).state;
  state = routeInput(state, { type: "encoder_turn", delta: 1 }, mockBehavior).state;
  state = routeInput(state, { type: "encoder_turn", delta: 1 }, mockBehavior).state;

  assert.equal(state.system.danceMode, "trigger-gate");

  state = routeInput(state, { type: "grid_press", x: 0, y: 0 }, mockBehavior).state;
  assert.equal((state.runtimeConfig as any).parts[0].l2.triggerProbabilityMode, "zero");
});

test("Dance Page menu activates the corresponding grid page", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.menu = { stack: [3], cursor: 0, editing: true };

  const turnDancePageTo = (mode: "none" | "mix" | "pan" | "fx" | "trigger-gate" | "xy") => {
    const options = ["none", "mix", "pan", "fx", "trigger-gate", "xy"];
    const current = options.indexOf(String((state.runtimeConfig as any).danceMode ?? "none"));
    const target = options.indexOf(mode);
    assert.notEqual(target, -1);
    for (let i = current; i < target; i += 1) {
      state = routeInput(state, { type: "encoder_turn", delta: 1 }, mockBehavior).state;
    }
    for (let i = current; i > target; i -= 1) {
      state = routeInput(state, { type: "encoder_turn", delta: -1 }, mockBehavior).state;
    }
    assert.equal(state.system.danceMode, mode);
    assert.equal((state.runtimeConfig as any).danceMode, mode);
  };

  turnDancePageTo("mix");
  let cells = toSimulatorFrame(state, mockBehavior).leds.cells;
  let mixMarker = cells[GRID_DOMAIN.toDisplayIndex({ x: 0, y: PLATFORM_CAPS.gridHeight - 1 })]!;
  assert.ok(mixMarker.g > 100, `mix page should light top-row volume marker, got ${mixMarker.g}`);

  turnDancePageTo("pan");
  cells = toSimulatorFrame(state, mockBehavior).leds.cells;
  const panMarkerLeft = cells[GRID_DOMAIN.toDisplayIndex({ x: 3, y: 0 })]!;
  const panMarkerRight = cells[GRID_DOMAIN.toDisplayIndex({ x: 4, y: 0 })]!;
  assert.ok(panMarkerLeft.r > 100 && panMarkerLeft.g > 100, "pan page should light the left pan marker");
  assert.ok(panMarkerRight.r > 100 && panMarkerRight.g > 100, "pan page should light the right pan marker");

  turnDancePageTo("fx");
  assert.equal(state.system.danceMode, "fx");

  turnDancePageTo("trigger-gate");
  cells = toSimulatorFrame(state, mockBehavior).leds.cells;
  const triggerGateCell = cells[GRID_DOMAIN.toDisplayIndex({ x: 2, y: 0 })]!;
  assert.ok(triggerGateCell.g > 100, "trigger-gate page should light trigger mode cells");

  turnDancePageTo("xy");
  cells = toSimulatorFrame(state, mockBehavior).leds.cells;
  const xyCell = cells[GRID_DOMAIN.toDisplayIndex({ x: 4, y: 4 })]!;
  assert.ok(xyCell.r > 50 && xyCell.g > 50 && xyCell.b > 50, "xy page should light the XY touch position");

  turnDancePageTo("none");
  assert.equal(state.system.danceMode, "none");
});

test("entering Dance menu loads the selected Dance page into the grid", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.danceMode = "pan";
  state.system.danceMode = "none";

  state = routeInput(state, { type: "encoder_turn", delta: 1 }, mockBehavior).state;
  state = routeInput(state, { type: "encoder_turn", delta: 1 }, mockBehavior).state;
  state = routeInput(state, { type: "encoder_turn", delta: 1 }, mockBehavior).state;
  state = routeInput(state, { type: "encoder_press" }, mockBehavior).state;

  assert.equal(state.system.danceMode, "pan");
  assert.deepEqual(state.menu.stack, [3]);

  const cells = toSimulatorFrame(state, mockBehavior).leds.cells;
  const panMarkerLeft = cells[GRID_DOMAIN.toDisplayIndex({ x: 3, y: 0 })]!;
  const panMarkerRight = cells[GRID_DOMAIN.toDisplayIndex({ x: 4, y: 0 })]!;
  assert.ok(panMarkerLeft.r > 100 && panMarkerLeft.g > 100, "entering Dance should light the left pan marker");
  assert.ok(panMarkerRight.r > 100 && panMarkerRight.g > 100, "entering Dance should light the right pan marker");
});

test("Fn+rightmost column FX page selects fx dance mode", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";

  state = routeInput(state, { type: "button_fn", pressed: true }, mockBehavior).state;
  state = routeInput(state, { type: "grid_press", x: PLATFORM_CAPS.gridWidth - 1, y: 2 }, mockBehavior).state;

  assert.equal(state.system.danceMode, "fx");
  assert.deepEqual(state.menu.stack, [3]);
  assert.equal(toSimulatorFrame(state, mockBehavior).display.page, "L4: Dance");
});

test("Fn overlay dims FX grid cells when danceMode is fx", () => {
  const state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.fnHeld = true;
  state.system.danceMode = "fx";
  state.runtimeConfig.danceMode = "fx";

  const cells = toSimulatorFrame(state, mockBehavior).leds.cells;
  const midCell = cells[GRID_DOMAIN.toDisplayIndex({ x: 2, y: 2 })]!;
  // middle cells dimmed: default FX dark (15,15,22) * 0.75 brightness * 0.25 dim = (3,3,4)
  assert.ok(midCell.r < 20 && midCell.g < 20 && midCell.b < 20);

  // right column FX page indicator should be cyan scaled by 0.75 brightness: g≈158
  const fxPage = cells[GRID_DOMAIN.toDisplayIndex({ x: PLATFORM_CAPS.gridWidth - 1, y: 2 })]!;
  assert.ok(fxPage.g > 100 && fxPage.g < 200, `fx page green should be ~158, got ${fxPage.g}`);
  assert.ok(fxPage.g > 0 || fxPage.b > 0);

  // left column part indicator should still be bright
  const partCell = cells[GRID_DOMAIN.toDisplayIndex({ x: 0, y: 0 })]!;
  assert.ok(partCell.g > 0);
});

test("Dance trigger-gate LEDs show per-part mode and all-parts actions", () => {
  const state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.danceMode = "trigger-gate";
  (state.runtimeConfig as any).parts[0].l2.triggerProbabilityMode = "custom";
  (state.runtimeConfig as any).parts[1].l2.triggerProbabilityMode = "full";

  const cells = toSimulatorFrame(state, mockBehavior).leds.cells;
  const part0Custom = cells[GRID_DOMAIN.toDisplayIndex({ x: 1, y: 0 })]!;
  const part0Zero = cells[GRID_DOMAIN.toDisplayIndex({ x: 0, y: 0 })]!;
  const part1Full = cells[GRID_DOMAIN.toDisplayIndex({ x: 2, y: 1 })]!;
  const allCustom = cells[GRID_DOMAIN.toDisplayIndex({ x: 6, y: 0 })]!;
  assert.ok(part0Custom.r > 100 && part0Custom.g > 80);
  assert.ok(part0Zero.r > 0 && part0Zero.r > part0Zero.g);
  assert.ok(part1Full.g > part1Full.r);
  assert.ok(allCustom.r > 100 && allCustom.g > 80);
});

test("Fn grid overlay shows active parts and Dance page options", () => {
  const state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.fnHeld = true;
  state.system.danceMode = "pan";
  state.runtimeConfig.danceMode = "pan";
  state.runtimeConfig.parts[1]!.l1.behaviorId = "none";
  state.runtimeConfig.parts[2]!.l1.behaviorId = "life";

  const cells = toSimulatorFrame(state, mockBehavior).leds.cells;
  const activePart = cells[GRID_DOMAIN.toDisplayIndex({ x: 0, y: 0 })]!;
  const nonePart = cells[GRID_DOMAIN.toDisplayIndex({ x: 0, y: 1 })]!;
  const configuredPart = cells[GRID_DOMAIN.toDisplayIndex({ x: 0, y: 2 })]!;
  const selectedPage = cells[GRID_DOMAIN.toDisplayIndex({ x: PLATFORM_CAPS.gridWidth - 1, y: 1 })]!;
  const inactivePage = cells[GRID_DOMAIN.toDisplayIndex({ x: PLATFORM_CAPS.gridWidth - 1, y: 0 })]!;

  // in Dance mode, no part is highlighted as selected; all available parts show green
  assert.ok(activePart.g > 0);
  // unused part slot shows dimmed underlying page background, not black
  assert.deepEqual(nonePart, { r: 2, g: 2, b: 3 });
  assert.deepEqual(configuredPart, activePart);
  assert.ok(selectedPage.g > 0 || selectedPage.b > 0);

  // non-navigation middle cells should be dimmed
  const middleCell = cells[GRID_DOMAIN.toDisplayIndex({ x: 3, y: 3 })]!;
  // pan marker at row 3 is ~191 channels before dimming; after 0.25 factor should be < 50
  assert.ok(middleCell.r < 60 && middleCell.g < 60 && middleCell.b < 60);
  // left-column available part indicator should still be bright
  assert.ok(activePart.g > 0);
});

test("Fn grid overlay highlights active part when not in Dance mode", () => {
  const state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.fnHeld = true;
  state.system.danceMode = "none";
  state.runtimeConfig.parts[1]!.l1.behaviorId = "none";
  state.runtimeConfig.parts[2]!.l1.behaviorId = "life";

  const cells = toSimulatorFrame(state, mockBehavior).leds.cells;
  const activePart = cells[GRID_DOMAIN.toDisplayIndex({ x: 0, y: 0 })]!;
  const nonePart = cells[GRID_DOMAIN.toDisplayIndex({ x: 0, y: 1 })]!;
  const configuredPart = cells[GRID_DOMAIN.toDisplayIndex({ x: 0, y: 2 })]!;

  // active part shows blue/cyan
  assert.ok(activePart.g > 0 && activePart.b > 0 && activePart.g === activePart.b && activePart.r < activePart.g);
  // unused part slot shows dimmed underlying behavior cell (mock has live cell at display 0,1)
  assert.deepEqual(nonePart, { r: 0, g: 48, b: 23 });
  // available part shows green, dimmer than active blue
  assert.ok(configuredPart.g > 0 && configuredPart.g < activePart.g);
});

test("Dance grid updates mixer volume and pan", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.danceMode = "mix";

  state = routeInput(state, { type: "grid_press", x: 1, y: 0 }, mockBehavior).state;
  assert.equal(state.runtimeConfig.instruments[1]?.mixer?.volume, 0);

  state = routeInput(state, { type: "grid_press", x: PLATFORM_CAPS.gridWidth - 1, y: 1 }, mockBehavior).state;
  assert.equal(state.system.danceMode, "mix");
  assert.equal(state.runtimeConfig.instruments[PLATFORM_CAPS.gridWidth - 1]?.mixer?.volume, 14);

  state.system.danceMode = "pan";

  state = routeInput(state, { type: "grid_press", x: 2, y: 1 }, mockBehavior).state;
  assert.equal(state.runtimeConfig.instruments[1]?.mixer?.panPos, 11);
});

test("Fn+leftmost part selection exits Dance grid mode", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.danceMode = "fx";
  state = routeInput(state, { type: "button_fn", pressed: true }, mockBehavior).state;
  state = routeInput(state, { type: "grid_press", x: 0, y: 2 }, mockBehavior).state;
  assert.equal(state.runtimeConfig.activePartIndex, 2);
  assert.equal(state.system.danceMode, "none");
});

test("Dance mix LEDs show volume markers", () => {
  const state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.danceMode = "mix";
  state.runtimeConfig.instruments[0]!.mixer = { route: "direct", volume: 100, panPos: PAN_CENTER_POS };
  state.runtimeConfig.instruments[1]!.mixer = { route: "fx_bus_1", volume: 0, panPos: PAN_CENTER_POS };

  const cells = toSimulatorFrame(state, mockBehavior).leds.cells;
  const direct = cells[GRID_DOMAIN.toDisplayIndex({ x: 0, y: PLATFORM_CAPS.gridHeight - 1 })]!;
  const fx = cells[GRID_DOMAIN.toDisplayIndex({ x: 1, y: 0 })]!;

  assert.ok(direct.g > direct.r && direct.g > direct.b);
  assert.ok(fx.g > fx.r && fx.g > fx.b);
});

test("Dance pan LEDs show a two-cell white marker for direct route", () => {
  const state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.danceMode = "pan";
  state.runtimeConfig.instruments[0]!.mixer = { route: "direct", volume: 100, panPos: PAN_CENTER_POS };
  assert.equal(state.runtimeConfig.panPositions, PAN_POSITION_COUNT);

  const cells = toSimulatorFrame(state, mockBehavior).leds.cells;
  const leftCenter = cells[GRID_DOMAIN.toDisplayIndex({ x: 3, y: 0 })]!;
  const rightCenter = cells[GRID_DOMAIN.toDisplayIndex({ x: 4, y: 0 })]!;

  // white {255,255,255} * brightness 0.75 ≈ 191 each channel
  assert.ok(leftCenter.r > 120 && leftCenter.g > 120 && leftCenter.b > 120);
  assert.ok(rightCenter.r > 120 && rightCenter.g > 120 && rightCenter.b > 120);
});

test("Dance pan writes bus pan for bus-routed instrument", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.danceMode = "pan";
  state.runtimeConfig.instruments[0]!.mixer = { route: "fx_bus_1", volume: 100, panPos: PAN_CENTER_POS };
  state.runtimeConfig.mixer = state.runtimeConfig.mixer ?? { buses: Array.from({ length: PLATFORM_CAPS.busCount }, () => ({ slot1: { type: "none", params: {} }, slot2: { type: "none", params: {} }, panPos: PAN_CENTER_POS, autoName: true, name: "(none)" })) };

  // press row 0 (instrument 0) at x=2 maps to the third coarse pan marker.
  state = routeInput(state, { type: "grid_press", x: 2, y: 0 }, mockBehavior).state;

  // bus 0 panPos should update, instrument panPos should also be set for state preservation
  assert.equal(state.runtimeConfig.mixer!.buses[0].panPos, 11);
  assert.equal(state.runtimeConfig.instruments[0]!.mixer?.panPos, 11);
});

test("Dance pan writes instrument pan for direct-routed instrument", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.danceMode = "pan";
  state.runtimeConfig.instruments[0]!.mixer = { route: "direct", volume: 100, panPos: PAN_CENTER_POS };

  state = routeInput(state, { type: "grid_press", x: 6, y: 0 }, mockBehavior).state;

  assert.equal(state.runtimeConfig.instruments[0]!.mixer?.panPos, 27);

  // edge: leftmost press (x=0) should set hard-left pan.
  state.runtimeConfig.instruments[0]!.mixer!.panPos = PAN_CENTER_POS;
  state = routeInput(state, { type: "grid_press", x: 0, y: 0 }, mockBehavior).state;
  assert.equal(state.runtimeConfig.instruments[0]!.mixer?.panPos, 0);

  // edge: rightmost press (x=7) should set hard-right pan.
  state.runtimeConfig.instruments[0]!.mixer!.panPos = PAN_CENTER_POS;
  state = routeInput(state, { type: "grid_press", x: 7, y: 0 }, mockBehavior).state;
  assert.equal(state.runtimeConfig.instruments[0]!.mixer?.panPos, PAN_POSITION_MAX);
});

test("Dance pan grid presses map to seven coarse stereo positions", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.danceMode = "pan";
  state.runtimeConfig.instruments[0]!.mixer = { route: "direct", volume: 100, panPos: PAN_CENTER_POS };

  const expected = [0, 5, 11, PAN_CENTER_POS, PAN_CENTER_POS, 21, 27, PAN_POSITION_MAX];
  for (let x = 0; x < expected.length; x += 1) {
    state.runtimeConfig.instruments[0]!.mixer!.panPos = PAN_CENTER_POS;
    state = routeInput(state, { type: "grid_press", x, y: 0 }, mockBehavior).state;
    assert.equal(state.runtimeConfig.instruments[0]!.mixer?.panPos, expected[x]);
  }
});

test("Dance pan LEDs show bus color for bus-routed instrument and synchronized markers", () => {
  const state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.danceMode = "pan";
  state.runtimeConfig.instruments[0]!.mixer = { route: "fx_bus_1", volume: 100, panPos: PAN_CENTER_POS };
  state.runtimeConfig.instruments[1]!.mixer = { route: "fx_bus_1", volume: 100, panPos: PAN_CENTER_POS };
  state.runtimeConfig.instruments[2]!.mixer = { route: "fx_bus_2", volume: 100, panPos: PAN_CENTER_POS };
  state.runtimeConfig.mixer!.buses[0].panPos = 5;
  state.runtimeConfig.mixer!.buses[1].panPos = 27;

  const t0 = resolveDancePanTarget(state, 0);
  const t1 = resolveDancePanTarget(state, 1);
  const t2 = resolveDancePanTarget(state, 2);

  // both rows on bus 0 target bus pan
  assert.equal(t0.route, "bus");
  assert.equal(t1.route, "bus");
  assert.equal(t0.panPos, 5);
  assert.equal(t1.panPos, 5);
  // same bus index 0
  assert.equal(t0.busIndex, 0);
  assert.equal(t1.busIndex, 0);
  // bus 1 color = purple
   assert.deepEqual(t0.color, DANCE_PAN_COLORS.fx_bus_1);
   assert.deepEqual(t1.color, DANCE_PAN_COLORS.fx_bus_1);

  // row 2 on bus 2 targets bus 1 pan
  assert.equal(t2.route, "bus");
  assert.equal(t2.panPos, 27);
  assert.equal(t2.busIndex, 1);
  // bus 2 color = cyan
   assert.deepEqual(t2.color, DANCE_PAN_COLORS.fx_bus_2);
});

test("Dance FX assignment stores selected effect config on grid cell", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.fxAssignMode = { config: { fxType: "filter_sweep", params: { cutoffPct: 25, resonancePct: 80 }, targetKey: "master" } };

  const result = routeInput(state, { type: "grid_press", x: 2, y: 3 }, mockBehavior);
  state = result.state;

  assert.deepEqual((state.runtimeConfig as any).touchFx.assignments, [
    { x: 2, y: 3, config: { fxType: "filter_sweep", params: { cutoffPct: 25, resonancePct: 80 }, targetKey: "master" } }
  ]);
  assert.equal(result.effects.length, 0);
});

test("Dance FX press and release emit momentary effects", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.danceMode = "fx";
  (state.runtimeConfig as any).touchFx.assignments = [
    { x: 1, y: 2, config: { fxType: "stutter", params: { rateHz: 12, depthPct: 90 }, targetKey: "master" } }
  ];

  const press = routeInput(state, { type: "grid_press", x: 1, y: 2 }, mockBehavior);
  assert.equal(press.state.system.activeFx.length, 1);
  assert.deepEqual(press.effects, [
    { type: "audio_command", command: { type: "momentary_fx_start", id: "momentary-fx:1:2", fxType: "stutter", params: { rateHz: 12, depthPct: 90 }, target: { type: "global" } } }
  ]);

  const release = routeInput(press.state, { type: "grid_release", x: 1, y: 2 }, mockBehavior);
  assert.equal(release.state.system.activeFx.length, 0);
  assert.deepEqual(release.effects, [{ type: "audio_command", command: { type: "momentary_fx_stop", id: "momentary-fx:1:2" } }]);
});

test("Dance FX targets route into audio commands", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.danceMode = "fx";
  (state.runtimeConfig as any).touchFx.assignments = [
    { x: 0, y: 0, config: { fxType: "stutter", params: { rateHz: 12 }, targetKey: "fx_bus_1" } },
    { x: 1, y: 0, config: { fxType: "stutter", params: { rateHz: 12 }, targetKey: "instrument_2" } }
  ];

  const bus = routeInput(state, { type: "grid_press", x: 0, y: 0 }, mockBehavior);
  assert.deepEqual(bus.effects, [
    { type: "audio_command", command: { type: "momentary_fx_start", id: "momentary-fx:0:0", fxType: "stutter", params: { rateHz: 12 }, target: { type: "fx_bus", index: 0 } } }
  ]);

  const inst = routeInput(state, { type: "grid_press", x: 1, y: 0 }, mockBehavior);
  assert.deepEqual(inst.effects, [
    { type: "audio_command", command: { type: "momentary_fx_start", id: "momentary-fx:1:0", fxType: "stutter", params: { rateHz: 12 }, target: { type: "instrument", index: 1 } } }
  ]);
});

test("Dance FX enforces fixed capability limit and same-type replacement", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.danceMode = "fx";
  (state.runtimeConfig as any).touchFx.assignments = [
    { x: 0, y: 0, config: { fxType: "stutter", params: { rateHz: 6 }, targetKey: "master" } },
    { x: 1, y: 0, config: { fxType: "freeze", params: { releaseMs: 500 }, targetKey: "master" } },
    { x: 2, y: 0, config: { fxType: "filter_sweep", params: { cutoffPct: 40 }, targetKey: "master" } },
    { x: 3, y: 0, config: { fxType: "pitch_shift", params: { semitones: 7 }, targetKey: "master" } },
    { x: 4, y: 0, config: { fxType: "stutter", params: { rateHz: 16 }, targetKey: "master" } }
  ];

  let current = state;
  for (let x = 0; x < PLATFORM_CAPS.touchFxMaxConcurrent; x += 1) current = routeInput(current, { type: "grid_press", x, y: 0 }, mockBehavior).state;
  assert.equal(current.system.activeFx.length, PLATFORM_CAPS.touchFxMaxConcurrent);

  const replaced = routeInput(current, { type: "grid_press", x: 4, y: 0 }, mockBehavior);
  assert.equal(replaced.state.system.activeFx.length, PLATFORM_CAPS.touchFxMaxConcurrent);
  assert.equal(replaced.state.system.activeFx.some((fx) => fx.cellX === 4), true);
  assert.deepEqual(replaced.effects, [
    { type: "audio_command", command: { type: "momentary_fx_stop", id: "momentary-fx:0:0" } },
    { type: "audio_command", command: { type: "momentary_fx_start", id: "momentary-fx:4:0", fxType: "stutter", params: { rateHz: 16 }, target: { type: "global" } } }
  ]);
});

test("Dance FX LEDs show assigned, active, and limit states", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.system.danceMode = "fx";
  (state.runtimeConfig as any).touchFx.assignments = [
    { x: 0, y: 0, config: { fxType: "stutter", params: {}, targetKey: "master" } },
    { x: 1, y: 0, config: { fxType: "freeze", params: {}, targetKey: "master" } },
    { x: 2, y: 0, config: { fxType: "filter_sweep", params: {}, targetKey: "master" } },
    { x: 3, y: 0, config: { fxType: "pitch_shift", params: {}, targetKey: "master" } },
    { x: 4, y: 0, config: { fxType: "freeze", params: {}, targetKey: "master" } }
  ];
  for (let x = 0; x < PLATFORM_CAPS.touchFxMaxConcurrent; x += 1) state = routeInput(state, { type: "grid_press", x, y: 0 }, mockBehavior).state;

  const cells = toSimulatorFrame(state, mockBehavior).leds.cells;
  const active = cells[GRID_DOMAIN.toDisplayIndex({ x: 0, y: 0 })]!;
  const limited = cells[GRID_DOMAIN.toDisplayIndex({ x: 4, y: 0 })]!;
  const empty = cells[GRID_DOMAIN.toDisplayIndex({ x: 5, y: 0 })]!;

  assert.ok(active.r > 100 && active.g > 80 && active.b < 20);
  assert.ok(Math.abs(limited.r - limited.g) <= 1 && Math.abs(limited.g - limited.b) <= 1);
  assert.ok(empty.b > empty.r && empty.b > empty.g);
});

test("Dance FX momentary presses do not auto-save but mix and pan do", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.autoSaveDefault = true;
  state.system.danceMode = "fx";
  (state.runtimeConfig as any).touchFx.assignments = [
    { x: 1, y: 2, config: { fxType: "stutter", params: { rateHz: 12 }, targetKey: "master" } }
  ];

  const fx = routeInput(state, { type: "grid_press", x: 1, y: 2 }, mockBehavior);
  assert.equal(fx.effects.some((effect) => effect.type === "store_save_default"), false);

  state = { ...fx.state, system: { ...fx.state.system, danceMode: "mix" } };
  const mix = routeInput(state, { type: "grid_press", x: 1, y: 2 }, mockBehavior);
  assert.equal(mix.effects.some((effect) => effect.type === "store_save_default"), true);

  state = { ...mix.state, system: { ...mix.state.system, danceMode: "pan" } };
  const pan = routeInput(state, { type: "grid_press", x: 1, y: 2 }, mockBehavior);
  assert.equal(pan.effects.some((effect) => effect.type === "store_save_default"), true);
});

test("sectioned row scan cursor starts from the top section", () => {
  const leds = cellsToLeds(
    Array.from({ length: PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight }, () => false),
    undefined,
    { axis: "rows", index: 0, sections: "2" },
    1
  );

  const top = leds[GRID_DOMAIN.toDisplayIndex({ x: 0, y: PLATFORM_CAPS.gridHeight - 1 })]!;
  const bottom = leds[GRID_DOMAIN.toDisplayIndex({ x: 0, y: 0 })]!;

  assert.ok(top.r > bottom.r);
});

function pixel565(pixels: Uint8Array, x: number, y: number): number {
  const offset = (y * 128 + x) * 2;
  return ((pixels[offset] ?? 0) << 8) | (pixels[offset + 1] ?? 0);
}

test("modulation mode labels are user-facing", () => {
  let state = createInitialState(mockBehavior);
  state.runtimeConfig.x.filterCutoff.enabled = true;
  const frame = toSimulatorFrame(state, mockBehavior);
  const rendered = frame.display.lines.join(" ");
  assert.equal(rendered.includes("filter_cutoff"), false);
});

test("additive pitch uses shared starting/lowest/highest", () => {
  let state = createInitialState(mockBehavior);
  state.transport.playing = true;
  state.runtimeConfig.algorithmStepUnit = "1/16";
  state.runtimeConfig.pitch.startingNote = 60;
  state.runtimeConfig.pitch.lowestNote = 48;
  state.runtimeConfig.pitch.highestNote = 84;
  state.runtimeConfig.pitch.outOfRange = "clamp";
  state.mappingConfig.deactivate.action = "note_on";
  state.runtimeConfig.x.pitch.enabled = true;
  state.runtimeConfig.x.pitch.steps = 1;
  state.runtimeConfig.y.pitch.enabled = true;
  state.runtimeConfig.y.pitch.steps = 8;

  const result = tick(state, mockBehavior);
  const note = result.events.find((e) => e.type === "note_on");
  assert.ok(note && note.type === "note_on");
  if (note && note.type === "note_on") {
    assert.ok(note.note >= 48 && note.note <= 84);
  }
});

test("default note mapping range is C2 to D5 with C4 start", () => {
  const state = createInitialState(mockBehavior);
  assert.equal(state.runtimeConfig.pitch.lowestNote, 36);
  assert.equal(state.runtimeConfig.pitch.startingNote, 60);
  assert.equal(state.runtimeConfig.pitch.highestNote, 74);
});

test("default axis pitch steps are x=0 and y=1", () => {
  const state = createInitialState(mockBehavior);
  assert.equal(state.runtimeConfig.x.pitch.steps, 0);
  assert.equal(state.runtimeConfig.y.pitch.steps, 1);
});

test("section restart makes pitch mapping local to scan section", () => {
  const state = createInitialState(mockBehavior) as any;
  const cfg = state.runtimeConfig;
  cfg.scanMode = "scanning";
  cfg.scanAxis = "rows";
  cfg.scanSections = "2";
  cfg.x.pitch.enabled = false;
  cfg.y.pitch.enabled = true;
  cfg.y.pitch.steps = 1;
  cfg.y.pitch.restartEachSection = false;

  const absolute = pitchFromIntent({ x: 0, y: 4 }, cfg, 60);
  cfg.y.pitch.restartEachSection = true;
  const firstSection = pitchFromIntent({ x: 0, y: 0 }, cfg, 60);
  const secondSection = pitchFromIntent({ x: 0, y: 4 }, cfg, 60);

  assert.notEqual(absolute, firstSection);
  assert.equal(secondSection, firstSection);
});

test("config save default requires confirmation before emitting effect", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";

  const turn = (delta: number) => {
    state = routeInput(state, { type: "encoder_turn", delta }, mockBehavior).state;
  };

  const press = (): { effects: any[] } => {
    const r = routeInput(state, { type: "encoder_press" }, mockBehavior);
    state = r.state;
    return r as any;
  };

  const selectLabel = (label: string) => {
    for (let i = 0; i < 80; i += 1) {
      const frame = toSimulatorFrame(state, mockBehavior);
      const selected = frame.display.lines.find((l) => l.startsWith("@@")) ?? "";
      if (selected.includes(label)) return;
      turn(1);
    }
    assert.fail(`failed to select label: ${label}`);
  };

  selectLabel("System");
  press();
  selectLabel("Saves");
  press();
  selectLabel("Default");
  press();

  // Cursor 0 should be Save Default.
  const before = press();
  assert.equal(before.effects.length, 0);
  assert.ok(state.system.confirm);

  // Choose Yes and confirm.
  turn(1);
  const confirmed = press();
  const saveEffect = confirmed.effects.find((e) => e.type === "store_save_default");
  assert.ok(saveEffect);
  assert.equal(saveEffect?.type === "store_save_default" ? saveEffect.mode : undefined, "immediate");
});

test("entering MIDI Out/In menu requests port lists", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";

  const turn = (delta: number) => {
    state = routeInput(state, { type: "encoder_turn", delta }, mockBehavior).state;
  };

  const press = (): { effects: any[] } => {
    const r = routeInput(state, { type: "encoder_press" }, mockBehavior);
    state = r.state;
    return r as any;
  };

  const selectLabel = (label: string) => {
    for (let i = 0; i < 80; i += 1) {
      const frame = toSimulatorFrame(state, mockBehavior);
      const selected = frame.display.lines.find((l) => l.startsWith("@@")) ?? "";
      if (selected.includes(label)) return;
      turn(1);
    }
    assert.fail(`failed to select label: ${label}`);
  };

  // System -> MIDI -> MIDI Out
  selectLabel("System");
  press();
  selectLabel("MIDI");
  press();
  selectLabel("MIDI Out");
  const out = press();
  assert.ok(out.effects.some((e) => e.type === "midi_list_outputs_request"));

  // Back to MIDI group and enter MIDI In
  state = routeInput(state, { type: "button_a" }, mockBehavior).state;
  selectLabel("MIDI In");
  const inn = press();
  assert.ok(inn.effects.some((e) => e.type === "midi_list_inputs_request"));
});

test("shift+back deletes character when editing draft name", () => {
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
      const frame = toSimulatorFrame(state, mockBehavior);
      const selected = frame.display.lines.find((l) => l.startsWith("@@")) ?? "";
      if (selected.includes(label)) return;
      turn(1);
    }
    assert.fail(`failed to select label: ${label}`);
  };

  selectLabel("System");
  press();
  selectLabel("Saves");
  press();
  selectLabel("Library");
  press();
  selectLabel("Save As");
  press();
  selectLabel("Name");
  press(); // enter editing

  state.system.draftName = "AB";
  state.system.nameCursor = 2;

  // Hold shift then press back.
  state = routeInput(state, { type: "button_shift", pressed: true }, mockBehavior).state;
  state = routeInput(state, { type: "button_a" }, mockBehavior).state;
  assert.equal(state.system.draftName, "A");
});

test("external MIDI Start resets engine and clears pending resync", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.midi.syncMode = "external";
  state.runtimeConfig.midi.clockInEnabled = true;
  state.runtimeConfig.midi.respondToStartStop = true;

  state.transport.playing = false;
  state.transport.ppqnPulse = 123;
  state.transport.tick = 77;
  state.scanIndex = 5;
  state.system.externalPpqnPulse = 999;
  state.system.pendingResync = true;

  state = routeInput(state, { type: "midi_start" }, mockBehavior).state;
  assert.equal(state.transport.playing, true);
  assert.equal(state.transport.ppqnPulse, 0);
  assert.equal(state.transport.tick, 0);
  assert.equal(state.scanIndex, 0);
  assert.equal(state.system.externalPpqnPulse, 0);
  assert.equal(state.system.pendingResync, false);
});

test("external MIDI Continue resumes without resetting position", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.midi.syncMode = "external";
  state.runtimeConfig.midi.clockInEnabled = true;
  state.runtimeConfig.midi.respondToStartStop = true;

  state.transport.playing = false;
  state.transport.ppqnPulse = 55;
  state.transport.tick = 10;
  state.scanIndex = 3;
  state.system.externalPpqnPulse = 222;
  state.system.pendingResync = true;

  state = routeInput(state, { type: "midi_continue" }, mockBehavior).state;
  assert.equal(state.transport.playing, true);
  assert.equal(state.transport.ppqnPulse, 55);
  assert.equal(state.transport.tick, 10);
  assert.equal(state.scanIndex, 3);
  assert.equal(state.system.externalPpqnPulse, 222);
  assert.equal(state.system.pendingResync, true);
});

test("external MIDI Start does nothing while pausedByUser is set", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.midi.syncMode = "external";
  state.runtimeConfig.midi.clockInEnabled = true;
  state.runtimeConfig.midi.respondToStartStop = true;

  state.transport.playing = false;
  state.transport.ppqnPulse = 44;
  state.scanIndex = 2;
  state.system.externalPpqnPulse = 88;
  state.system.pendingResync = true;
  state.system.pausedByUser = true;

  state = routeInput(state, { type: "midi_start" }, mockBehavior).state;
  assert.equal(state.transport.playing, false);
  assert.equal(state.transport.ppqnPulse, 44);
  assert.equal(state.scanIndex, 2);
  assert.equal(state.system.externalPpqnPulse, 88);
  assert.equal(state.system.pendingResync, true);
});

test("external MIDI clock advances external position even when locally paused", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.runtimeConfig.midi.syncMode = "external";
  state.runtimeConfig.midi.clockInEnabled = true;
  state.transport.playing = false;
  state.system.externalPpqnPulse = 0;

  const result = routeInput(state, { type: "midi_clock", pulses: 24 }, mockBehavior);
  assert.equal(result.events.length, 0);
  assert.equal(result.state.system.externalPpqnPulse, 24);
});

test("Fn+left-column grid press selects active part", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state = routeInput(state, { type: "button_fn", pressed: true }, mockBehavior).state;
  state = routeInput(state, { type: "grid_press", x: 0, y: 3 }, mockBehavior).state;
  assert.equal((state.runtimeConfig as any).activePartIndex, 3);
  assert.equal(state.system.toast?.message, "Part 4");
});

test("Fn+Shift+Enter opens contextual help popup", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";

  state = routeInput(state, { type: "button_shift", pressed: true }, mockBehavior).state;
  state = routeInput(state, { type: "button_fn", pressed: true }, mockBehavior).state;
  state = routeInput(state, { type: "encoder_press", id: "main" }, mockBehavior).state;

  assert.equal(state.system.confirm?.kind, "help_info");
  assert.equal(state.system.confirm?.options[0], "Close");
  assert.equal(state.system.confirm?.scroll, 0);
});

test("contextual help uses specific text for submenu entries", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";

  const turn = (delta: number) => {
    state = routeInput(state, { type: "encoder_turn", delta }, mockBehavior).state;
  };

  const press = () => {
    state = routeInput(state, { type: "encoder_press", id: "main" }, mockBehavior).state;
  };

  const selectLabel = (label: string) => {
    for (let i = 0; i < 120; i += 1) {
      const frame = toSimulatorFrame(state, mockBehavior);
      const selected = frame.display.lines.find((line) => line.startsWith("@@")) ?? "";
      if (selected.includes(label)) return;
      turn(1);
    }
    assert.fail(`failed to select label: ${label}`);
  };

  selectLabel("L3: Voice");
  press();
  selectLabel("Instruments");

  state = routeInput(state, { type: "button_shift", pressed: true }, mockBehavior).state;
  state = routeInput(state, { type: "button_fn", pressed: true }, mockBehavior).state;
  press();

  assert.equal(state.system.confirm?.kind, "help_info");
  if (state.system.confirm?.action.kind === "help_info") {
    const text = state.system.confirm.action.lines.join(" ");
    assert.match(text, /destination instrument slots that Sense trigger mappings play into/i);
    assert.doesNotMatch(text, /opens this submenu and shows related settings/i);
  }
});

test("help popup turn scrolls and enter closes without executing menu action", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";
  state.menu.cursor = 5; // System at root

  state = routeInput(state, { type: "button_shift", pressed: true }, mockBehavior).state;
  state = routeInput(state, { type: "button_fn", pressed: true }, mockBehavior).state;
  state = routeInput(state, { type: "encoder_press", id: "main" }, mockBehavior).state;

  assert.equal(state.system.confirm?.kind, "help_info");
  if (state.system.confirm?.action.kind === "help_info") {
    state.system.confirm = {
      ...state.system.confirm,
      action: { ...state.system.confirm.action, lines: ["l1", "l2", "l3", "l4", "l5", "l6", "l7", "l8"] },
      scroll: 0
    };
  }

  state = routeInput(state, { type: "encoder_turn", id: "main", delta: 1 }, mockBehavior).state;
  assert.equal(state.system.confirm?.scroll, 1);

  state = routeInput(state, { type: "encoder_turn", id: "main", delta: -1 }, mockBehavior).state;
  assert.equal(state.system.confirm?.scroll, 0);

  for (let i = 0; i < 20; i += 1) {
    state = routeInput(state, { type: "encoder_turn", id: "main", delta: 1 }, mockBehavior).state;
  }
  assert.equal(state.system.confirm?.scroll, 3, "scroll should advance far enough to reveal the last help line");

  const helpView = currentMenuView({
    state,
    menuTree: (nextState) => buildMenuTree(nextState, {
      resolveBehavior: () => mockBehavior,
      axisGroup,
      presetListNodes: () => [],
      presetRenameNodes: () => [],
      midiOutputNodes: () => [],
      midiInputNodes: () => [],
      sampleBrowserNodes: () => []
    }),
    resolveBehavior: () => mockBehavior,
    fitOledText: (text: string) => fitOledText(text, OLED_TEXT_COLUMNS),
    readAnyValue,
    formatDisplayValue,
    oledTextLines: 8
  });

  assert.deepEqual(helpView.lines, ["l4", "l5", "l6", "l7", "l8", "@@> Close"]);

  for (let i = 0; i < 20; i += 1) {
    state = routeInput(state, { type: "encoder_turn", id: "main", delta: -1 }, mockBehavior).state;
  }
  assert.equal(state.system.confirm?.scroll, 0, "scroll should clamp at top");

  state = routeInput(state, { type: "encoder_press", id: "main" }, mockBehavior).state;
  assert.equal(state.system.confirm, null);
  assert.equal(state.menu.stack.length, 0);
});

test("modifier releases are applied while help popup is open", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "normal";

  state = routeInput(state, { type: "button_shift", pressed: true }, mockBehavior).state;
  state = routeInput(state, { type: "button_fn", pressed: true }, mockBehavior).state;
  state = routeInput(state, { type: "encoder_press", id: "main" }, mockBehavior).state;
  assert.equal(state.system.confirm?.kind, "help_info");

  state = routeInput(state, { type: "button_fn", pressed: false }, mockBehavior).state;
  state = routeInput(state, { type: "button_shift", pressed: false }, mockBehavior).state;
  assert.equal(state.system.fnHeld, false);
  assert.equal(state.system.shiftHeld, false);

  state = routeInput(state, { type: "button_a", pressed: true }, mockBehavior).state;
  assert.equal(state.system.confirm, null);
  assert.equal(state.system.fnHeld, false);
  assert.equal(state.system.shiftHeld, false);
});

test("startup splash close shows help hint toast", () => {
  let state = createInitialState(mockBehavior);
  state.system.oledMode = "splash";
  state.system.oledSplashText = "Starting up";
  state.system.oledSplashUntilMs = Date.now() - 1;

  state = tick(state, mockBehavior, 0).state;

  assert.equal(state.system.oledMode, "normal");
  assert.equal(state.system.toast?.message, "Help=Sh+Fn+Enter");
});

test("sample assign mode cycles high-medium-low-off when velocity levels are on", () => {
  let state = createInitialState(mockBehavior) as any;
  state.system.oledMode = "normal";
  state.runtimeConfig.instruments[0].type = "sampler";
  state.runtimeConfig.instruments[0].sample.selectedSlot = 2;
  state.runtimeConfig.instruments[0].sample.velocityLevelsEnabled = true;
  state.system.sampleAssign = { instrumentSlot: 0, sampleSlot: 2 };

  const press = () => {
    state = routeInput(state, { type: "grid_press", x: 1, y: 1 } as DeviceInput, mockBehavior).state;
    return state.runtimeConfig.instruments[0].sample.assignments.find((a: any) => a.x === 1 && a.y === 1);
  };

  const a1 = press();
  assert.equal(a1.level, "high");
  const a2 = press();
  assert.equal(a2.level, "medium");
  const a3 = press();
  assert.equal(a3.level, "low");
  press();
  const a4 = state.runtimeConfig.instruments[0].sample.assignments.find((a: any) => a.x === 1 && a.y === 1);
  assert.equal(a4, undefined);
});

test("trigger probability assign mode supports shift row and fn+shift column", () => {
  let state = createInitialState(mockBehavior) as any;
  state.system.oledMode = "normal";
  state.system.triggerProbabilityAssign = { partIndex: 0 };
  const activePart = state.runtimeConfig.activePartIndex ?? 0;
  const total = PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight;
  state.runtimeConfig.parts[activePart].l2.triggerProbabilityMap = Array.from({ length: total }, () => "zero");

  state = routeInput(state, { type: "button_shift", pressed: true } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "grid_press", x: 2, y: 1 } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "button_shift", pressed: false } as DeviceInput, mockBehavior).state;
  for (let cx = 0; cx < PLATFORM_CAPS.gridWidth; cx += 1) {
    assert.equal(state.runtimeConfig.parts[activePart].l2.triggerProbabilityMap[1 * PLATFORM_CAPS.gridWidth + cx], "low");
  }

  const colX = 3;
  state = routeInput(state, { type: "button_fn", pressed: true } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "button_shift", pressed: true } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "grid_press", x: colX, y: 2 } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "button_shift", pressed: false } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "button_fn", pressed: false } as DeviceInput, mockBehavior).state;
  for (let cy = 0; cy < PLATFORM_CAPS.gridHeight; cy += 1) {
    assert.equal(state.runtimeConfig.parts[activePart].l2.triggerProbabilityMap[cy * PLATFORM_CAPS.gridWidth + colX], "low");
  }
});

test("trigger-gate page edits only the selected part row", () => {
  let state = createInitialState(mockBehavior) as any;
  state.system.oledMode = "normal";
  state.system.danceMode = "trigger-gate";
  state.runtimeConfig.parts[0].l2.triggerProbabilityMode = "full";
  state.runtimeConfig.parts[1].l2.triggerProbabilityMode = "full";

  state = routeInput(state, { type: "grid_press", x: 0, y: 1 } as DeviceInput, mockBehavior).state;
  assert.equal(state.runtimeConfig.parts[0].l2.triggerProbabilityMode, "full", "part 0 unchanged");
  assert.equal(state.runtimeConfig.parts[1].l2.triggerProbabilityMode, "zero", "part 1 mode set to zero");
});

test("trigger-gate all-parts buttons edit all parts", () => {
  let state = createInitialState(mockBehavior) as any;
  state.system.oledMode = "normal";
  state.system.danceMode = "trigger-gate";
  for (let i = 0; i < PLATFORM_CAPS.partCount; i += 1) {
    state.runtimeConfig.parts[i].l2.triggerProbabilityMode = "full";
  }

  state = routeInput(state, { type: "grid_press", x: 6, y: 0 } as DeviceInput, mockBehavior).state;
  for (let i = 0; i < PLATFORM_CAPS.partCount; i += 1) {
    assert.equal(state.runtimeConfig.parts[i].l2.triggerProbabilityMode, "custom", `part ${i} mode set to custom`);
  }
});

test("Fn+Play toggles active part trigger mode to zero and restores it", () => {
  let state = createInitialState(mockBehavior) as any;
  state.system.oledMode = "normal";
  state.runtimeConfig.parts[0].l2.triggerProbabilityMode = "custom";

  state = routeInput(state, { type: "button_fn", pressed: true } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "button_s", pressed: true } as DeviceInput, mockBehavior).state;
  assert.equal(state.runtimeConfig.parts[0].l2.triggerProbabilityMode, "zero");

  state = routeInput(state, { type: "button_s", pressed: true } as DeviceInput, mockBehavior).state;
  assert.equal(state.runtimeConfig.parts[0].l2.triggerProbabilityMode, "custom");
  state = routeInput(state, { type: "button_fn", pressed: false } as DeviceInput, mockBehavior).state;
});

test("sample assign mode supports shift row and fn+shift column", () => {
  let state = createInitialState(mockBehavior) as any;
  state.system.oledMode = "normal";
  state.system.fnHeld = false;
  state.runtimeConfig.instruments[0].type = "sampler";
  state.runtimeConfig.instruments[0].sample.selectedSlot = 1;
  state.runtimeConfig.instruments[0].sample.velocityLevelsEnabled = false;
  state.system.sampleAssign = { instrumentSlot: 0, sampleSlot: 1 };

  state = routeInput(state, { type: "button_shift", pressed: true } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "grid_press", x: 2, y: 3 } as DeviceInput, mockBehavior).state;
  const rowAssigned = state.runtimeConfig.instruments[0].sample.assignments.filter((a: any) => a.y === 3 && a.sampleSlot === 1);
  assert.equal(rowAssigned.length, PLATFORM_CAPS.gridWidth);

  state = routeInput(state, { type: "button_fn", pressed: true } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "grid_press", x: 3, y: 4 } as DeviceInput, mockBehavior).state;
  state = routeInput(state, { type: "button_fn", pressed: false } as DeviceInput, mockBehavior).state;
  const colAssigned = state.runtimeConfig.instruments[0].sample.assignments.filter((a: any) => a.x === 3 && a.sampleSlot === 1);
  assert.equal(colAssigned.length, PLATFORM_CAPS.gridHeight);
});
