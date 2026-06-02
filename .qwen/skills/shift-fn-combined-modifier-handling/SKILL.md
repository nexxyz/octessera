---
name: shift-fn-combined-modifier-handling
description: Centralized combined modifier handling for Shift+Fn keys across device and simulator implementations, hardware keyboard button behavior with arrow key repeats
source: auto-skill
extracted_at: '2026-06-01T21:10:13.736Z'
---

This skill describes the implementation of centralized combined modifier handling for Shift+Fn keys in the Cell Symphony project.

## Key Changes Made

1. **Centralized Logic**: Removed simulator-specific combined modifier handling and centralized the logic in `packages/platform-core/src/inputRouter.ts` 
2. **Hardware Keyboard Behavior**: Made keyboard inputs behave like hardware buttons in `apps/desktop/src/ui/App.tsx` with proper key repeat support for arrow keys
3. **Field Renaming**: Renamed `thirdModifierHeld` to `combinedModifierHeld` across the codebase (field was already present)
4. **Test Coverage**: Added focused tests in `packages/platform-core/tests/combinedModifier.test.ts` covering all combined modifier scenarios
5. **Fake Test Removal**: Removed `test_combined_modifier_implementation.ts` which was a log-only test file

## Technical Details

### Field Renaming
The field `thirdModifierHeld` was renamed to `combinedModifierHeld` across the codebase to improve clarity:
- `packages/platform-core/src/initialState.ts` - Field already present, no change needed
- `packages/platform-core/src/platformTypes.ts` - Updated field name
- `syntax_check.ts` - Updated field reference
- `simple_syntax_check.ts` - Updated field reference
- `test_combined_modifier_simple.ts` - Updated field reference
- `verify_combined_modifier.ts` - Updated field reference

The new name `combinedModifierHeld` more accurately describes the field's purpose:
- It represents a combined modifier state (Shift + Fn)
- It's clearer than the generic "third modifier"
- It aligns with the project's naming conventions and intent

## Technical Details

### Centralized Combined Modifier Logic
The core router in `inputRouter.ts` now handles:
- When both Shift and Fn are pressed together, `combinedModifierHeld` is set to true
- When either key is released, `combinedModifierHeld` is reset to false
- This ensures consistent behavior across hardware and simulator

### Keyboard Hardware Button Behavior
In `App.tsx`, keyboard inputs now:
- Track pressed keys and handle repeats for arrow keys like encoder turns
- Clear held keys properly on keyup and window blur
- Maintain consistency with hardware button behavior

### Input Adapter Updates
In `keyboardAdapter.ts`:
- Backspace and Escape both trigger `button_a` for consistent hardware behavior
- Arrow keys now properly trigger encoder turns

## Verification
All changes verified through:
- Added comprehensive tests in `combinedModifier.test.ts`
- Manual verification in desktop simulator
- Type checking of modified files