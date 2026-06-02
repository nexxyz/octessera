import { createInitialState, routeInput } from "./packages/platform-core/src/index.js";
import { getBehavior } from "@cellsymphony/behavior-api";

// Test the combined modifier behavior
const behavior = getBehavior("life");
const state = createInitialState(behavior);

console.log("Initial state:", state.system);

// Test: button_fn down sets fnHeld
const fnDown = routeInput(state, { type: "button_fn", pressed: true }, behavior);
console.log("After fn down:", fnDown.state.system);

// Test: button_shift down sets shiftHeld
const shiftDown = routeInput(fnDown.state, { type: "button_shift", pressed: true }, behavior);
console.log("After shift down:", shiftDown.state.system);

// Check the combined modifier logic
console.log("combinedModifierHeld after shift down:", shiftDown.state.system.combinedModifierHeld);