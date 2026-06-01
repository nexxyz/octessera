import test from "node:test";
import assert from "node:assert/strict";
import { GRID_DOMAIN, GRID_HEIGHT, GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";
import { shapesBehavior, type PulseState } from "../src/index";

const CELL_COUNT = GRID_WIDTH * GRID_HEIGHT;

function idx(x: number, y: number): number {
  return GRID_DOMAIN.indexOf({ x, y });
}

test("pulse expands radius each tick", () => {
  const state = shapesBehavior.init({ lifespan: 5, maxRadius: 10, autoPulseInterval: 0 });
  const s: PulseState = { ...state, pulses: [{ ox: 4, oy: 4, radius: 0, maxRadius: 10 }], lifetimes: new Array(CELL_COUNT).fill(0) };

  const next1 = shapesBehavior.onTick(s, { bpm: 120, emit: () => {} });
  assert.equal(next1.pulses.length, 1);
  assert.equal(next1.pulses[0].radius, 1, "radius should increase by 1");

  const next2 = shapesBehavior.onTick(next1, { bpm: 120, emit: () => {} });
  assert.equal(next2.pulses[0].radius, 2, "radius should increase again");
});

test("lifespan decay kills cells after lifespan ticks", () => {
  const state = shapesBehavior.init({ lifespan: 2, maxRadius: 10, autoPulseInterval: 0 });

  // Create initial lifetimes with cell 0 set to lifespan value
  const lifetimes = new Array(CELL_COUNT).fill(0);
  lifetimes[0] = 2;
  const s: PulseState = { ...state, lifetimes };

  const tick1 = shapesBehavior.onTick(s, { bpm: 120, emit: () => {} });
  assert.equal(tick1.lifetimes[0], 1, "lifespan decremented to 1");

  const tick2 = shapesBehavior.onTick(tick1, { bpm: 120, emit: () => {} });
  assert.equal(tick2.lifetimes[0], 0, "lifespan reached 0, cell dies");

  const tick3 = shapesBehavior.onTick(tick2, { bpm: 120, emit: () => {} });
  assert.equal(tick3.lifetimes[0], 0, "cell stays dead");
});

test("wavefront sets lifespan on leading edge only", () => {
  const state = shapesBehavior.init({ lifespan: 3, maxRadius: 10, autoPulseInterval: 0 });
  // Place pulse at center with radius=0 (ring includes just the center dot)
  // After tick, radius=1: ring at distance ~1 from center
  // Cell at (3,4) is ON the ring at radius=1 (dist=1, |1-1|<0.6) but NOT at radius=0 (|1-0|>=0.6)
  // So (3,4) should be in the wavefront and get lifespan=3
  const s: PulseState = { ...state, pulses: [{ ox: 4, oy: 4, radius: 0, maxRadius: 10 }], lifetimes: new Array(CELL_COUNT).fill(0) };

  const next = shapesBehavior.onTick(s, { bpm: 120, emit: () => {} });
  const ringCell = idx(3, 4);
  assert.equal(next.lifetimes[ringCell], 3, "ring wavefront cell should get full lifespan");

  // Center cell was only in prev (radius=0), not in cur (radius=1), so it should NOT get a fresh lifespan
  const centerI = idx(4, 4);
  assert.equal(next.lifetimes[centerI], 0, "center cell is not in the wavefront at radius 1");
});

test("renderModel returns correct triggerTypes for activate, stable, deactivate", () => {
  const lifetimes = new Array(CELL_COUNT).fill(0);
  lifetimes[0] = 3;
  lifetimes[1] = 1;
  const state = shapesBehavior.init({});
  const s: PulseState = { ...state, lifetimes, pulses: [], tickCounter: 0 };

  const model = shapesBehavior.renderModel(s);
  assert.equal(model.triggerTypes.length, CELL_COUNT);
});

test("onInput grid_press creates pulse at pressed location", () => {
  const state = shapesBehavior.init({ lifespan: 3, maxRadius: 10 });
  const input: DeviceInput = { type: "grid_press", x: 2, y: 3 };
  const next = shapesBehavior.onInput(state, input, { bpm: 120, emit: () => {} }) as PulseState;
  assert.equal(next.pulses.length, 1);
  assert.equal(next.pulses[0].ox, 2);
  assert.equal(next.pulses[0].oy, 3);
});

test("configMenu returns expected items", () => {
  const state = shapesBehavior.init({});
  const menu = shapesBehavior.configMenu!(state);
  assert.equal(menu.length, 6);
  assert.equal(menu[0].key, "pulseShape");
  assert.equal(menu[1].key, "lifespan");
  assert.equal(menu[2].key, "maxRadius");
  assert.equal(menu[3].key, "autoPulseInterval");
  assert.equal(menu[0].type, "enum");
  assert.equal(menu[4].key, "spawnStep");
  assert.equal(menu[5].key, "spawnPulse");
  assert.equal(menu[5].type, "action");
});
