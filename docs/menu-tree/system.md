# System Menu Tree

This file is part of the canonical split-out menu tree spec. See [`../menu-tree-spec.md`](../menu-tree-spec.md) for the canonical index.

### System

```
System
├── Save Current: (action)              ← direct root shortcut
├── Load Preset (group)                 ← direct root shortcut
│   ├── (none): (action)                ← refreshes the preset list
│   └── <preset>: (action)              ← confirm, then load preset
├── Master Vol: [0..100] step 1
├── !Panic: (action)
├── !Sys. Info: (action)
├── !Basic Help: (action)
├── Recording (group)
│   ├── Max Time: [1..120] min  default 10
│   ├── Start Audio: (action)           ← final internal stereo output
│   ├── St. Audio+OLED: (action)        ← synchronized audio and accepted OLED frames
│   └── Stop: (action)                  ← finalize active recording
├── Notes (group)
│   ├── Note Length: [30..2000] step 10 ms  default 120
│   ├── Vel Scale: [0..200] step 5 %   default 100
│   ├── Vel Curve: [linear | soft | hard]
├── MIDI (group)
│   ├── Enabled: [on | off]
│   ├── USB MIDI: [on | off]          ← board-only; bidirectional class device; restart-applied
│   ├── MIDI Out (group)              ← dynamic: one action per detected MIDI output port
│   ├── MIDI In (group)               ← dynamic: one action per detected MIDI input port
│   ├── Sync / Clock (group)
│   │   ├── Sync Mode: [internal | external]
│   │   ├── Clock Out: [on | off]
│   │   ├── Clock In: [on | off]
│   │   └── Follow S/S: [on | off]
├── Audio (group)
│   ├── USB Audio: [on | off]  default off             ← Pi only; optional Jack-mix mirror; restart-applied
│   ├── HDMI Audio: [on | off]  default off            ← Pi only; optional Jack-mix mirror; visible in Host; restart-applied
│   ├── Perf. Mode: [Lat | Cap]  default Lat  ← full choices Latency/Capacity; Raspberry and Orange; restart-sensitive
│   ├── Polyphony: [fixed12 | fixed16 | auto-soft | auto-balanced | auto-hard | none]  default auto-balanced
│   └── Engine (group)
│       ├── CPU Warn %: [70 | 75 | 80 | 85 | 90 | 95]  default 85
│       ├── Bus Idle: [exact | -140 | -120 | -100 | -80]  default -120
│       └── Buf Frames: [64 | 128 | 256 | 512 | 1024 | 2048]  ← shown where Perf. Mode is unavailable; restart-sensitive
├── USB Role: [Gadget | Host]            ← Raspberry capability only; restart-sensitive
├── SD Card 2 (group)                  ← board-only
│   ├── Start Transfer: (action)
│   └── Stop Transfer: (action)
├── UI (group)
│   ├── Ghost Cells: [on | off]  default off  ← shows dim cells from inactive layers behind active layer
│   ├── Auto Map: [on | off]  default on  ← enables context-sensitive aux mappings
│   ├── Number Style: [bar | numbers | bar+numbers]  ← controls rendering of bar-style numeric params, default bar+numbers
│   ├── Dim Timer: [0..600] step 10 s       default 60 (0=off; statically dims non-OLED LEDs with a small visible floor at low brightness)
│   ├── OLED Sleep: [0..600] step 10 s      default 60 (0=off; OLED only; Pi shows sparse ambient LED twinkle after the sleep splash)
│   ├── OLED Bright: [10..100] step 5     default 75 (bar display when Number Style is bar or bar+numbers)
│   ├── Grid Bright: [10..100] step 5     default 75 (bar display when Number Style is bar or bar+numbers)
│   └── Button Bright: [10..100] step 5   default 75 (bar display when Number Style is bar or bar+numbers)
├── HDMI Video (group)                  ← board-only
│   ├── Mode: [Terminal | live-grid | plain-grid | active-behavior | cycle-behaviors]  default Terminal (stored none)
│   ├── Bars per cycle: [1..64] bars  default 4 (cycle-behaviors only)
│   └── Grid Lines: [on | off]  default off
├── Saves (group)
│   ├── Library (group)
│   │   ├── Save As (group)
│   │   │   ├── Name: (text, max 32 chars)
│   │   │   └── Save: (action)
│   │   ├── Save Current: (action)
│   │   ├── Load (group)             ← dynamic: one action per preset
│   │   ├── Rename (group)           ← dynamic: one text+action per preset
│   │   ├── Delete (group)           ← dynamic: one action per preset
│   │   └── Refresh List: (action)
│   └── Default (group)
│       ├── Save Default: (action)
│       ├── Load Default: (action)
│       ├── Auto Save: [on | off]
│       └── Backups: [on | off]
├── Setup (group)
│   ├── Configure WiFi: (action)
│   ├── Backup / Restore: (action)
│   ├── Updates (group)
│   │   ├── Check: (action)
│   │   ├── Apply: (action)
│   │   └── Rollback: (action)
│   └── Hardware Test: (action)
├── Reset (group)
│   ├── Load Empty: (action)
│   └── Load Factory: (action)
├── Reboot: (action)                  ← confirm, then show shutdown splash and reboot
└── Shutdown: (action)                ← confirm, then show shutdown splash and exit/poweroff
```

`Setup` combines the existing Wi-Fi, Backup / Restore, Updates, and Hardware Test actions without changing their action keys. `Check` is unconfirmed; `Apply` and `Rollback` retain their existing confirmation and updater behavior. `Reset > Load Empty` and `Reset > Load Factory` retain their existing confirmation and native reset behavior. Desktop and Orange omit the Raspberry-only USB Role row; desktop also omits the board-only USB MIDI, SD Card 2, and HDMI Video groups and all Jack/USB/HDMI/SD2 device controls. Basic Help opens native help with the shortcut cheat sheet. Reboot and Shutdown stay at the bottom of System. `Stop/Sync: Sh+Space` follows the transport mode: internal sync stops/resets, silences internal audio, and performs bounded held-note cleanup without broad MIDI panic, while external sync arms resync. `Fn+Space` is reset-stop with the same bounded cleanup.

`System > MIDI > Enabled` is the runtime MIDI gate and selects no port or device. `USB MIDI` is visible only in Gadget mode, enables the bidirectional USB MIDI class device after restart, auto-routes outbound gadget MIDI, and leaves inbound selection under `MIDI In`. `System > USB Role` is Raspberry-only; Host hides and disables USB Audio, USB MIDI, and SD Card 2 Start Transfer, while HDMI Audio and Stop Transfer remain available for cleanup. Gadget does not restore previously disabled outputs. `System > SD Card 2` exposes the second card to a USB host; conflicting USB audio, MIDI, and recording must be inactive. Start Transfer and Stop Transfer retain their existing confirmation, transfer, eject, and cancel semantics.

`System > Sys. Info` opens a native loading popup and requests asynchronously identified, sanitized OS/version, Octessera version, primary IP/MAC when available, hostname, and explicit board profile information. The desktop UI only renders the resulting native snapshot.

`System > Recording` uses one `Max Time` setting for both formats. `Start Audio` writes the final internal stereo mix as WAV; `St. Audio+OLED` writes synchronized 128x128 MJPG/PCM AVI with audio-master timing at 10 fps; `Stop` finalizes either active format. Recording roots and partial/incomplete lifecycle are defined in the authoritative control and runtime-boundary specs.

`System > Reboot` and `System > Shutdown` retain their native confirmation and recovery-save flow. The runtime stops follow-ups, independently attempts external MIDI panic and internal audio silence, acknowledges the final shutdown/reboot snapshot, and then submits the fixed board-specific power request. The ordinary-menu toasts remain `Rebooting` and `Shutting down`.

`System > Setup > Configure WiFi` is confirmed as `Open Portal`. The confirmation stops and resets playback, clears note state, and sends MIDI panic/all-notes-off cleanup; playback does not auto-resume. It then emits the typed setup portal effect. The modal reports `starting`, `portal_ready` with `Octessera Setup <4-char code>` and `192.168.42.1` for 10 minutes, `finalizing`, `succeeded`, `failed`, `timed_out`, or desktop `unsupported`. Browser Applying is provisional and an AP disconnect is expected; the OLED result is authoritative. Success needs only a usable global `wlan0` IPv4 address, not Internet access, a default route, DNS, or ICMP. Success and timeout cards auto-hide, failure remains dismissible, and a new `Open Portal` action retries. The portal can change Wi-Fi, hostname, SSH, and the board's admin login (`pi` on Raspberry; `octessera` on Orange), with no reboot. Configure WiFi does not start or advertise Backup / Restore. The setup modal takes priority over system info and help.

`System > Setup > Backup / Restore` is a direct, unconfirmed action with stable key `system.backupRestore`. On Pi it uses the existing authenticated service at `http://<regular-ip>:8081`, selected from usable regular `wlan0` IPv4, with a generated 10-character code and 15-minute lifetime. The OLED card shows IP, port, code, expiry, and `> Stop service`; Back hides it while the service continues, and Stop closes it and revokes the code. Desktop is unsupported. This action is separate from rolling `System > Saves > Default > Backups`.

`System > HDMI Video > Mode` displays `Terminal` for the canonical stored/runtime value `none`. Terminal releases and disables Octessera framebuffer output so Linux terminal ownership can show, while snapshots retain the black/inactive HDMI grid for compatibility. `Bars per cycle` appears only for `cycle-behaviors` and sets how many musical bars each behavior remains shown before Cycle Behaviors advances.
