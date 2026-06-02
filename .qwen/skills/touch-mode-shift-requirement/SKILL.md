---
name: touch-mode-shift-requirement
description: Modified touch mode change logic to require both Fn and Shift keys for rightmost column navigation
source: auto-skill
extracted_at: '2026-06-01T22:19:39.855Z'
---

## Problem
The touch mode change behavior was previously enabled by pressing Fn + Rightmost column. Users requested that this be changed to require both Fn and Shift keys to be held simultaneously while pressing the rightmost column, making the interaction more deliberate.

## Solution
Modified the condition in `packages/platform-core/src/inputRouter.ts` to change the touch mode change requirement from:
- `fnHeld && !shiftHeld` 
to:
- `fnHeld && shiftHeld`

This ensures that users must hold both Fn and Shift keys simultaneously when pressing the rightmost column to trigger touch mode changes.

## Key Changes
- File: `packages/platform-core/src/inputRouter.ts`
- Line: 231
- Condition changed from `!nextState.system.shiftHeld` to `nextState.system.shiftHeld`

## Result
Touch mode change now requires Fn + Shift + Rightmost column instead of just Fn + Rightmost column, providing a more deliberate interaction pattern.