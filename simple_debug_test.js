// This is a simplified test to help understand the combined modifier issue

// Simulate the state transitions
let state = {
  system: {
    fnHeld: false,
    shiftHeld: false,
    combinedModifierHeld: false
  }
};

console.log("Initial state:", state.system);

// Step 1: Press fn down
state.system.fnHeld = true;
console.log("After fn down:", state.system);

// Step 2: Press shift down
state.system.shiftHeld = true;
console.log("After shift down:", state.system);

// Step 3: Apply combined modifier logic
if (state.system.shiftHeld && state.system.fnHeld && !state.system.combinedModifierHeld) {
  console.log("Combined modifier logic should trigger");
  state.system.combinedModifierHeld = true;
  console.log("After combined modifier logic:", state.system);
} else {
  console.log("Combined modifier logic not triggered");
  console.log("shiftHeld:", state.system.shiftHeld);
  console.log("fnHeld:", state.system.fnHeld);
  console.log("combinedModifierHeld:", state.system.combinedModifierHeld);
}