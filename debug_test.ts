import { createInitialState, routeInput } from "./packages/platform-core/src/index";
import { getBehavior } from "@cellsymphony/behavior-api";

async function test() {
  const behavior = getBehavior("life")!;
  const state = createInitialState(behavior);
  
  console.log("Initial state:", {
    shiftHeld: state.system.shiftHeld,
    fnHeld: state.system.fnHeld,
    combinedModifierHeld: state.system.combinedModifierHeld
  });
  
  // Test: button_fn down sets fnHeld
  const fnDown = routeInput(state, { type: "button_fn", pressed: true }, behavior);
  console.log("After fn down:", {
    shiftHeld: fnDown.state.system.shiftHeld,
    fnHeld: fnDown.state.system.fnHeld,
    combinedModifierHeld: fnDown.state.system.combinedModifierHeld
  });
  
  // Test: button_shift down sets shiftHeld and combinedModifierHeld
  const shiftDown = routeInput(fnDown.state, { type: "button_shift", pressed: true }, behavior);
  console.log("After shift down:", {
    shiftHeld: shiftDown.state.system.shiftHeld,
    fnHeld: shiftDown.state.system.fnHeld,
    combinedModifierHeld: shiftDown.state.system.combinedModifierHeld
  });
}

test();