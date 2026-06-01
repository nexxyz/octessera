// Debug script to analyze the key event flow

console.log("=== Key Event Analysis ===");

// This is what we expect to happen with Space key:
console.log("\n1. Expected Space key behavior:");
console.log("   Press: key=' ', shiftKey=false, ctrlKey=false");
console.log("   Expected result: { type: 'device_input', input: { type: 'button_s' } }");
console.log("   Release: key=' ', shiftKey=false, ctrlKey=false");
console.log("   Expected result: { type: 'device_input', input: { type: 'button_s', pressed: false } }");

// This is what we expect to happen with Space + Shift:
console.log("\n2. Expected Space + Shift behavior:");
console.log("   Press: key=' ', shiftKey=true, ctrlKey=false");
console.log("   Expected result: { type: 'emergency_brake' }");
console.log("   Release: key=' ', shiftKey=true, ctrlKey=false");
console.log("   Expected result: null (no event generated)");

// The issue may be that Windows is sending multiple events or the wrong events
console.log("\n3. Potential Windows-specific issues:");
console.log("   - Windows might be sending 'Meta' key events instead of 'Space'");
console.log("   - There might be duplicate or incorrect key events being triggered");
console.log("   - The key event sequence might be malformed");

// Let's test the function logic step by step
console.log("\n4. Logic verification:");

// Simulate current keyboard adapter logic
function simulateKeyboardEvent(key, shiftKey, ctrlKey) {
  console.log(`   Testing key="${key}", shiftKey=${shiftKey}, ctrlKey=${ctrlKey}`);
  
  if (key === "Shift" && ctrlKey) {
    console.log("   -> Combined modifier (Shift+Ctrl) - returning null");
    return null;
  }
  
  if (key === "Control" && shiftKey) {
    console.log("   -> Combined modifier (Control+Shift) - returning null");
    return null;
  }
  
  if (key === "Shift" && !ctrlKey) {
    console.log("   -> Shift key press - returning { type: 'shift', active: true }");
    return { type: "shift", active: true };
  }
  
  if (key === "Control" && !shiftKey) {
    console.log("   -> Control key press - returning { type: 'fn', active: true }");
    return { type: "fn", active: true };
  }
  
  if (key === " " && shiftKey) {
    console.log("   -> Space + Shift - returning { type: 'emergency_brake' }");
    return { type: "emergency_brake" };
  }
  
  if (key === " ") {
    console.log("   -> Space key only - returning { type: 'device_input', input: { type: 'button_s' } }");
    return { type: "device_input", input: { type: "button_s" } };
  }
  
  console.log("   -> No match - returning null");
  return null;
}

// Test cases
console.log("\n5. Testing individual cases:");
simulateKeyboardEvent(" ", false, false);  // Space only
simulateKeyboardEvent(" ", true, false);   // Space + Shift
simulateKeyboardEvent("Shift", false, true); // Shift + Ctrl
simulateKeyboardEvent("Control", true, false); // Control + Shift

console.log("\n6. Looking for deeper issues:");
console.log("   - Could there be key repeat events causing issues?");
console.log("   - Is there an issue with how the application is interpreting key events?");
console.log("   - Are we seeing Windows-specific key codes (like 'OS' or 'Meta') instead of 'Space'?");

console.log("\nThis suggests the root cause may be in how Windows translates Space key events,");
console.log("or how the input system is detecting key events from the OS.");