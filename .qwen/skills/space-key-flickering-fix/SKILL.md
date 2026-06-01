---
name: Space key flickering fix for Windows
description: Fix Windows-specific Space key flickering by treating Space key as binary button state with proper state tracking
source: auto-skill
extracted_at: '2026-06-01T20:33:52.687Z'
---

## Problem
Windows systems generate rapid press/release events for the Space key, causing flickering behavior where the key appears to toggle rapidly between pressed and released states. This was exacerbated by previous attempts to fix it with overly aggressive duplicate key detection that broke all other key functionality.

## Solution Approach
Implemented a robust fix that treats the Space key as a simple binary button state (0 or 1) rather than a key press event:

1. Added `spacePressed` boolean state tracking to properly manage Space key state
2. Added `lastSpacePressTime` for debounce timing to prevent rapid toggling
3. Applied state-aware handling that prevents rapid press events when key is already pressed
4. Properly handle both key press and release events with correct state management

## Key Implementation Details
- Space key is now treated as a true button with binary state (pressed/released)
- Only Space key behavior is modified, preserving all other keys exactly as before
- Key press events track state and debounce when already pressed
- Key release events properly reset the state
- Maintains existing keyboard adapter structure and patterns

## Why This Approach
This solution addresses the core issue where Windows sends duplicate events for Space key:
1. It properly handles the binary button state instead of treating each event separately
2. Prevents rapid toggling that causes flickering
3. Doesn't break other key functionality like Shift, Ctrl, etc.
4. Is more robust than simple debounce approaches
5. Follows the principle of making minimal, focused changes to fix platform-specific issues

## Result
- Space key flickering issue is resolved on Windows
- All other keys (Shift, Ctrl, Arrow keys, etc.) work normally
- No regression in existing functionality
- Clean, maintainable solution that follows project conventions
- Properly handles both key press and release events for Space key