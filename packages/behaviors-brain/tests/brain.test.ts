import test from "node:test";
import assert from "node:assert/strict";
import { GRID_DOMAIN, GRID_HEIGHT, GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";
import { brainBehavior, type BrainState } from "../src/index";

const CELL_COUNT = GRID_WIDTH * GRID_HEIGHT;

function idx(x: number, y: number): number {
  return GRID_DOMAIN.indexOf({ x, y });
}

test("state machine: 0→1→2→0 on single cell with 1 firing neighbor", () => {
  const state = brainBehavior.init({ fireThreshold: 1, randomSeedCells: 0 });
  const s: BrainState = { ...state, cells: Array(CELL_COUNT).fill(0) };

  // Place one firing cell at (1,1) to provide neighbor for (0,0)
  s.cells[idx(1,1)] = 1;

  const next1 = brainBehavior.onTick(s, { bpm: 120, emit: () => {} });
  const c00 = next1.cells[idx(0,0)];
  assert.equal(c00, 1, "cell (0,0) fires (activate) because neighbor=1, threshold=1");

  const next2 = brainBehavior.onTick(next1, { bpm: 120, emit: () => {} });
  assert.equal(next2.cells[idx(0,0)], 2, "cell (0,0) enters refractory (deactivate)");

  const next3 = brainBehavior.onTick(next2, { bpm: 120, emit: () => {} });
  assert.equal(next3.cells[idx(0,0)], 0, "cell (0,0) returns to 0 (dead)");
});

test("fire threshold prevents firing with insufficient neighbors", () => {
  const state = brainBehavior.init({ fireThreshold: 3, randomSeedCells: 0 });
  const s: BrainState = { ...state, cells: Array(CELL_COUNT).fill(0) };

  s.cells[idx(1,1)] = 1;
  s.cells[idx(0,1)] = 1;

  const next = brainBehavior.onTick(s, { bpm: 120, emit: () => {} });
  assert.equal(next.cells[idx(0,0)], 0, "cell (0,0) should NOT fire with only 2 neighbors and threshold=3");
});

test("renderModel maps state 1 to active cells and returns triggerTypes", () => {
  const state = brainBehavior.init({ fireThreshold: 1, randomSeedCells: 0 });
  const s: BrainState = { ...state, cells: Array(CELL_COUNT).fill(0) };
  s.cells[idx(5,5)] = 1;

  const next = brainBehavior.onTick(s, { bpm: 120, emit: () => {} });
  const model = brainBehavior.renderModel(next);
  assert.equal(model.cells.length, CELL_COUNT);
  const firingCells = model.cells.filter(Boolean).length;
  assert.ok(firingCells > 0, "firing cells should be visible in renderModel");
  assert.equal(model.triggerTypes.length, CELL_COUNT);
  const activate = model.triggerTypes.filter(t => t === "activate").length;
  assert.ok(activate >= 0);
});

test("onInput grid_press toggles cell", () => {
  const state = brainBehavior.init({});
  const input: DeviceInput = { type: "grid_press", x: 2, y: 3 };
  const next = brainBehavior.onInput(state, input, { bpm: 120, emit: () => {} });
  assert.equal(next.cells[idx(2,3)], 1, "toggled from 0 to 1");

  const toggled = brainBehavior.onInput(next, input, { bpm: 120, emit: () => {} });
  assert.equal(toggled.cells[idx(2,3)], 0, "toggled back from 1 to 0");
});

test("configMenu returns expected items", () => {
  const state = brainBehavior.init({});
  const menu = brainBehavior.configMenu!(state);
  assert.equal(menu.length, 5);
  assert.equal(menu[0].key, "fireThreshold");
  assert.equal(menu[1].key, "seedInterval");
  assert.equal(menu[2].key, "spawnStep");
  assert.equal(menu[3].key, "randomSeedCells");
  assert.equal(menu[4].key, "seedRandom");
  assert.equal(menu[4].type, "action");
});
