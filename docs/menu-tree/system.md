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
├── Audio / USB (group)
│   ├── Jack Audio: [on | off]  default on             ← restart-applied
│   ├── USB Audio: [on | off]  default off             ← restart-applied
│   ├── HDMI Audio: [on | off]  default off            ← restart-applied
│   ├── MIDI Out: [on | off]  default off             ← USB gadget exposure preference
│   ├── Save / Reboot: (action)         ← confirms with Cancel or Save / Reboot, saves payload, asks platform to apply and reboot
│   ├── Start SD2 Xfer: (action)        ← confirms, stops playback, blocks input in transfer popup, rejects active USB audio, USB MIDI out, or recording on Pi, temporarily exposes OLED SD2 as USB storage; waits cancellably if no host is connected
│   └── Stop SD2 Xfer: (action)         ← confirms host eject first, restores normal USB audio/MIDI gadget
├── Sound (group)                     ← merged: Audio + Sound controls
│   ├── Master Vol: [0..100] step 1  default 73
│   ├── Note Length: [30..2000] step 10 ms  default 120
│   ├── Velocity Scale: [0..200] step 5 %   default 100
│   ├── Velocity Curve: [linear | soft | hard]
│   ├── Voice Limit: [fixed12 | fixed16 | auto-soft | auto-balanced | auto-hard | none]  default auto-balanced
│   └── Output Buffer: [64 | 128 | 256 | 512 | 1024 | 2048] frames  default 256  ← CPAL/ALSA buffer; render quantum capability default 128; restart required; OCTESSERA_AUDIO_OUTPUT_BUFFER_FRAMES wins
├── MIDI (group)
│   ├── Enabled: [on | off]
│   ├── !Panic: (action)
│   ├── MIDI Out (group)             ← dynamic: one action per detected MIDI output port
│   ├── MIDI In (group)              ← dynamic: one action per detected MIDI input port
│   ├── Sync / Clock (group)
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
│   ├── Check: (action)               ← Pi checks matching board-profile release assets; Orange checks its runtime-updater pair; desktop opens the releases page
│   ├── Apply: (action)               ← confirms, then Pi/Orange stages a runtime candidate and schedules guarded health validation
│   └── Rollback: (action)            ← confirms, then Pi/Orange stages the previous runtime release through the same guard
├── HDMI (group)
│   ├── Mode: [Terminal | live-grid | plain-grid | active-behavior | cycle-behaviors]  default Terminal (stored none)
│   ├── Bars per cycle: [1..64] bars  default 4 (cycle-behaviors only)
│   └── Grid Lines: [on | off]  default off
├── Diagnostics (group)
│   └── Hardware Test: (action)       ← confirms, then runs pre-hardware Pi checks
├── Info: (action)                    ← opens native loading/system information popup
├── Configure WiFi: (action)          ← key system.configureWifi; confirms, stops/resets playback, then opens the typed setup portal effect
├── Backup / Restore: (action)        ← key system.backupRestore; direct Pi transfer service, no confirmation
├── !Basic Help (action)              ← opens shortcut cheat-sheet help popup
├── Reboot: (action)                  ← confirm, then show shutdown splash and reboot
└── Shutdown: (action)                ← confirm, then show shutdown splash and exit/poweroff
```

Diagnostics is a pre-hardware Pi check. Update actions are native host effects: `Check` is unconfirmed, while `Apply` and `Rollback` confirm before a supported Raspberry Pi or Orange host runs its profile-qualified updater. Orange uses only `octessera-<version>-orange-pi-zero-2w-runtime-updater-aarch64.zip` with `SHA256SUMS-orange-pi-zero-2w-runtime-updater.txt`; these actions update the runtime release only. Full Armbian, kernel, device-tree, and image replacement remains manual, and the standalone manual runtime ZIP is not an OTA asset. Profile, asset, manifest, checksum, and guard failures fail closed without a Raspberry or manual/image fallback. `Apply` reports `Update health validation scheduled.` until the candidate passes process and stability checks; downloaded Apply candidates also require profile/version readiness. A failed validation automatically restores the previous release. Legacy installations require provisioning or an OS-bundle update, or a reflash, before online apply; the legacy updater must not apply new releases. Desktop `Check` opens the GitHub releases page; desktop apply and rollback report unsupported. Load Empty lives under Saves, confirms with `Confirm Load Empty`, stops playback with MIDI panic/note safety, loads an empty musical patch state, and preserves device preferences such as brightness, MIDI setup, audio buffer, favourites, and preset names. Basic Help opens native help with the shortcut cheat sheet. Reboot and Shutdown stay at the bottom of System. `Stop/Sync: Sh+Space` follows the transport mode: internal sync emergency-stops and clears held notes, while external sync arms resync. `Fn+Space` is reset-stop: stop, reset position, and MIDI panic.

`System > Configure WiFi` is confirmed as `Open Portal`. The confirmation stops and resets playback, clears note state, and sends MIDI panic/all-notes-off cleanup; playback does not auto-resume. It then emits the typed setup portal effect. The modal reports `starting`, `portal_ready` with `Octessera Setup <4-char code>` and `192.168.42.1` for 10 minutes, `finalizing`, `succeeded`, `failed`, `timed_out`, or desktop `unsupported`. Browser Applying is provisional and an AP disconnect is expected; the OLED result is authoritative. Success needs only a usable global `wlan0` IPv4 address, not Internet access, a default route, DNS, or ICMP. Success and timeout cards auto-hide, failure remains dismissible, and a new `Open Portal` action retries. The portal can change Wi-Fi, hostname, SSH, and the board's admin login (`pi` on Raspberry; `octessera` on Orange), with no reboot. Configure WiFi does not start or advertise Backup / Restore. The setup modal takes priority over system info and help.

`System > Backup / Restore` is a direct, unconfirmed action with stable key `system.backupRestore`. On Pi it uses the existing authenticated service at `http://<regular-ip>:8081`, selected from usable regular `wlan0` IPv4, with a generated 10-character code and 15-minute lifetime. The OLED card shows IP, port, code, expiry, and `> Stop service`; Back hides it while the service continues, and Stop closes it and revokes the code. Desktop is unsupported. This action is separate from rolling `System > Saves > Default > Backups`.

`System > HDMI > Mode` displays `Terminal` for the canonical stored/runtime value `none`. Terminal releases and disables Octessera framebuffer output so Linux terminal ownership can show, while snapshots retain the black/inactive HDMI grid for compatibility. `Bars per cycle` appears only for `cycle-behaviors` and sets how many musical bars each behavior remains shown before Cycle Behaviors advances. This menu contract does not claim live Orange HDMI signal qualification.
