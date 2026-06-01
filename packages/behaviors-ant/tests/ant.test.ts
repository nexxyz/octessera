import test from "node:test";
import assert from "node:assert/strict";
import { GRID_DOMAIN, GRID_HEIGHT, GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";
import { antBehavior, type AntState } from "../src/index";

const CELL_COUNT = GRID_WIDTH * GRID_HEIGHT;

function idx(x: number, y: number): number {
  return GRID_DOMAIN.indexOf({ x, y });
}

test("ant moves forward each tick with correct direction rules", () => {
  const state = antBehavior.init({ maxAnts: 1, autoSpawnInterval: 0 });
  const s: AntState = { ...state, ants: [{ x: 4, y: 4, dir: 0 }] };

  // dir=0 is up, on a white cell (false) → turns right (dir=1, east)
  const next1 = antBehavior.onTick(s, { bpm: 120, emit: () => {} });
  assert.equal(next1.ants.length, 1);
  assert.equal(next1.ants[0].x, 5);
  assert.equal(next1.ants[0].y, 4);
  assert.equal(next1.ants[0].dir, 1);

  // Now on black cell (toogled to true) → dir=1 (east) turns right → dir=2 (south)
  const next2 = antBehavior.onTick(next1, { bpm: 120, emit: () => {} });
  assert.equal(next2.ants[0].x, 5);
  assert.equal(next2.ants[0].y, 5);
  assert.equal(next2.ants[0].dir, 2);
});

test("ant flips cell it leaves", () => {
  const state = antBehavior.init({ maxAnts: 1, autoSpawnInterval: 0 });
  const s: AntState = { ...state, ants: [{ x: 4, y: 4, dir: 0 }] };

  const next = antBehavior.onTick(s, { bpm: 120, emit: () => {} });
  // The ant was at (4,4) on a white cell (false). It flips to true.
  assert.equal(next.cells[idx(4,4)], true, "cell (4,4) should be flipped to true");
});

test("grid wrapping works at edges", () => {
  const state = antBehavior.init({ maxAnts: 1, autoSpawnInterval: 0 });

  // Ant at x=0, dir=0 (up) on a BLACK cell → turns left (dir=3, west), wraps x to GRID_WIDTH-1
  const cells = new Array(CELL_COUNT).fill(false);
  cells[idx(0, 5)] = true; // black cell
  const s: AntState = { ...state, ants: [{ x: 0, y: 5, dir: 0 }], cells };

  const next = antBehavior.onTick(s, { bpm: 120, emit: () => {} });
  assert.equal(next.ants[0].x, GRID_WIDTH - 1, "x should wrap from 0 to GRID_WIDTH-1");
  assert.equal(next.ants[0].y, 5);
  assert.equal(next.ants[0].dir, 3, "ant should be facing west");

  // Ant at y=0, dir=0 (up) on a WHITE cell → turns right (dir=1, east), moves x+1
  const cells2 = new Array(CELL_COUNT).fill(false);
  const s2: AntState = { ...state, ants: [{ x: 3, y: 0, dir: 0 }], cells: cells2 };

  const next2 = antBehavior.onTick(s2, { bpm: 120, emit: () => {} });
  assert.equal(next2.ants[0].x, 4);
  assert.equal(next2.ants[0].y, 0);
  assert.equal(next2.ants[0].dir, 1);
});

test("maxAnts limits ants spawned via onInput", () => {
  const state = antBehavior.init({ maxAnts: 2, autoSpawnInterval: 0 });
  let s: AntState = { ...state, ants: [] };

  const input: DeviceInput = { type: "grid_press", x: 1, y: 1 };
  s = antBehavior.onInput(s, input, { bpm: 120, emit: () => {} }) as AntState;
  s = antBehavior.onInput(s, input, { bpm: 120, emit: () => {} }) as AntState;
  const third = antBehavior.onInput(s, input, { bpm: 120, emit: () => {} }) as AntState;
  assert.equal(third.ants.length, 2);
});

test("renderModel returns expected structure", () => {
  const state = antBehavior.init({});
  const model = antBehavior.renderModel(state);
  assert.equal(model.name, "ant");
  assert.equal(model.cells.length, CELL_COUNT);
  assert.equal(model.triggerTypes.length, CELL_COUNT);
});

test("configMenu returns expected items", () => {
  const state = antBehavior.init({});
  const menu = antBehavior.configMenu!(state);
  assert.equal(menu.length, 4);
  assert.equal(menu[0].key, "maxAnts");
  assert.equal(menu[1].key, "autoSpawnInterval");
  assert.equal(menu[2].key, "spawnStep");
  assert.equal(menu[3].key, "spawnAnt");
  assert.equal(menu[3].type, "action");
});
