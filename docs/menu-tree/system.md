# System Menu Tree

This file is part of the canonical split-out menu tree spec. See [`../menu-tree-spec.md`](../menu-tree-spec.md) for the canonical index.

### System

```
System
├── Load Preset (group)                 ← direct root shortcut
│   ├── (none): (action)                ← refreshes the preset list
│   └── <preset>: (action)              ← confirm, then load preset
├── Recording (group)
│   ├── Max Time: [1..120] min  default 10
│   ├── Start Audio: (action)           ← final internal stereo output
│   ├── St. Audio+OLED: (action)        ← synchronized audio and accepted OLED frames
│   └── Stop: (action)                  ← finalize active recording
├── Notes (group)
│   ├── Note Length: [30..2000] step 10 ms  default 120
│   ├── Vel Scale: [0..200] step 5 %   default 100
│   └── Vel Curve: [linear | soft | hard]
├── MIDI (group)
│   ├── MIDI Active: [on | off]
│   ├── MIDI Host (group)                ← desktop, Orange, and Raspberry Host
│   │   ├── MIDI Out (group)             ← dynamic: one action per detected output port
│   │   └── MIDI In (group)              ← dynamic: one action per detected input port
│   ├── USB Device (group)               ← Orange and Raspberry Gadget
│   │   └── USB MIDI: [on | off]         ← owns both Gadget MIDI directions; restart-applied
│   └── Sync / Clock (group)
│       ├── Sync: [internal | external]
│       ├── Clock Out: [on | off]
│       ├── Clock In: [on | off]
│       └── Follow S/S: [on | off]
├── Audio (group)
│   ├── USB Audio: [on | off]            ← Raspberry and Orange; hidden in Raspberry Host
│   ├── HDMI Audio: [on | off]           ← Raspberry and Orange
│   ├── Master Vol: [0..100] step 1
│   ├── Perf. Mode: [Latency | Capacity] ← Raspberry and Orange; restart-sensitive
│   ├── Polyphony: [fixed12 | fixed16 | auto-soft | auto-balanced | auto-hard | none]
│   └── Engine (group)
│       ├── CPU Warn %: [70 | 75 | 80 | 85 | 90 | 95]
│       ├── Bus Idle: [exact | -140 | -120 | -100 | -80]
│       └── Buf Frames: [64 | 128 | 256 | 512 | 1024 | 2048] ← desktop only
├── UI (group)
│   ├── Ghost Cells: [on | off]
│   ├── Auto Map: [on | off]
│   ├── Number Style: [bar | numbers | bar+numbers]
│   ├── Dim Timer: [0..600] step 10 s
│   ├── OLED Sleep: [0..600] step 10 s
│   ├── OLED Bright: [10..100] step 5
│   ├── Grid Bright: [10..100] step 5
│   └── Button Bright: [10..100] step 5
├── SD Card 2 (group)                    ← Raspberry and Orange
│   ├── Start Transfer: (action)         ← hidden in Raspberry Host
│   └── Stop Transfer: (action)
├── HDMI Video (group)                   ← Raspberry and Orange
│   ├── Mode: [Terminal | live-grid | plain-grid | active-behavior | cycle-behaviors]
│   ├── Bars per cycle: [1..64] bars    ← cycle-behaviors only
│   └── Grid Lines: [on | off]
├── Saves (group)
│   ├── Library (group)
│   │   ├── Save As (group)
│   │   │   ├── Name: (text, max 32 chars)
│   │   │   └── Save: (action)
│   │   ├── Load (group)                 ← dynamic: one action per preset
│   │   ├── Rename (group)               ← dynamic: one text+action per preset
│   │   ├── Delete (group)               ← dynamic: one action per preset
│   │   ├── Save Current: (action)
│   │   └── Refresh List: (action)
│   └── Default (group)
│       ├── Auto Save: [on | off]
│       ├── Backups: [on | off]
│       ├── Save Default: (action)
│       └── Load Default: (action)
├── Setup (group)
│   ├── USB Role: [Gadget | Host]         ← Raspberry capability only
│   ├── Updates (group)
│   │   ├── Check: (action)
│   │   ├── Apply: (action)
│   │   └── Rollback: (action)
│   ├── Configure WiFi: (action)
│   ├── Backup / Restore: (action)
│   └── Hardware Test: (action)
├── Reset (group)
│   ├── Load Empty: (action)
│   └── Load Factory: (action)
├── Panic: (action)
├── Sys. Info: (action)
├── Basic Help: (action)
├── Reboot: (action)
└── Shutdown: (action)
```

The exact System children are the following after conditional rows are resolved:

```
Desktop:
Load Preset, Recording, Notes, MIDI, Audio, UI, Saves, Setup, Reset,
Panic, Sys. Info, Basic Help, Reboot, Shutdown

Orange:
Load Preset, Recording, Notes, MIDI, Audio, UI, SD Card 2, HDMI Video,
Saves, Setup, Reset, Panic, Sys. Info, Basic Help, Reboot, Shutdown

Raspberry Gadget:
Load Preset, Recording, Notes, MIDI, Audio, UI, SD Card 2, HDMI Video,
Saves, Setup, Reset, Panic, Sys. Info, Basic Help, Reboot, Shutdown

Raspberry Host:
Load Preset, Recording, Notes, MIDI, Audio, UI, SD Card 2, HDMI Video,
Saves, Setup, Reset, Panic, Sys. Info, Basic Help, Reboot, Shutdown
```

The conditional child branches resolve as follows:

```
Desktop
  MIDI: MIDI Active; MIDI Host (MIDI Out, MIDI In); Sync / Clock
  Audio: Master Vol; Polyphony; Engine (CPU Warn %, Bus Idle, Buf Frames)
  Setup: Updates (Check, Apply, Rollback); Configure WiFi; Backup / Restore; Hardware Test

Orange
  MIDI: MIDI Active; MIDI Host (MIDI Out, MIDI In); USB Device (USB MIDI); Sync / Clock
  Audio: USB Audio; HDMI Audio; Master Vol; Perf. Mode; Polyphony; Engine (CPU Warn %, Bus Idle)
  SD Card 2: Start Transfer; Stop Transfer
  HDMI Video: Mode; Bars per cycle when cycle-behaviors; Grid Lines
  Setup: Updates (Check, Apply, Rollback); Configure WiFi; Backup / Restore; Hardware Test

Raspberry Gadget
  MIDI: MIDI Active; USB Device (USB MIDI); Sync / Clock
  Audio: USB Audio; HDMI Audio; Master Vol; Perf. Mode; Polyphony; Engine (CPU Warn %, Bus Idle)
  SD Card 2: Start Transfer; Stop Transfer
  HDMI Video: Mode; Bars per cycle when cycle-behaviors; Grid Lines
  Setup: USB Role; Updates (Check, Apply, Rollback); Configure WiFi; Backup / Restore; Hardware Test

Raspberry Host
  MIDI: MIDI Active; MIDI Host (MIDI Out, MIDI In); Sync / Clock
  Audio: HDMI Audio; Master Vol; Perf. Mode; Polyphony; Engine (CPU Warn %, Bus Idle)
  SD Card 2: Stop Transfer
  HDMI Video: Mode; Bars per cycle when cycle-behaviors; Grid Lines
  Setup: USB Role; Updates (Check, Apply, Rollback); Configure WiFi; Backup / Restore; Hardware Test
```

`MIDI Active` is the global runtime MIDI gate and does not select a port or device. Desktop and Orange expose `MIDI Host`, whose children are `MIDI Out` followed by `MIDI In`. Raspberry exposes `MIDI Host` only in Host role and `USB Device` only in Gadget role; Orange exposes both. `USB Device` is the computer-facing gadget interface and contains `USB MIDI`, which automatically owns both Gadget MIDI directions while enabled. Host-selected input and output IDs are ignored during that time; Orange retains them for later use after USB Device MIDI is disabled.

For ordinary recursive menu groups, parameter rows and submenus precede action rows. This puts `Name` before `Save`, `Auto Save` and `Backups` before the default actions, the `Updates` submenu before the Setup actions, and `Save Current` after the Library submenus. Dynamic preset workflows are the exception: `Rename` retains its existing preset-selection rows before `New Name` and `Apply`. There is no duplicate direct `System > Save Current` row.

`System > Audio > Master Vol` keeps key `masterVolume` and its existing range. `System > Notes` keeps the existing note-length, velocity-scale, and velocity-curve keys and semantics. Audio output rows remain restart-sensitive where they were, and all existing action/config keys, values, persistence, effects, confirmations, and Host disabling remain unchanged.

`Setup` keeps the existing USB role, Wi-Fi, Backup / Restore, Updates, and Hardware Test actions without changing their keys. `USB Role` is Raspberry-capability-only and precedes `Updates`. `Configure WiFi` and Backup / Restore retain their existing native behavior. `Reset > Load Empty` and `Reset > Load Factory` retain their existing confirmation and reset behavior.

`System > Saves > Library` keeps dynamic Load, Rename, and Delete rows. `System > Saves > Default` keeps rolling `Backups`, while `Auto Save`, Save Default, and Load Default retain their existing persistence behavior. Basic Help opens native help with the shortcut cheat sheet. Reboot and Shutdown remain the final System actions.

`System > SD Card 2` and `System > HDMI Video` are omitted on desktop. Raspberry Host hides USB Audio, USB MIDI, and SD2 Start Transfer while retaining HDMI Audio and SD2 Stop Transfer for cleanup. Raspberry Gadget shows USB Device and SD2 Start Transfer. Orange shows both MIDI Host and USB Device and has no USB Role row. Desktop shows MIDI Host and no USB Device.

`System > Sys. Info` opens a native loading popup and requests asynchronously identified, sanitized OS/version, Octessera version, primary IP/MAC when available, hostname, and explicit board profile information. The desktop UI only renders the resulting native snapshot.

`System > Recording` uses one `Max Time` setting for both formats. `Start Audio` writes the final internal stereo mix as WAV; `St. Audio+OLED` writes synchronized 128x128 MJPG/PCM AVI with audio-master timing at 10 fps; `Stop` finalizes either active format. Recording roots and partial/incomplete lifecycle are defined in the authoritative control and runtime-boundary specs.

`System > Reboot` and `System > Shutdown` retain their native confirmation and recovery-save flow. The runtime stops follow-ups, independently attempts external MIDI panic and internal audio silence, acknowledges the final shutdown/reboot snapshot, and then submits the fixed board-specific power request. The ordinary-menu toasts remain `Rebooting` and `Shutting down`.

`System > Setup > Configure WiFi` is confirmed as `Open Portal`. The confirmation stops and resets playback, clears note state, and sends MIDI panic/all-notes-off cleanup; playback does not auto-resume. It then emits the typed setup portal effect. The modal reports `starting`, `portal_ready` with `Octessera Setup <4-char code>` and `192.168.42.1` for 10 minutes, `finalizing`, `succeeded`, `failed`, `timed_out`, or desktop `unsupported`. Browser Applying is provisional and an AP disconnect is expected; the OLED result is authoritative. Success needs only a usable global `wlan0` IPv4 address, not Internet access, a default route, DNS, or ICMP. Success and timeout cards auto-hide, failure remains dismissible, and a new `Open Portal` action retries. The portal can change Wi-Fi, hostname, SSH, and the board's admin login (`pi` on Raspberry; `octessera` on Orange), with no reboot. Configure WiFi does not start or advertise Backup / Restore. The setup modal takes priority over system info and help.

`System > Setup > Backup / Restore` is a direct, unconfirmed action with stable key `system.backupRestore`. On Pi it uses the existing authenticated service at `http://<regular-ip>:8081`, selected from usable regular `wlan0` IPv4, with a generated 10-character code and 15-minute lifetime. The OLED card shows IP, port, code, expiry, and `> Stop service`; Back hides it while the service continues, and Stop closes it and revokes the code. Desktop is unsupported. This action is separate from rolling `System > Saves > Default > Backups`.

`System > HDMI Video > Mode` displays `Terminal` for the canonical stored/runtime value `none`. Terminal releases and disables Octessera framebuffer output so Linux terminal ownership can show, while snapshots retain the black/inactive HDMI grid for compatibility. `Bars per cycle` appears only for `cycle-behaviors` and sets how many musical bars each behavior remains shown before Cycle Behaviors advances.
