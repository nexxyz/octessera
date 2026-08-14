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
│   └── Load Empty: (action)              ← confirm, stop playback, load an empty patch while preserving device preferences
├── Recording (group)
│   ├── Max Time: [1..120] min  default 10
│   ├── Start Audio: (action)           ← Pi main-SD WAV of internal stereo output
│   └── Stop: (action)                  ← finalize active recording
├── Audio & USB (group)
│   ├── Jack Audio: [on | off]  default on             ← restart-applied
│   ├── USB Audio: [on | off]  default off             ← restart-applied
│   ├── HDMI Audio: [on | off]  default off            ← restart-applied
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
│   └── Output Buffer: [64 | 128 | 256 | 512 | 1024 | 2048] frames  default 256  ← CPAL/ALSA buffer; Orange engine block = output/4; restart required; OCTESSERA_AUDIO_OUTPUT_BUFFER_FRAMES wins
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
│   ├── Dim Timer: [0..600] step 10 s       default 60 (0=off; statically dims non-OLED LEDs with a small visible floor at low brightness)
│   ├── OLED Sleep: [0..600] step 10 s      default 60 (0=off; OLED only; Pi shows sparse ambient LED twinkle after the sleep splash)
│   ├── OLED Bright: [10..100] step 5     default 75 (bar display when Number Style is bar or bar+numbers)
│   ├── Grid Bright: [10..100] step 5     default 75 (bar display when Number Style is bar or bar+numbers)
│   └── Button Bright: [10..100] step 5   default 75 (bar display when Number Style is bar or bar+numbers)
├── Updates (group)
│   ├── Check: (action)               ← Pi checks matching board-profile release assets; desktop opens the releases page
│   ├── Apply: (action)               ← confirms, then Pi stages a candidate and schedules guarded health validation
│   └── Rollback: (action)            ← confirms, then Pi stages the previous release through the same guard
├── HDMI (group)
│   ├── Mode: [none | live-grid | plain-grid | active-behavior | cycle-behaviors]  default none
│   ├── Cycle Bars: [1..64] bars  default 4
│   └── Grid Lines: [on | off]  default off
├── Diagnostics (group)
│   └── Hardware Test: (action)       ← confirms, then runs pre-hardware Pi checks
├── Info: (action)                    ← opens native loading/system information popup
├── Configure WiFi: (action)          ← key system.configureWifi; confirms, stops/resets playback, then opens the typed setup portal effect
├── !Basic Help (action)              ← opens shortcut cheat-sheet help popup
├── Reboot: (action)                  ← confirm, then show shutdown splash and reboot
└── Shutdown: (action)                ← confirm, then show shutdown splash and exit/poweroff
```

Diagnostics is a pre-hardware Pi check. Update actions are native host effects: `Check` is unconfirmed, while `Apply` and `Rollback` confirm before a supported Raspberry Pi runs the system updater. `Apply` reports `Update health validation scheduled.` until the candidate passes process and stability checks; downloaded Apply candidates also require profile/version readiness. A failed validation automatically restores the previous release. Orange update check, apply, rollback, and OTA remain unsupported in 0.7.5. Legacy installations require provisioning or an OS-bundle update, or a reflash, before online apply; the legacy updater must not apply new releases. Desktop `Check` opens the GitHub releases page; desktop apply and rollback report unsupported. Load Empty lives under Saves, confirms with `Confirm Load Empty`, stops playback with MIDI panic/note safety, loads an empty musical patch state, and preserves device preferences such as brightness, MIDI setup, audio buffer, favourites, and preset names. Basic Help opens native help with the shortcut cheat sheet. Reboot and Shutdown stay at the bottom of System. `Stop/Sync: Sh+Space` follows the transport mode: internal sync emergency-stops and clears held notes, while external sync arms resync. `Fn+Space` is reset-stop: stop, reset position, and MIDI panic.

`System > Configure WiFi` is confirmed as `Open Portal`. The confirmation stops and resets playback, clears note state, and sends MIDI panic/all-notes-off cleanup; playback does not auto-resume. It then emits the typed setup portal effect. The modal reports `starting` while the hotspot starts, `portal_ready` with `Octessera Setup <4-char code>` and `192.168.42.1` for 30 minutes, `finalizing` while settings apply, `succeeded` without a reboot, `failed` with possible partial settings, `timed_out` when the portal closes, or desktop `unsupported`. The portal can change Wi-Fi, hostname, SSH, and the board's admin login (`pi` on Raspberry; `octessera` on Orange). Hide suppresses only the current phase and does not cancel; the next phase, including the terminal result, reopens the modal. Retry requires a new action. The setup modal takes priority over USB transfer, system info, and help.
