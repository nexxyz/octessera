# System Menu Tree

This file is part of the canonical split-out menu tree spec. See [`../menu-tree-spec.md`](../menu-tree-spec.md) for the canonical index.

### System

```
System
├── Saves (group)
│   ├── Library (group)
│   │   ├── Save As (group)
│   │   │   ├── Name: (text, max 32 chars)  ← on exit/press: saves preset
│   │   │   └── Save: (action)
│   │   ├── Save Current: (action)    ← saves currently loaded preset (with confirm)
│   │   ├── Load (group)             ← dynamic: one action per preset
│   │   ├── Rename (group)           ← dynamic: one text+action per preset
│   │   ├── Delete (group)           ← dynamic: one action per preset
│   │   └── Refresh List: (action)
│   ├── Default (group)
│   │   ├── Save Default: (action)
│   │   ├── Load Default: (action)
│   │   ├── Auto Save: [on | off]    ← auto-persists settled config after cooldown
│   │   └── Backups: [on | off]      ← rolling safety backups, default on
│   ├── Factory (group)
│   │   └── Load Factory: (action)
├── Recording (group)
│   ├── Max Time: [1..120] min  default 10
│   ├── Start Audio: (action)           ← Pi main-SD WAV of internal stereo output
│   └── Stop: (action)                  ← finalize active recording
├── USB (group)
│   ├── Audio Out: [jack | usb | both]  default jack  ← restart-applied
│   ├── MIDI Out: [on | off]  default off             ← USB gadget exposure preference
│   ├── Save & Reboot: (action)         ← confirms with Cancel / Save & Reboot, saves payload, asks platform to apply and reboot
│   ├── Start SD2 Xfer: (action)        ← confirms, stops playback, blocks input in transfer popup, rejects active USB audio, USB MIDI out, or recording on Pi, temporarily exposes OLED SD2 as USB storage; waits cancellably if no host is connected
│   └── Stop SD2 Xfer: (action)         ← confirms host eject first, restores normal USB audio/MIDI gadget
├── Sound (group)                     ← merged: Audio + Sound controls
│   ├── Master Vol: [0..100] step 1  default 73
│   ├── Note Length: [30..2000] step 10 ms  default 120
│   ├── Velocity Scale: [0..200] step 5 %   default 100
│   ├── Velocity Curve: [linear | soft | hard]
│   ├── Voice Limit: [fixed12 | fixed16 | auto-soft | auto-balanced | auto-hard | none]  default auto-balanced
│   └── Output Buffer: [64 | 128 | 256 | 512 | 1024 | 2048] frames  default 256  ← restart required; env override still wins
├── MIDI (group)
│   ├── Enabled: [on | off]
│   ├── !Panic: (action)
│   ├── MIDI Out (group)             ← dynamic: one action per detected MIDI output port
│   ├── MIDI In (group)              ← dynamic: one action per detected MIDI input port
│   ├── Sync & Clock (group)
│   │   ├── Sync Mode: [internal | external]
│   │   ├── Clock Out: [on | off]
│   │   ├── Clock In: [on | off]
│   │   └── Follow S/S: [on | off]
├── UI (group)
│   ├── Ghost Cells: [on | off]  default off  ← shows dim cells from inactive layers behind active layer
│   ├── Auto Map: [on | off]  default on  ← enables context-sensitive aux mappings
│   ├── Number Style: [bar | numbers | bar+numbers]  ← controls rendering of bar-style numeric params, default bar+numbers
│   ├── Dim Timer: [0..600] step 10 s       default 60 (0=off; dims non-OLED LEDs)
│   ├── OLED Sleep: [0..600] step 10 s      default 60 (0=off; OLED only)
│   ├── OLED Bright: [10..100] step 5     default 75 (bar display when Number Style is bar or bar+numbers)
│   ├── Grid Bright: [10..100] step 5     default 75 (bar display when Number Style is bar or bar+numbers)
│   └── Button Bright: [10..100] step 5   default 75 (bar display when Number Style is bar or bar+numbers)
├── Updates (group)
│   ├── Check: (action)               ← placeholder update status request; no updater is wired yet
│   ├── Apply: (action)               ← confirms, then calls a placeholder host effect
│   └── Rollback: (action)            ← confirms, then calls a placeholder host effect
├── Diagnostics (group)
│   └── Hardware Test: (action)       ← confirms, then runs pre-hardware Pi checks
├── !Basic Help (action)              ← opens shortcut cheat-sheet help popup
├── Reboot: (action)                  ← confirm, then show shutdown splash and reboot
├── Shutdown: (action)                ← confirm, then show shutdown splash and exit/poweroff
└── Clear all: (action)               ← confirm, stop playback, clear patch state while preserving device preferences
```

Diagnostics is a pre-hardware Pi check, and the update actions are native placeholders for OTA flow control: `Check` is unconfirmed, while `Apply` and `Rollback` confirm before calling placeholder Pi/desktop host effects. Clear all confirms with `Confirm Clear All`, stops playback with MIDI panic/note safety, clears musical patch state, and preserves device preferences such as brightness, MIDI setup, audio buffer, favourites, and preset names. Basic Help opens native help with the shortcut cheat sheet. `Stop/Sync: Sh+Space` follows the transport mode: internal sync emergency-stops and clears held notes, while external sync arms resync. `Fn+Space` is reset-stop: stop, reset position, and MIDI panic.
