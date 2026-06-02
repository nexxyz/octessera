// Let's directly analyze the exact issue with the failing test
// by looking at what happens when we step through it

import { createInitialState, routeInput } from "./packages/platform-core/src/index";
import { getBehavior } from "@cellsymphony/behavior-api";

// Reproduce the exact failing sequence from the test
const behavior = getBehavior("life")!;
const state = createInitialState(behavior);

console.log("=== Starting state ===");
console.log("fnHeld:", state.system.fnHeld);
console.log("shiftHeld:", state.system.shiftHeld);
console.log("combinedModifierHeld:", state.system.combinedModifierHeld);

// Step 1: button_fn down sets fnHeld
console.log("\n=== Step 1: fn down ===");
const fnDown = routeInput(state, { type: "button_fn", pressed: true }, behavior);
console.log("fnHeld:", fnDown.state.system.fnHeld);
console.log("shiftHeld:", fnDown.state.system.shiftHeld);
console.log("combinedModifierHeld:", fnDown.state.system.combinedModifierHeld);

// Step 2: button_fn up clears fnHeld  
console.log("\n=== Step 2: fn up ===");
const fnUp = routeInput(fnDown.state, { type: "button_fn", pressed: false }, behavior);
console.log("fnHeld:", fnUp.state.system.fnHeld);
console.log("shiftHeld:", fnUp.state.system.shiftHeld);
console.log("combinedModifierHeld:", fnUp.state.system.combinedModifierHeld);

// Step 3: Shift down, but now fnHeld is false
console.log("\n=== Step 3: shift down ===");
const shiftDown = routeInput(fnUp.state, { type: "button_shift", pressed: true }, behavior);
console.log("fnHeld:", shiftDown.state.system.fnHeld);
console.log("shiftHeld:", shiftDown.state.system.shiftHeld);
console.log("combinedModifierHeld:", shiftDown.state.system.combinedModifierHeld);

// What the test asserts:
console.log("\n=== Test assertions ===");
console.log("fnHeld should be true:", shiftDown.state.system.fnHeld === true);
console.log("shiftHeld should be true:", shiftDown.state.system.shiftHeld === true);
console.log("combinedModifierHeld should be true:", shiftDown.state.system.combinedModifierHeld === true);

// Now, let's also try the reverse sequence:
console.log("\n=== Reverse test: Fn+Shift together ===");
const state2 = createInitialState(behavior);

// First press Fn
const fnDown2 = routeInput(state2, { type: "button_fn", pressed: true }, behavior);
console.log("After fn down - fnHeld:", fnDown2.state.system.fnHeld, "shiftHeld:", fnDown2.state.system.shiftHeld);

// Then press Shift (both should be held)
const shiftDown2 = routeInput(fnDown2.state, { type: "button_shift", pressed: true }, behavior);
console.log("After shift down - fnHeld:", shiftDown2.state.system.fnHeld, "shiftHeld:", shiftDown2.state.system.shiftHeld, "combinedModifierHeld:", shiftDown2.state.system.combinedModifierHeld);