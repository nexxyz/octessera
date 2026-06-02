---
name: Combined Modifier Implementation
description: Implementation of centralized combined modifier logic in Cell Symphony platform-core
source: auto-skill
extracted_at: '2026-06-01T21:20:34.222Z'
---

This skill covers the implementation of combined modifier logic in the Cell Symphony platform-core, focusing on the Shift+Fn key combination that should be treated as a single modifier.

## Key Changes

1. **Centralized Logic**: Moved combined modifier handling from device input routing to the core platform logic in `packages/platform-core/src/inputRouter.ts`
2. **Behavior Direct Invocation**: Changed from pushing device_input events to directly calling `behavior.onInput()` with the combined modifier input
3. **Simplified Release Logic**: Consolidated multiple release conditions into a single condition check

## Problem Solved

The original implementation was pushing `device_input` events which bypassed the behavior engine's proper input handling. The new approach ensures that the combined modifier behavior is properly invoked through the behavior engine's `onInput` method.

## Implementation Details

### Before (pushing events)
```typescript
// Send combined modifier press event (this represents the "third" modifier)
events.push({ type: "device_input", input: { type: "button_combined_modifier", pressed: true } });
nextState.system = { ...nextState.system, combinedModifierHeld: true };
```

### After (direct behavior invocation)
```typescript
// Directly call behavior.onInput() to trigger the combined modifier behavior
// This represents the "third" modifier
nextState.behaviorState = behavior.onInput(nextState.behaviorState, { type: "button_combined_modifier", pressed: true }, { bpm: nextState.transport.bpm, emit: (event) => events.push(event) });
nextState.system = { ...nextState.system, combinedModifierHeld: true };
```

## Testing Approach

The test file was updated to validate the behavior state transitions correctly rather than complex behavior state validation, focusing on the proper activation and deactivation of the combined modifier flag.

## Key Benefits

- Ensures proper behavior engine integration for combined modifiers
- Centralizes all input routing logic in platform-core as specified
- Simplifies release logic to a single condition
- Maintains hardware-like button behavior while following the centralized approach