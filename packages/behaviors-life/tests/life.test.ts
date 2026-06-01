import test from "node:test";
import assert from "node:assert/strict";
import { GRID_DOMAIN, GRID_HEIGHT, GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";
import { lifeBehavior, type LifeState } from "../src/index";

const CELL_COUNT = GRID_WIDTH * GRID_HEIGHT;

function idx(x: number, y: number): number {
  return GRID_DOMAIN.indexOf({ x, y });
}

function setCells(state: LifeState, positions: number[]): LifeState {
  const cells = state.cells.slice();
  for (const i of positions) cells[i] = true;
  return { ...state, cells };
}

test("block (2x2) is stable under B3/S23", () => {
  const state = lifeBehavior.init({});
  const block = [idx(1,1), idx(2,1), idx(1,2), idx(2,2)];
  const s = setCells(state, block);

  const next = lifeBehavior.onTick(s, { bpm: 120, emit: () => {} });
  for (const i of block) assert.equal(next.cells[i], true, `cell ${i} should stay alive`);
  const count = next.cells.filter(Boolean).length;
  assert.equal(count, 4);
});

test("blinker (3-in-a-row) toggles between horizontal and vertical", () => {
  const state = lifeBehavior.init({});
  const horizontal = [idx(1,2), idx(2,2), idx(3,2)];
  const s = setCells(state, horizontal);

  const next1 = lifeBehavior.onTick(s, { bpm: 120, emit: () => {} });
  assert.equal(next1.cells[idx(2,2)], true, "center stays alive");
  assert.equal(next1.cells[idx(2,1)], true, "top becomes alive");
  assert.equal(next1.cells[idx(2,3)], true, "bottom becomes alive");
  assert.equal(next1.cells[idx(1,2)], false, "left dies");
  assert.equal(next1.cells[idx(3,2)], false, "right dies");

  const next2 = lifeBehavior.onTick(next1, { bpm: 120, emit: () => {} });
  assert.equal(next2.cells[idx(2,2)], true);
  assert.equal(next2.cells[idx(1,2)], true);
  assert.equal(next2.cells[idx(3,2)], true);
  assert.equal(next2.cells[idx(2,1)], false);
  assert.equal(next2.cells[idx(2,3)], false);
});

test("glider moves diagonally after 4 generations", () => {
  const state = lifeBehavior.init({});
  const glider = [idx(1,0), idx(2,1), idx(0,2), idx(1,2), idx(2,2)];
  let s = setCells(state, glider);

  for (let gen = 0; gen < 4; gen++) {
    s = lifeBehavior.onTick(s, { bpm: 120, emit: () => {} });
  }

  const alive = s.cells.map((c, i) => c ? i : -1).filter(i => i >= 0);
  assert.ok(alive.length >= 3, "glider should survive 4 gens with 3+ cells");
});

test("renderModel returns triggerTypes for activate, stable, deactivate", () => {
  const state = lifeBehavior.init({});
  const blinker = [idx(1,2), idx(2,2), idx(3,2)];
  const s = setCells(state, blinker);

  const next = lifeBehavior.onTick(s, { bpm: 120, emit: () => {} });
  const model = lifeBehavior.renderModel(next);
  assert.equal(model.triggerTypes.length, CELL_COUNT);

  const activate = model.triggerTypes.filter(t => t === "activate").length;
  const deactivate = model.triggerTypes.filter(t => t === "deactivate").length;
  const stable = model.triggerTypes.filter(t => t === "stable").length;
  assert.equal(activate, 2, "2 cells born (top+bottom)");
  assert.equal(deactivate, 2, "2 cells die (left+right)");
  assert.equal(stable, 1, "1 cell stays alive (center)");
});

test("onInput grid_press toggles cell at (x,y)", () => {
  const state = lifeBehavior.init({});
  const input: DeviceInput = { type: "grid_press", x: 3, y: 4 };
  const next = lifeBehavior.onInput(state, input, { bpm: 120, emit: () => {} });
  assert.equal(next.cells[idx(3,4)], true);
});

test("serialize/deserialize round-trip", () => {
  const state = lifeBehavior.init({});
  state.cells[0] = true;
  const raw = lifeBehavior.serialize(state);
  const restored = lifeBehavior.deserialize(raw);
  assert.equal(restored.generation, 0);
  assert.equal(restored.cells[0], true);
  assert.equal(restored.randomCellsPerTick, 0);
});

test("configMenu returns expected items", () => {
  const state = lifeBehavior.init({});
  const menu = lifeBehavior.configMenu!(state);
  assert.equal(menu.length, 4);
  assert.equal(menu[0].key, "randomCellsPerTick");
  assert.equal(menu[1].key, "randomTickInterval");
  assert.equal(menu[2].key, "spawnStep");
  assert.equal(menu[3].key, "spawnRandom");
  assert.equal(menu[3].type, "action");
});
