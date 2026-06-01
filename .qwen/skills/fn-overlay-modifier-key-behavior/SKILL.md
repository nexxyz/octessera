---
name: FN overlay modifier key behavior
description: Centralized fix for FN overlay behavior when Shift+FN keys are pressed together
source: auto-skill
extracted_at: '2026-06-01T19:14:13.863Z'
---

When Shift+FN keys are pressed together, the FN overlay should not be displayed because this combination is treated as a system navigation command rather than a modifier key usage. The fix centralizes the overlay logic to check both `fnHeld` and `shiftHeld` states, suppressing the overlay when both keys are held together.

The implementation modifies the `runtimeHelpers.ts` file to replace direct `fnHeld` checks with centralized logic: `state.system.fnHeld && !state.system.shiftHeld` before displaying FN overlays in all touch modes (mix, pan, fx, and default cases).

This prevents two related issues:
1. The FN overlay incorrectly showing when Shift+FN are pressed together
2. The FN overlay "sticking" when FN is released while Shift is still held

The fix is centralized and not specific to trigger-gate mode, addressing the general FN navigation and modifier key behavior problem.

Key technical details:
- Modified `touchModeToLeds` function in `runtimeHelpers.ts` 
- Applied to all three touch modes (mix, pan, fx)
- Changed overlay display condition from `state.system.fnHeld` to `state.system.fnHeld && !state.system.shiftHeld`
- Added missing `filterTriggerGatedIntents` function to prevent build errors
- All changes are in a single file with no breaking changes to existing functionality