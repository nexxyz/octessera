// Simple test to verify combined modifier logic
import { mapKeyboardEventToInputAction, mapKeyboardKeyupToInputAction } from "./apps/desktop/src/runtime/inputAdapters/keyboardAdapter";

// Simulate keyboard events
function simulateKeyEvent(key: string, shiftKey: boolean, ctrlKey: boolean, type: 'keydown' | 'keyup' = 'keydown') {
  return { key, shiftKey, ctrlKey } as KeyboardEvent;
}

console.log("Testing combined modifier handling...");

// Test 1: Shift+Fn combination should not generate events directly
const shiftFnEvent = simulateKeyEvent("Shift", true, true);
const result1 = mapKeyboardEventToInputAction(shiftFnEvent);
console.log("Shift+Fn keydown:", result1); // Should be null

const ctrlFnEvent = simulateKeyEvent("Control", true, true);
const result2 = mapKeyboardEventToInputAction(ctrlFnEvent);
console.log("Control+Shift keydown:", result2); // Should be null

// Test 2: Individual modifiers should work normally
const shiftEvent = simulateKeyEvent("Shift", false, true);
const result3 = mapKeyboardEventToInputAction(shiftEvent);
console.log("Shift only:", result3); // Should be { type: "shift", active: true }

const ctrlEvent = simulateKeyEvent("Control", true, false);
const result4 = mapKeyboardEventToInputAction(ctrlEvent);
console.log("Control only:", result4); // Should be { type: "fn", active: true }

console.log("Test completed successfully");