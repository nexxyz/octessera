import test from "node:test";
import assert from "node:assert/strict";
import { GRID_DOMAIN, GRID_HEIGHT, GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";
import { bounceBehavior, type BounceState } from "../src/index";

const CELL_COUNT = GRID_WIDTH * GRID_HEIGHT;

function idx(x: number, y: number): number {
  return GRID_DOMAIN.indexOf({ x, y });
}

test("ball moves with velocity each tick", () => {
  const state = bounceBehavior.init({ maxBalls: 1 });
  const s: BounceState = { ...state, balls: [{ x: 3, y: 3, vx: 1, vy: 0 }] };

  const next = bounceBehavior.onTick(s, { bpm: 120, emit: () => {} });
  assert.equal(next.balls.length, 1);
  assert.ok(next.balls[0].x > 3, "ball should move right");
  assert.equal(next.balls[0].y, 3);
});

test("ball bounces off right edge", () => {
  const state = bounceBehavior.init({ maxBalls: 1 });
  // Ball at x=7.0 with vx=1 → next position is 8.0 which triggers bounce
  const s: BounceState = { ...state, balls: [{ x: GRID_WIDTH - 1, y: 3, vx: 1, vy: 0 }] };

  const next = bounceBehavior.onTick(s, { bpm: 120, emit: () => {} });
  assert.ok(next.balls[0].vx < 0, "velocity should reverse after hitting right edge");
  assert.ok(next.balls[0].x <= GRID_WIDTH - 1, "position should be in bounds");
});

test("ball bounces off left edge", () => {
  const state = bounceBehavior.init({ maxBalls: 1 });
  const s: BounceState = { ...state, balls: [{ x: -0.5, y: 3, vx: -1, vy: 0 }] };

  const next = bounceBehavior.onTick(s, { bpm: 120, emit: () => {} });
  assert.ok(next.balls[0].vx > 0, "velocity should reverse after hitting left edge");
});

test("maxBalls limits balls spawned via onInput", () => {
  const state = bounceBehavior.init({ maxBalls: 2 });
  let s: BounceState = { ...state, balls: [] };

  const input: DeviceInput = { type: "grid_press", x: 2, y: 2 };
  s = bounceBehavior.onInput(s, input, { bpm: 120, emit: () => {} }) as BounceState;
  s = bounceBehavior.onInput(s, input, { bpm: 120, emit: () => {} }) as BounceState;
  const third = bounceBehavior.onInput(s, input, { bpm: 120, emit: () => {} }) as BounceState;
  assert.equal(third.balls.length, 2);
});

test("renderModel returns trigger types for activate, stable, deactivate", () => {
  const state = bounceBehavior.init({ maxBalls: 1 });
  const s: BounceState = { ...state, balls: [{ x: 2, y: 2, vx: 0.5, vy: 0 }] };

  const next = bounceBehavior.onTick(s, { bpm: 120, emit: () => {} });
  const model = bounceBehavior.renderModel(next);
  assert.equal(model.triggerTypes.length, CELL_COUNT);
  const activate = model.triggerTypes.filter(t => t === "activate").length;
  assert.ok(activate > 0, "ball should activate cells along its path");
});

test("configMenu returns expected items", () => {
  const state = bounceBehavior.init({});
  const menu = bounceBehavior.configMenu!(state);
  assert.equal(menu.length, 4);
  assert.equal(menu[0].key, "spawnInterval");
  assert.equal(menu[1].key, "spawnStep");
  assert.equal(menu[2].key, "maxBalls");
  assert.equal(menu[3].key, "addBall");
  assert.equal(menu[3].type, "action");
});
