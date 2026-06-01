import test from "node:test";
import assert from "node:assert/strict";
import { GRID_HEIGHT, GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";
import { sequencerBehavior, type SequencerState } from "../src/index";

const CELL_COUNT = GRID_WIDTH * GRID_HEIGHT;

test("onInput grid_press toggles cell at (x,y)", () => {
  const state = sequencerBehavior.init({});
  for (let i = 0; i < CELL_COUNT; i++) assert.equal(state.cells[i], false);

  const input: DeviceInput = { type: "grid_press", x: 0, y: 0 };
  const next = sequencerBehavior.onInput(state, input, { bpm: 120, emit: () => {} });
  assert.equal(next.cells[0], true);
  assert.equal(next.cells[1], false);

  const toggled = sequencerBehavior.onInput(next, input, { bpm: 120, emit: () => {} });
  assert.equal(toggled.cells[0], false);
});

test("onInput non-grid_press returns state unchanged", () => {
  const state = sequencerBehavior.init({});
  const next = sequencerBehavior.onInput(state, { type: "encoder_turn", id: "SW1", delta: 1 }, { bpm: 120, emit: () => {} });
  assert.equal(next, state);
});

test("onTick is no-op", () => {
  const state = sequencerBehavior.init({});
  const next = sequencerBehavior.onTick(state, { bpm: 120, emit: () => {} });
  assert.equal(next, state);
});

test("renderModel returns expected structure", () => {
  const state = sequencerBehavior.init({});
  const model = sequencerBehavior.renderModel(state);
  assert.equal(model.name, "sequencer");
  assert.equal(model.statusLine, "Manual");
  assert.equal(model.cells.length, CELL_COUNT);
});

test("serialize/deserialize round-trip", () => {
  const state = sequencerBehavior.init({});
  state.cells[5] = true;
  state.cells[10] = true;
  const raw = sequencerBehavior.serialize(state);
  const restored = sequencerBehavior.deserialize(raw);
  assert.equal(restored.cells[5], true);
  assert.equal(restored.cells[10], true);
  assert.equal(restored.cells[0], false);
});
