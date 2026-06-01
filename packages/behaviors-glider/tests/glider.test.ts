import test from "node:test";
import assert from "node:assert/strict";
import { GRID_DOMAIN, GRID_HEIGHT, GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";
import { gliderBehavior, type GliderState } from "../src/index";

const CELL_COUNT = GRID_WIDTH * GRID_HEIGHT;

function idx(x: number, y: number): number {
  return GRID_DOMAIN.indexOf({ x, y });
}

const GLIDER_OFFSETS: Array<[number, number]> = [[1,0], [2,1], [0,2], [1,2], [2,2]];
const GLIDER_CELLS = [idx(1,0), idx(2,1), idx(0,2), idx(1,2), idx(2,2)];

test("Conway B3/S23 rules: block is stable", () => {
  const state = gliderBehavior.init({ spawnInterval: 0 });
  const block = [idx(1,1), idx(2,1), idx(1,2), idx(2,2)];
  const s: GliderState = { ...state, cells: (() => {
    const c = new Array(CELL_COUNT).fill(false);
    for (const i of block) c[i] = true;
    return c;
  })() };

  const next = gliderBehavior.onTick(s, { bpm: 120, emit: () => {} });
  for (const i of block) assert.equal(next.cells[i], true, `block cell ${i} should survive`);
  assert.equal(next.cells.filter(Boolean).length, 4);
});

test("glider spawns on first tick of each interval period", () => {
  const origRandom = Math.random;
  Math.random = () => 0.5;
  try {
    const state = gliderBehavior.init({ spawnInterval: 3 });
    let s: GliderState = { ...state, cells: new Array(CELL_COUNT).fill(false), tickCounter: 0 };

    // (tickCounter-1)%3 === 0 → spawn on tick 1, 4, 7, 10...
    const tick1 = gliderBehavior.onTick(s, { bpm: 120, emit: () => {} });
    assert.equal(tick1.tickCounter, 1);
    assert.ok(tick1.cells.filter(Boolean).length > 0, "glider spawns on first tick");

    // tick 2: no spawn
    const tick2 = gliderBehavior.onTick(tick1, { bpm: 120, emit: () => {} });
    assert.equal(tick2.tickCounter, 2);

    // tick 3: no spawn
    const tick3 = gliderBehavior.onTick(tick2, { bpm: 120, emit: () => {} });
    assert.equal(tick3.tickCounter, 3);

    // tick 4: spawn again
    const tick4 = gliderBehavior.onTick(tick3, { bpm: 120, emit: () => {} });
    assert.equal(tick4.tickCounter, 4);
    assert.ok(tick4.cells.filter(Boolean).length > 0, "cells present on tick 4");
    // With deterministic random position, the glider survives through tick 3,
    // then a new spawn at tick 4 adds cells before evolution.
    assert.ok(tick4.cells.filter(Boolean).length >= tick1.cells.filter(Boolean).length,
      "more cells by tick 4 (accumulated spawns)");
  } finally {
    Math.random = origRandom;
  }
});

test("spawnStep shifts which tick triggers the spawn", () => {
  const origRandom = Math.random;
  Math.random = () => 0.5;
  try {
    const state = gliderBehavior.init({ spawnInterval: 4 });
    let s: GliderState = { ...state, cells: new Array(CELL_COUNT).fill(false), tickCounter: 0, spawnStep: 2 };

    // spawnStep=2, interval=4: spawn at (tickCounter-1)%4 === 2%4 → tick 3, 7, 11...
    const tick1 = gliderBehavior.onTick(s, { bpm: 120, emit: () => {} });
    assert.equal(tick1.cells.filter(Boolean).length, 0, "no spawn on tick 1 (step 2)");

    const tick2 = gliderBehavior.onTick(tick1, { bpm: 120, emit: () => {} });
    assert.equal(tick2.cells.filter(Boolean).length, 0, "no spawn on tick 2");

    const tick3 = gliderBehavior.onTick(tick2, { bpm: 120, emit: () => {} });
    assert.ok(tick3.cells.filter(Boolean).length > 0, "spawn on tick 3 (step 2)");
  } finally {
    Math.random = origRandom;
  }
});

test("spawnInterval 0 disables spawning", () => {
  const state = gliderBehavior.init({ spawnInterval: 0 });
  const s: GliderState = { ...state, cells: new Array(CELL_COUNT).fill(false), tickCounter: 0 };

  // Run many ticks with spawnInterval=0 — no cells should appear
  let current = s as GliderState;
  const initialCount = current.cells.filter(Boolean).length;
  for (let i = 0; i < 10; i++) {
    current = gliderBehavior.onTick(current, { bpm: 120, emit: () => {} });
  }
  assert.equal(current.cells.filter(Boolean).length, initialCount, "no cells should spawn with interval 0");
});

test("onInput grid_press spawns glider at pressed position", () => {
  const state = gliderBehavior.init({});
  const ox = 3;
  const oy = 4;
  const input: DeviceInput = { type: "grid_press", x: ox, y: oy };
  const next = gliderBehavior.onInput(state, input, { bpm: 120, emit: () => {} });
  assert.notEqual(next, state, "should return a new state on grid_press");
  for (const [dx, dy] of GLIDER_OFFSETS) {
    const i = idx(ox + dx, oy + dy);
    assert.equal(next.cells[i], true, `glider cell at (${ox+dx},${oy+dy}) should be true`);
  }
});

test("renderModel returns triggerTypes for activate, stable, deactivate", () => {
  const state = gliderBehavior.init({ spawnInterval: 0 });
  const s: GliderState = { ...state, cells: (() => {
    const c = new Array(CELL_COUNT).fill(false);
    for (const i of GLIDER_CELLS) c[i] = true;
    return c;
  })() };

  const next = gliderBehavior.onTick(s, { bpm: 120, emit: () => {} });
  const model = gliderBehavior.renderModel(next);
  assert.equal(model.triggerTypes.length, CELL_COUNT);
  assert.equal(model.name, "glider");
  const totalTriggers = model.triggerTypes.filter(t => t !== "none").length;
  assert.ok(totalTriggers > 0, "some cells should have trigger types");
});

test("configMenu returns expected items", () => {
  const state = gliderBehavior.init({});
  const menu = gliderBehavior.configMenu!(state);
  assert.equal(menu.length, 3);
  assert.equal(menu[0].key, "spawnInterval");
  assert.equal(menu[1].key, "spawnStep");
  assert.equal(menu[2].key, "spawnGlider");
  assert.equal(menu[2].type, "action");
});
