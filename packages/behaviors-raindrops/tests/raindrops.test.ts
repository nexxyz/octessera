import test from "node:test";
import assert from "node:assert/strict";
import { GRID_DOMAIN, GRID_HEIGHT, GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";
import { raindropsBehavior, type RaindropsState } from "../src/index";

const CELL_COUNT = GRID_WIDTH * GRID_HEIGHT;

function idx(x: number, y: number): number {
  return GRID_DOMAIN.indexOf({ x, y });
}

test("drop falls down one row each tick", () => {
  const state = raindropsBehavior.init({ autoDropInterval: 0, splashRadius: 3 });
  const s: RaindropsState = { ...state, drops: [{ x: 3, y: 0 }], rings: [] };

  const next1 = raindropsBehavior.onTick(s, { bpm: 120, emit: () => {} });
  assert.equal(next1.drops.length, 1);
  assert.equal(next1.drops[0].y, 1, "drop should move down one row");
  assert.equal(next1.drops[0].x, 3);
});

test("drop lands at bottom and creates splash ring", () => {
  const state = raindropsBehavior.init({ autoDropInterval: 0, splashRadius: 3 });
  const s: RaindropsState = { ...state, drops: [{ x: 4, y: GRID_HEIGHT - 2 }], rings: [] };

  const next = raindropsBehavior.onTick(s, { bpm: 120, emit: () => {} });
  assert.equal(next.drops.length, 0, "drop should be removed after landing");
  assert.equal(next.rings.length, 1, "splash ring should be created");
  assert.equal(next.rings[0].ox, 4);
  assert.equal(next.rings[0].oy, GRID_HEIGHT - 1);
  assert.equal(next.rings[0].radius, 0);
});

test("splash radius 0 prevents ring creation", () => {
  const state = raindropsBehavior.init({ autoDropInterval: 0, splashRadius: 0 });
  const s: RaindropsState = { ...state, drops: [{ x: 4, y: GRID_HEIGHT - 2 }], rings: [] };

  const next = raindropsBehavior.onTick(s, { bpm: 120, emit: () => {} });
  assert.equal(next.drops.length, 0, "drop should land");
  assert.equal(next.rings.length, 0, "no rings should be created when splashRadius is 0");
});

test("ring expands each tick until splashRadius", () => {
  const state = raindropsBehavior.init({ autoDropInterval: 0, splashRadius: 3 });
  const s: RaindropsState = { ...state, drops: [], rings: [{ ox: 4, oy: 4, radius: 0 }] };

  const next1 = raindropsBehavior.onTick(s, { bpm: 120, emit: () => {} });
  assert.equal(next1.rings.length, 1);
  assert.equal(next1.rings[0].radius, 1);

  const next2 = raindropsBehavior.onTick(next1, { bpm: 120, emit: () => {} });
  assert.equal(next2.rings[0].radius, 2);

  const next3 = raindropsBehavior.onTick(next2, { bpm: 120, emit: () => {} });
  assert.equal(next3.rings[0].radius, 3);

  const next4 = raindropsBehavior.onTick(next3, { bpm: 120, emit: () => {} });
  assert.equal(next4.rings.length, 0, "ring should be removed when radius exceeds splashRadius");
});

test("renderModel returns expected structure", () => {
  const state = raindropsBehavior.init({});
  const model = raindropsBehavior.renderModel(state);
  assert.equal(model.name, "raindrops");
  assert.equal(model.cells.length, CELL_COUNT);
  assert.equal(model.triggerTypes.length, CELL_COUNT);
});

test("configMenu returns expected items", () => {
  const state = raindropsBehavior.init({});
  const menu = raindropsBehavior.configMenu!(state);
  assert.equal(menu.length, 4);
  assert.equal(menu[0].key, "autoDropInterval");
  assert.equal(menu[1].key, "spawnStep");
  assert.equal(menu[2].key, "splashRadius");
  assert.equal(menu[3].key, "dropNow");
  assert.equal(menu[3].type, "action");
});
