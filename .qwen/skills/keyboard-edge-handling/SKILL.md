---
name: Keyboard Edge Handling
description: Implementation of proper keyboard edge handling with pressedKeys tracking for hardware-like button behavior
source: auto-skill
extracted_at: '2026-06-01T21:20:34.222Z'
---

This skill covers the implementation of proper keyboard edge handling in the Cell Symphony application, specifically for simulating hardware-like button behavior using pressedKeys tracking.

## Key Changes

1. **Pressed Keys Tracking**: Implemented a `pressedKeys` Set to track which keys are currently pressed
2. **Edge-Only Handling**: Added logic to suppress repeated key events for specific keys to simulate hardware button behavior
3. **Hardware-like Behavior**: Keys like Shift, Control, Space, Enter, Backspace, Escape now behave like physical buttons with edge-only events

## Implementation Details

### Key Changes in `apps/desktop/src/ui/App.tsx`

- Added `const pressedKeys = new Set<string>();` to track pressed keys
- Implemented edge-only handling for specific keys using:
  ```typescript
  const edgeOnlyKeys = new Set(["Shift", "Control", " ", "Enter", "Backspace", "Escape"]);
  if (edgeOnlyKeys.has(event.key)) {
    if (pressedKeys.has(event.key) || event.repeat) return;
    pressedKeys.add(event.key);
  }
  ```
- Added cleanup for pressed keys when keys are released or window loses focus
- Maintained the original encoder-repeat behavior for arrow keys

## Problem Solved

Before this change, keyboard events would trigger repeated events for held keys, which doesn't match the behavior of physical hardware buttons. This caused issues with button press/release semantics in the simulator.

## Testing Approach

The implementation was verified by ensuring that:
- Keys that should behave like hardware buttons (Shift, Space, etc.) only trigger on press events
- Keys that should repeat (like arrow keys) maintain their browser-native repeat behavior
- No key state is leaked between different sessions or windows

## Key Benefits

- Hardware-like button behavior for keyboard input
- Proper suppression of repeated key events for physical button simulation
- Maintains compatibility with existing keyboard controls
- Provides a consistent user experience between physical and simulated hardware