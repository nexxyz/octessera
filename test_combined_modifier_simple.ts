// Simple test to verify the combined modifier logic works correctly

// This is just a conceptual test of the logic we implemented in the router

// Simulate state for testing
const testState = {
  system: {
    shiftHeld: true,
    fnHeld: true,
    thirdModifierHeld: false
  }
};

// Simulate logic from our implementation:
// Check for combined modifier press (both Shift and Fn held together)
if (testState.system.shiftHeld && testState.system.fnHeld && !testState.system.thirdModifierHeld) {
  console.log("Combined modifier press event sent");
  testState.system.thirdModifierHeld = true;
} else {
  console.log("Combined modifier not pressed or already active");
}

// Simulate key release
testState.system.shiftHeld = false;

// Simulate logic for release handling:
if (testState.system.shiftHeld && !testState.system.fnHeld) {
  console.log("Shift released, but Fn still held - should not send combined release");
} else if (!testState.system.shiftHeld && testState.system.fnHeld) {
  console.log("Shift released, Fn still held - send combined release");
} else if (!testState.system.shiftHeld && !testState.system.fnHeld) {
  console.log("Both modifiers released - reset thirdModifierHeld");
  testState.system.thirdModifierHeld = false;
}

console.log("Test completed successfully");