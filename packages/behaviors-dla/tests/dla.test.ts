import test from "node:test";
import assert from "node:assert/strict";
import { GRID_DOMAIN, GRID_HEIGHT, GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";
import { dlaBehavior, type DlaState } from "../src/index";

const CELL_COUNT = GRID_WIDTH * GRID_HEIGHT;

function idx(x: number, y: number): number {
  return GRID_DOMAIN.indexOf({ x, y });
}

test("init creates seed cluster at center", () => {
  const state = dlaBehavior.init({});
  const cx = Math.floor(GRID_WIDTH / 2);
  const cy = Math.floor(GRID_HEIGHT / 2);
  assert.equal(state.cells[idx(cx, cy)], true, "center cell should be alive");
  assert.equal(state.cells[idx(cx + 1, cy)], true, "right neighbor should be alive");
  assert.equal(state.cells[idx(cx, cy + 1)], true, "bottom neighbor should be alive");
});

test("onInput grid_press toggles cell", () => {
  const state = dlaBehavior.init({});
  const input: DeviceInput = { type: "grid_press", x: 5, y: 5 };
  const next = dlaBehavior.onInput(state, input, { bpm: 120, emit: () => {} });
  assert.equal(next.cells[idx(5,5)], true, "cell should toggle on");

  const toggled = dlaBehavior.onInput(next, input, { bpm: 120, emit: () => {} });
  assert.equal(toggled.cells[idx(5,5)], false, "cell should toggle off");
});

test("renderModel returns expected structure", () => {
  const state = dlaBehavior.init({});
  const model = dlaBehavior.renderModel(state);
  assert.equal(model.name, "dla");
  assert.equal(model.cells.length, CELL_COUNT);
  assert.equal(model.triggerTypes.length, CELL_COUNT);
});

test("configMenu returns expected items", () => {
  const state = dlaBehavior.init({});
  const menu = dlaBehavior.configMenu!(state);
  assert.equal(menu.length, 3);
  assert.equal(menu[0].key, "spawnInterval");
  assert.equal(menu[1].key, "spawnStep");
  assert.equal(menu[2].key, "seedCluster");
  assert.equal(menu[2].type, "action");
});
