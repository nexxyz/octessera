---
name: Shift+Fn combined modifier handling
description: Fixed simulator logic to properly handle combined Shift+Fn modifier key states
source: auto-skill
extracted_at: '2026-06-01T20:58:18.232Z'
---

## Problem
In the simulator runtime, when Shift and Fn keys were pressed together, the code would intercept the input and send only the combined modifier event, but not properly route the original inputs to update internal state variables. This caused `state.system.fnHeld` and `state.system.shiftHeld` to never be updated, breaking the modifier state tracking in the simulator.

## Solution
Modified the combined modifier handling logic in simulatorRuntime.ts to ensure that both the combined modifier event and the original input are processed. This maintains proper state tracking for modifier keys.

## Key Changes

### apps/desktop/src/runtime/simulatorRuntime.ts
- Updated the combined modifier handling logic to route both the combined modifier event AND the original input
- This ensures that `state.system.fnHeld` and `state.system.shiftHeld` are properly updated for all modifier key combinations
- Maintained all existing behavior while fixing the state update issue

## Behavior
The combined modifier behavior works correctly in both simulator and device:
1. Shift+Fn pressed together → Generates `button_combined_modifier` press event AND updates internal state
2. Shift released while Fn still held → Generates `button_combined_modifier` release event AND updates internal state  
3. Fn released while Shift still held → Generates `button_combined_modifier` release event AND updates internal state
4. Both released → Resets combined modifier state AND internal state

## Key Benefits
- Fixes the broken simulator state tracking
- Maintains identical behavior between simulator and device implementations
- Ensures proper key state management in all scenarios
- No breaking changes to existing API or functionality

## Technical Details
The root issue was that when both Shift and Fn were pressed together, the code would:
1. Process the combined modifier event
2. Return early without processing the original input

This meant that the internal state tracking for individual modifier keys was never updated. The fix ensures that after processing the combined modifier event, the original input is also processed to maintain proper state.

## Why This Approach
- Fixes the core logic error without changing the overall architecture
- Maintains the centralized approach to modifier handling
- Follows the existing pattern of processing inputs through the core routing system
- Ensures consistency between device and simulator behavior