---
name: Combined modifier key handling
description: Platform-independent handling of Shift+Fn key combinations as single combined modifier input
source: auto-skill
extracted_at: '2026-06-01T19:45:53.665Z'
---

When Shift and Fn keys are pressed together, they should be treated as a single combined modifier input rather than two separate modifier events. This approach centralizes the logic in platform-core rather than in platform-specific adapters.

The implementation:
1. Added `button_combined_modifier` type to DeviceInput union in device-contracts
2. Implemented detection logic in platform-core's inputRouter.ts to identify when Shift+Fn are pressed together
3. Routes the combination as a single `button_combined_modifier` input instead of separate events
4. Added debouncing mechanism in keyboard adapter to prevent rapid toggling issues
5. Updated keyboard adapter tests to verify Shift+Fn combinations are not processed by keyboard adapter

Key benefits:
- Platform-independent solution (centralized in platform-core)
- Maintains hardware/software parity
- Avoids desktop-only logic that bypasses platform-core
- Proper state management for modifier combinations
- Prevents rapid toggling issues with rapid key presses
- No breaking changes to existing functionality

This approach was necessary because the original keyboard adapter approach was deemed platform-specific and not suitable for the hardware abstraction layer.

The solution specifically addresses:
- When Shift+Fn are pressed together, a `button_combined_modifier` press event is generated
- When either Shift or Fn is released, a `button_combined_modifier` release event is generated
- The system tracks state with a `thirdModifierHeld` flag to properly manage the combined modifier state
- Debouncing prevents rapid toggling issues when keys are pressed/released quickly