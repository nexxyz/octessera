import test from "node:test";
import assert from "node:assert/strict";
import { createInitialState, routeInput } from "../src/index";
import { getBehavior } from "@cellsymphony/behavior-api";

test("combined modifier makes fn and shift effectively inactive", () => {
  const behavior = getBehavior("life")!;
  const state = createInitialState(behavior);
  state.system.oledMode = "normal";

  const fnDown = routeInput(state, { type: "button_fn", pressed: true }, behavior);
  assert.equal(fnDown.state.system.physicalFnHeld, true);
  assert.equal(fnDown.state.system.fnHeld, true);
  assert.equal(fnDown.state.system.shiftHeld, false);
  assert.equal(fnDown.state.system.combinedModifierHeld, false);

  const shiftDown = routeInput(fnDown.state, { type: "button_shift", pressed: true }, behavior);
  assert.equal(shiftDown.state.system.physicalFnHeld, true);
  assert.equal(shiftDown.state.system.physicalShiftHeld, true);
  assert.equal(shiftDown.state.system.fnHeld, false);
  assert.equal(shiftDown.state.system.shiftHeld, false);
  assert.equal(shiftDown.state.system.combinedModifierHeld, true);

  const shiftUp = routeInput(shiftDown.state, { type: "button_shift", pressed: false }, behavior);
  assert.equal(shiftUp.state.system.physicalFnHeld, true);
  assert.equal(shiftUp.state.system.physicalShiftHeld, false);
  assert.equal(shiftUp.state.system.fnHeld, true);
  assert.equal(shiftUp.state.system.shiftHeld, false);
  assert.equal(shiftUp.state.system.combinedModifierHeld, false);

  const fnUp = routeInput(shiftUp.state, { type: "button_fn", pressed: false }, behavior);
  assert.equal(fnUp.state.system.physicalFnHeld, false);
  assert.equal(fnUp.state.system.fnHeld, false);
  assert.equal(fnUp.state.system.combinedModifierHeld, false);
});

test("combined modifier works when shift is pressed before fn", () => {
  const behavior = getBehavior("life")!;
  const state = createInitialState(behavior);
  state.system.oledMode = "normal";

  const shiftDown = routeInput(state, { type: "button_shift", pressed: true }, behavior);
  assert.equal(shiftDown.state.system.shiftHeld, true);

  const fnDown = routeInput(shiftDown.state, { type: "button_fn", pressed: true }, behavior);
  assert.equal(fnDown.state.system.physicalShiftHeld, true);
  assert.equal(fnDown.state.system.physicalFnHeld, true);
  assert.equal(fnDown.state.system.shiftHeld, false);
  assert.equal(fnDown.state.system.fnHeld, false);
  assert.equal(fnDown.state.system.combinedModifierHeld, true);

  const fnUp = routeInput(fnDown.state, { type: "button_fn", pressed: false }, behavior);
  assert.equal(fnUp.state.system.shiftHeld, true);
  assert.equal(fnUp.state.system.fnHeld, false);
  assert.equal(fnUp.state.system.combinedModifierHeld, false);
});

test("combined modifier prevents shift back grid clear", () => {
  const behavior = getBehavior("life")!;
  const state = createInitialState(behavior);
  state.system.oledMode = "normal";

  const fnDown = routeInput(state, { type: "button_fn", pressed: true }, behavior);
  const shiftDown = routeInput(fnDown.state, { type: "button_shift", pressed: true }, behavior);
  const back = routeInput(shiftDown.state, { type: "button_a", pressed: true }, behavior);

  assert.equal(back.state.system.toast?.message, undefined);
  assert.equal(back.events.some((event) => event.type === "cc"), false);
});

test("shift back still clears grid when shift is active alone", () => {
  const behavior = getBehavior("life")!;
  const state = createInitialState(behavior);
  state.system.oledMode = "normal";

  const shiftDown = routeInput(state, { type: "button_shift", pressed: true }, behavior);
  const back = routeInput(shiftDown.state, { type: "button_a", pressed: true }, behavior);

  assert.equal(back.state.system.toast?.message, "Grid cleared");
});
