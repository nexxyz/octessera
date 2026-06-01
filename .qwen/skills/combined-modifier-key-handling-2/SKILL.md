---
name: combined-modifier-key-handling-2
description: Handles Shift+Fn key combinations as a single combined modifier input instead of separate modifier events, with debouncing to prevent toggling issues and Windows key event robustness
source: auto-skill
extracted_at: '2026-06-01T20:01:26.171Z'
---

## Problem
When Shift and Fn keys are pressed together, they should be treated as a single combined modifier input rather than two separate modifier events. Rapid key presses were causing toggling issues, and Windows-specific key event behavior was causing Space key flickering.

## Solution
Implemented platform-independent handling in `platform-core` to:
1. Detect Shift+Fn combinations as a single "button_combined_modifier" event
2. Handle proper press and release events for the combined modifier
3. Add debouncing to prevent rapid toggling issues
4. Implement duplicate key tracking to prevent Windows-specific key event problems

## Key Changes

### platform-core/src/inputRouter.ts
- Added logic to detect when both Shift and Fn are held together
- Sends single "button_combined_modifier" event instead of two separate events
- Properly handles release events when either modifier is released
- Uses existing `thirdModifierHeld` state flag to prevent duplicate events

### apps/desktop/src/runtime/inputAdapters/keyboardAdapter.ts
- Prevents direct generation of combined modifier events from keyboard input
- Added debouncing mechanism to prevent toggling issues with rapid key releases
- Implemented duplicate key tracking to prevent Windows-specific key event problems
- Enhanced Space key handling to prevent flickering issues
- Delegates combined modifier handling to platform-core

### apps/desktop/tests/keyboardAdapter.test.ts
- Added test for debouncing rapid key releases
- Added test for Space key press and release behavior

## Behavior
1. Shift+Fn pressed together → Single "button_combined_modifier" press event
2. Shift released while Fn still held → "button_combined_modifier" release event
3. Fn released while Shift still held → "button_combined_modifier" release event
4. Rapid key releases are debounced to prevent toggling issues
5. Duplicate key events are filtered out to prevent Windows-specific problems
6. All other modifier behavior remains unchanged

## Testing
The implementation was tested by:
1. Verifying proper event generation for combined modifiers
2. Confirming debouncing prevents rapid toggling
3. Ensuring duplicate key filtering works correctly
4. Testing Space key behavior to prevent flickering
5. Ensuring existing functionality is not broken