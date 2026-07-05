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
│   │   └── Auto Save: [on | off]    ← auto-persists settled config after cooldown
│   ├── Factory (group)
│   │   └── Load Factory: (action)
├── Diagnostics (group)
│   └── Hardware Test: (action)       ← confirms, then runs pre-hardware Pi checks
├── Updates (group)
│   ├── Check: (action)               ← read-only update status request
│   ├── Apply: (action)               ← confirms before applying the staged update
│   └── Rollback: (action)            ← confirms before switching back to the previous release
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
│   │   └── Follow Start/Stop: [on | off]
├── UI (group)
│   ├── Ghost Cells: [on | off]  default off  ← shows dim cells from inactive parts behind active part
│   ├── Auto Map: [on | off]  default on  ← enables context-sensitive aux mappings
│   ├── Number Style: [bar | numbers | bar+numbers]  ← controls rendering of bar-style numeric params, default bar+numbers
│   ├── Screen Sleep: [0..600] step 10 s    default 60 (0=off)
│   ├── OLED Bright: [10..100] step 5     default 75 (bar display when Number Style is bar or bar+numbers)
│   ├── Grid Bright: [10..100] step 5     default 75 (bar display when Number Style is bar or bar+numbers)
│   └── Button Bright: [10..100] step 5   default 75 (bar display when Number Style is bar or bar+numbers)
├── !Basic Help (action)              ← opens shortcut cheat-sheet help popup
├── Reboot: (action)                  ← confirm, then show shutdown splash and reboot
└── Shutdown: (action)                ← confirm, then show shutdown splash and exit/poweroff
```

Diagnostics is a pre-hardware Pi check, and the update actions are native placeholders for OTA flow control: `Check` is unconfirmed, while `Apply` and `Rollback` confirm before handing off to the Pi host adapter. Basic Help opens native help with the shortcut cheat sheet. `Stop/Sync: Sh+Space` follows the transport mode: internal sync emergency-stops and clears held notes, while external sync arms resync.
