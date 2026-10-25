# System Menu Tree

Part of the authoritative menu/control spec; see [`../menu-tree-spec.md`](../menu-tree-spec.md) for
the index and notation. System holds device-local settings and the instrument's housekeeping
actions. Persistence rules shared with the rest of the menu (when System edits save, what a patch
load keeps) are in [Save and Load](../menu-and-controls-spec.md#save-and-load).

## Tree

```
System
├── Load Preset (group)                 ← direct shortcut to the named presets
│   ├── (none): (action)                ← shown when no presets exist; refreshes the list
│   └── <preset>: (action)              ← confirm, then load preset
├── Saves (group)
│   ├── Library (group)
│   │   ├── Save As (group)
│   │   │   ├── Name: (text, max 32)
│   │   │   └── Save: (action)
│   │   ├── Load (group)                 ← one action per preset, or (none)
│   │   ├── Rename (group)               ← one pick action per preset, or (none); after a pick, New Name (text, max 32) and Apply follow
│   │   ├── Delete (group)               ← one action per preset, or (none)
│   │   ├── Save Current: (action)
│   │   └── Refresh List: (action)
│   ├── Default (group)
│   │   ├── Auto Save: [on | off]        default off
│   │   ├── Backups: [on | off]          default on
│   │   ├── Save Patch: (action)
│   │   └── Load Patch: (action)
│   └── System (group)
│       ├── Save System: (action)
│       └── Load System: (action)
├── Recording (group)
│   ├── Max Time: [1..120] min  default 10
│   ├── Start Audio: (action)           ← final internal stereo output
│   ├── Audio+Video: (action)           ← synchronized audio, OLED, NeoKeys and grid
│   └── Stop: (action)                  ← finalize active recording
├── Notes (group)
│   ├── Note Length: [30..2000] ms  default 120
│   ├── Vel Scale: [0..200] step 5 %  default 100
│   └── Vel Curve: [linear | soft | hard]
├── Audio (group)
│   ├── USB Audio: [on | off]            ← Raspberry and Orange; hidden in Raspberry Host; restart-sensitive
│   ├── HDMI Audio: [on | off]           ← Raspberry and Orange; restart-sensitive
│   ├── Master Vol: [0..100] step 1
│   ├── Perf. Mode: [latency | capacity] ← Raspberry and Orange; shown as Lat | Cap; restart-sensitive
│   ├── Polyphony: [fixed12 | fixed16 | auto-soft | auto-balanced | auto-hard | none]
│   └── Engine (group)
│       ├── CPU Warn %: [70 | 75 | 80 | 85 | 90 | 95]  default 85
│       └── Bus Idle: [exact | -140 | -120 | -100 | -80]  default -120
├── MIDI (group)
│   ├── MIDI Active: [on | off]
│   ├── MIDI Host (group)                ← desktop, Orange, and Raspberry Host
│   │   ├── MIDI Out (group)             ← Disconnect, then one action per detected output port
│   │   └── MIDI In (group)              ← Disconnect, then one action per detected input port
│   ├── USB Device (group)               ← Orange and Raspberry Gadget
│   │   └── USB MIDI: [on | off]         ← owns both Gadget MIDI directions; restart-sensitive
│   └── Sync / Clock (group)
│       ├── Sync: [internal | external]
│       ├── Clock Out: [on | off]
│       ├── Clock In: [on | off]
│       └── Follow S/S: [on | off]
├── Connections (group)                ← Raspberry and Orange
│   ├── Wi-Fi (group)
│   │   ├── Wi-Fi: [on | off]            default on
│   │   └── Setup Portal: (action)       ← confirm Open Wi-Fi Setup
│   ├── Bluetooth (group)
│   │   ├── Bluetooth: [on | off]        default off
│   │   ├── Audio Out: [on | off]        ← while Bluetooth is on; default off
│   │   ├── Devices (group)              ← while Bluetooth is on; one connect/disconnect action per paired device
│   │   ├── Pair New (group)             ← while Bluetooth is on; searches while open; one pair action per found device
│   │   └── Forget (group)               ← while Bluetooth is on; one forget action per paired device
│   ├── SD Card 2 (group)                ← Raspberry and Orange
│   │   ├── Start Transfer: (action)     ← hidden in Raspberry Host
│   │   └── Stop Transfer: (action)
│   └── USB Role: [Gadget | Host]        ← Raspberry only; restart-sensitive
├── Display (group)
│   ├── Ghost Cells: [on | off]
│   ├── Auto Map: [on | off]
│   ├── Number Style: [bar | numbers | bar+numbers]
│   ├── Dim Timer: [0..600] step 10 s
│   ├── OLED Sleep: [0..600] step 10 s
│   ├── OLED Bright: [10..100] step 5
│   ├── Grid Bright: [10..100] step 5
│   ├── Button Bright: [10..100] step 5
│   └── HDMI Video (group)               ← Raspberry and Orange
│       ├── Mode: [Terminal | live-grid | plain-grid | active-behavior | cycle-behaviors]
│       ├── Bars per cycle: [1..64]      ← cycle-behaviors only
│       └── Grid Lines: [on | off]
├── Live Show Mode (group)               ← Raspberry and Orange
│   ├── Start 48h: (action)              ← confirm, then Wi-Fi off and housekeeping postponed for 48 h
│   └── End: (action)
├── Maintenance (group)
│   ├── Updates (group)
│   │   ├── Check: (action)
│   │   ├── Apply: (action)              ← Raspberry and Orange
│   │   └── Rollback: (action)           ← Raspberry and Orange
│   ├── Backup / Restore: (action)       ← Raspberry and Orange
│   ├── Hardware Test: (action)
│   ├── Reset (group)
│   │   ├── Load Empty: (action)
│   │   └── Load Factory: (action)
│   └── Sys. Info: (action)
├── Panic: (action)
├── Basic Help: (action)
├── Reboot: (action)
└── Shutdown: (action)
```

`Save Current` exists only under `Saves > Library`; there is no direct `System > Save Current` row.
Ordering exceptions to the parameters-before-actions rule are listed in
[Menu Structure](../menu-and-controls-spec.md#menu-structure).

## Resolved per platform

System children after conditional rows are resolved:

```
Desktop:
Load Preset, Saves, Recording, Notes, Audio, MIDI, Connections, Display,
Maintenance, Panic, Basic Help, Reboot, Shutdown

Orange, Raspberry Gadget, and Raspberry Host:
Load Preset, Saves, Recording, Notes, Audio, MIDI, Connections, Display,
Live Show Mode, Maintenance, Panic, Basic Help, Reboot, Shutdown
```

Conditional branches:

```
Desktop
  Audio: Master Vol; Polyphony; Engine (CPU Warn %, Bus Idle)
  MIDI: MIDI Active; MIDI Host (MIDI Out, MIDI In); Sync / Clock
  Display: Ghost Cells … Button Bright; no HDMI Video

Orange
  Audio: USB Audio; HDMI Audio; Master Vol; Perf. Mode; Polyphony; Engine (CPU Warn %, Bus Idle)
  MIDI: MIDI Active; MIDI Host (MIDI Out, MIDI In); USB Device (USB MIDI); Sync / Clock
  Connections: Wi-Fi (Wi-Fi, Setup Portal); Bluetooth; SD Card 2 (Start Transfer, Stop Transfer)
  Display: Ghost Cells … Button Bright; HDMI Video (Mode; Bars per cycle when cycle-behaviors; Grid Lines)

Raspberry Gadget
  Audio: USB Audio; HDMI Audio; Master Vol; Perf. Mode; Polyphony; Engine (CPU Warn %, Bus Idle)
  MIDI: MIDI Active; USB Device (USB MIDI); Sync / Clock
  Connections: Wi-Fi (Wi-Fi, Setup Portal); Bluetooth; SD Card 2 (Start Transfer, Stop Transfer); USB Role
  Display: Ghost Cells … Button Bright; HDMI Video

Raspberry Host
  Audio: HDMI Audio; Master Vol; Perf. Mode; Polyphony; Engine (CPU Warn %, Bus Idle)
  MIDI: MIDI Active; MIDI Host (MIDI Out, MIDI In); Sync / Clock
  Connections: Wi-Fi (Wi-Fi, Setup Portal); Bluetooth; SD Card 2 (Stop Transfer); USB Role
  Display: Ghost Cells … Button Bright; HDMI Video

Raspberry and Orange
  Maintenance: Updates (Check, Apply, Rollback); Backup / Restore; Hardware Test; Reset (Load Empty, Load Factory); Sys. Info

Desktop
  Maintenance: Updates (Check); Hardware Test; Reset (Load Empty, Load Factory); Sys. Info
```

Raspberry Host hides USB Audio, USB MIDI, and SD Card 2 Start Transfer, but keeps HDMI Audio and
Stop Transfer for cleanup. Switching back to Gadget does not restore outputs that Host disabled.

## Confirmations

Unless listed otherwise, a confirmation offers `Cancel` and `Confirm` with Cancel selected, and
nothing changes until Confirm.

| Action | Title | Text |
|---|---|---|
| Load Preset / Library Load | `Confirm Load` | `Load preset <name>?` |
| Library Delete | `Confirm Delete` | `Delete preset <name>?` |
| Save As > Save | `Confirm Save` | `Save preset <name>?` |
| Save Current | `Confirm Save` | `Overwrite preset <name>?` |
| Rename > Apply | `Confirm Rename` | `Rename <old> to <new>?` |
| Save Patch / Load Patch | `Confirm Patch` | `Save current patch?` / `Load saved patch?` |
| Save System / Load System | `Confirm System` | `Save System settings?` / `Load System settings?` |
| SD Card 2 Start Transfer | `Confirm SD2 Transfer` | `USB audio/MIDI disconnect. Host owns OLED SD2 until stopped.` |
| SD Card 2 Stop Transfer | `Confirm SD2 Transfer` | `Eject OLED SD2 on the host first, then stop transfer.` |
| Setup Portal | `Open Wi-Fi Setup` | see [Setup Portal](#setup-portal); options `Cancel` / `Open Portal` |
| Live Show Mode > Start 48h | `Live Show Mode` | options `Cancel` / `Start` |
| Updates > Apply | `Confirm Update` | `Apply the update now?` |
| Updates > Rollback | `Confirm Rollback` | `Rollback to the previous release?` |
| Hardware Test | `Confirm Hardware Test` | `Run the hardware test?` |
| Reset > Load Empty | `Confirm Load Empty` | `Load empty patch state?` |
| Reset > Load Factory | `Confirm Factory` | `Load factory settings?` |
| Panic | `Confirm MIDI` | `Send MIDI panic?` |
| Reboot / Shutdown | `Confirm Reboot` / `Confirm Shutdown` | `Reboot Octessera?` / `Shut down Octessera?` |

Refresh List, the Recording actions, Updates > Check, Backup / Restore, Sys. Info, Basic Help,
Live Show Mode > End, MIDI port rows, and Bluetooth rows run without confirmation.

## Load Preset and Saves

- `Load Preset` lists one confirmed load action per named preset, or a `(none)` refresh action. The
  same list is available under `Saves > Library > Load`.
- `Saves > System` holds local device settings, `Saves > Default` holds the default musical patch,
  and `Library` holds named presets. These actions use the same native input on hardware and
  desktop. When each one writes, and what a load keeps, is defined in
  [Save and Load](../menu-and-controls-spec.md#save-and-load).

## Recording

- One `Max Time` (`runtimeConfig.recording.maxMinutes`, `1..120`, default `10`) limits both formats.
- `Start Audio` emits `RuntimePlatformEffect::RecordingStartAudio { max_minutes }` and writes a
  44.1 kHz stereo signed-16 little-endian PCM WAV of the final internal stereo mix.
- `Audio+Video` emits `RuntimePlatformEffect::RecordingStartAudioOled { max_minutes }` and writes an
  AVI with 272x128 lossless PNG-frame video (`MPNG`) at 20 fps plus the same PCM audio. The frame is
  the OLED on the left, the four NeoKeys as a bottom-aligned 16-pixel column in the middle (black
  back/play/shift/layer glyphs), and the 8x8 grid on the right as 16-pixel blocks, with LED colors at
  full brightness. Audio is the master clock: each video frame advances every 2,205 audio frames,
  and accepted OLED frames and LED colors hold between updates.
- `Stop` emits `RuntimePlatformEffect::RecordingStop`, finalizes either format, and is safe when
  idle.
- Both formats capture only the final internal stereo output; external MIDI instruments are not
  recorded.
- Toasts: `Recording started`; `Recording is already running` for a duplicate start; `Recording
  saved` or `Recording incomplete` on manual Stop; `Max time: saved` on a clean max-duration finish.
- Recording does not keep the OLED awake. The normal `OLED Sleep` timer applies, and an Audio+Video
  take records all-black frames (OLED, keys, and grid) while the screen sleeps.
- Roots are fixed per host adapter: desktop uses the Tauri Audio/Music directory under
  `Octessera/recordings` (WAV) and `Octessera/screen-recordings` (Audio+Video); Raspberry uses
  `/home/pi/recordings` and `/home/pi/screen-recordings`; Orange uses
  `/var/lib/octessera/recordings` and `/var/lib/octessera/screen-recordings`.
- A reserved `.partial.*` file becomes `.wav`/`.avi` after successful finalization, or
  `.incomplete.wav`/`.incomplete.avi` after audio gaps or audio ingress overflow. A dropped screen
  frame only holds the previous picture and is logged. Failed workers keep the partial file, and
  existing takes are never replaced.

## Notes

`Note Length`, `Vel Scale`, and `Vel Curve` keep the keys `sound.noteLengthMs`,
`sound.velocityScalePct`, and `sound.velocityCurve`.

## Audio

- `Master Vol` keeps key `masterVolume`.
- The Jack (DAC) mix is always on. `USB Audio` and `HDMI Audio` are optional mirrors of it on the Pi
  boards, never replacements. HDMI audio is separate from HDMI video. Simultaneous physical outputs
  run on independent clocks and can drift or echo; they are not sample-aligned.
- `Perf. Mode` persists `runtimeConfig.sound.optimizeFor` as `latency` or `capacity` and shows the
  compact values `Lat` and `Cap`. Neither mode changes voice or FX limits. Desktop hides it.
- `Polyphony` stores `sound.voiceStealingMode`.
- `Engine > CPU Warn %` persists `runtimeConfig.dsp.workerWarningThreshold` and `Bus Idle` persists
  `runtimeConfig.dsp.busIdleThreshold`. Both propagate straight to the realtime engine without
  rebuilding prepared audio or changing structural render-plan revisions. Worker warning
  presentation ([status icons](../menu-and-controls-spec.md#status-icons-and-transport-indicators))
  and bus silence handling are separate runtime phases.

## MIDI

- `MIDI Active` is the global runtime MIDI gate. It selects no port or device.
- `MIDI Host` lists `MIDI Out` then `MIDI In`. Each starts with `Disconnect`, followed by one action
  per detected port.
- `USB Device` is the computer-facing gadget interface. While `USB MIDI` is on it owns both Gadget
  MIDI input and output, and Host-selected input/output IDs are ignored. Orange keeps those Host
  selections for use after USB MIDI is turned off. USB MIDI is restart-sensitive: a pending edit does
  not change the live route until the next process start.
- General MIDI enable, ports, clock, and sync stay under `System > MIDI`.

## Restart-sensitive settings

USB Audio, HDMI Audio, Perf. Mode, USB MIDI, and USB Role are restart-sensitive. A
changed edit opens the native confirmed System-only Apply flow:

1. `Apply System` with `Restart required.`. In Host role it adds `Before Host reboot:`,
   `Unplug computer USB.`, and `Audio/MIDI/SD2 off.`. Options: `Cancel` (default), `Save this one`
   (only when that setting can be saved on its own), and `Save System`. Back means Cancel.
2. `Saving...` / `Please wait` while the System file is written. A second Apply during this shows
   `System Apply save pending, try again`.
3. `Restart?` with `System saved.` / `Reboot to apply?`. In Host role it adds
   `Unplug computer USB`, `before Host reboot.`, and `Audio/MIDI/SD2 off.`. Options: `Continue`
   (default) and `Reboot now`. Back means Continue.

The save writes only System settings, never discards unsaved musical edits, and keeps unrelated
dirty edits. Stale or failed acknowledgements never reboot or advance the persisted baseline.

## Connections

### Wi-Fi

- The `Wi-Fi` switch (key `wifi.enabled`, persisted as `runtimeConfig.wifi.enabled`, default on) is
  a local System setting on Raspberry and Orange. It is never part of portable patches or `.oct`
  backups, so a restore cannot switch a fresh board's radio off. Desktop has no switch row.
- A change applies immediately and shows `Wi-Fi off: no SSH` or `Wi-Fi on`. The Pi host mirrors the
  setting into `/var/lib/octessera/show-hold/wifi-off` on every change and once at startup, writing
  or removing the file. `octessera-show-hold-apply` (also run at boot and whenever that file
  changes) sets the NetworkManager radio with `nmcli radio wifi off|on`, which keeps saved
  connections.
- The switch and Live Show Mode are independent. The show blocks the radio with `rfkill` only while
  it lasts, and its End or expiry only lifts that block, so a switched-off radio stays off. On
  Raspberry the network-health timer runs only while no show is active and the switch is on.

### Setup Portal

- Stable key `system.configureWifi`, on Raspberry and Orange. The confirmation is `Open Wi-Fi Setup`
  (`Playback stops.`, `Wi-Fi disconnects.`, `Setup may change:`, `SSH, hostname,`, `and login.`)
  with `Cancel` and `Open Portal`; Cancel toasts `Cancelled`.
- `Open Portal` stops and resets playback, clears note state, and sends MIDI panic/all-notes-off
  cleanup. Playback never auto-resumes. It emits the typed setup portal effect, and opening the
  portal switches the Wi-Fi setting back on and removes the switch file before the radio comes on.
- The modal reports `starting`, `portal_ready` with `Octessera Setup <4-char code>` and
  `192.168.42.1` for 10 minutes, `finalizing`, `succeeded`, `failed`, `timed_out`, or desktop
  `unsupported`. The modal ranks after confirmation dialogs and ahead of system info and help.
- The browser's Applying page is provisional and an AP disconnect is expected; the OLED result is
  authoritative. Success needs only a usable global `wlan0` IPv4 address, not Internet access, a
  default route, DNS, or ICMP.
- Success and timeout cards auto-hide, a failure stays until dismissed, and a new `Open Portal`
  retries.
- The portal can change Wi-Fi, hostname, SSH, and the board's admin login (`pi` on Raspberry,
  `octessera` on Orange) without a reboot. It does not start or advertise Backup / Restore.

### Bluetooth

- `Bluetooth` (key `bluetooth.enabled`, persisted as `runtimeConfig.bluetooth.enabled`, default off)
  powers the board radio; off powers it down. The remaining rows appear only while it is on.
- `Audio Out` (key `bluetooth.audio`, persisted as `runtimeConfig.bluetooth.audio`, default off)
  sends a copy of the final mix to the first paired, connected speaker. The primary callback fills a
  bounded, never-blocking monitor tap, and a background feeder (SCHED_OTHER, nice 10, on the DSP
  cores) plays it through bluez-alsa. Monitor audio is dropped under pressure, lags by a few hundred
  milliseconds, and never changes the wired outputs; a disconnected speaker stops it.
- `Devices` lists paired devices as `name [kbd]` or `name [audio]`, with ` - on` for connected ones.
  Pressing a row connects or disconnects it.
- `Pair New` emits `BluetoothScan { active: true }` when it opens and `active: false` when Back
  leaves it. It opens while still empty and lists found unpaired keyboards and speakers. Pressing a
  row pairs, trusts, then connects. A passkey shows as a modal; Back or a Main click cancels pairing.
- `Forget` removes a pairing. Results and failures arrive as `BluetoothStatus` toasts.
- While Bluetooth is on, keyboard capture also accepts Bluetooth keyboards under the same HDMI gate,
  so Raspberry Gadget can be played from a Bluetooth keyboard.

### SD Card 2 and USB Role

- `Start Transfer` and `Stop Transfer` keep their modal/cancel behavior. The second card is exposed
  to a USB host only while conflicting USB audio, MIDI, and recording are inactive.
- `USB Role` (Raspberry only) is the last Connections row. Host hides and disables USB Audio, USB
  MIDI, and SD2 Start Transfer; see [Resolved per platform](#resolved-per-platform).

## Display

- `Auto Map` is described under
  [Auto-map](../menu-and-controls-spec.md#auto-map), `Ghost Cells` under
  [Overlays and priority](../menu-and-controls-spec.md#overlays-and-priority), and the brightness
  rows under [Brightness](../menu-and-controls-spec.md#brightness).
- `HDMI Video > Mode` persists `runtimeConfig.hdmi.mode`
  (`terminal|live-grid|plain-grid|active-behavior|cycle-behaviors`, default `terminal`; older saves
  stored `none`, which loads as `terminal`). Terminal releases and disables Octessera framebuffer output so the Linux terminal can
  show, while snapshots keep their black/inactive HDMI grid.
- `Bars per cycle` (`hdmi.cycleMeasures`) appears only in `cycle-behaviors` and sets how many bars
  each behavior stays on screen before Cycle Behaviors advances.
- Hardware HDMI timing can be sampled on Raspberry and Orange without framebuffer writes using
  `cargo run -p octessera-pi --bin octessera-pi-hdmi-bench --release`.

## Live Show Mode

Board-only (Raspberry and Orange); desktop reports it as board-only.

- `Start 48h` (`maintenance.hold`) opens a `Live Show Mode` confirmation (`Cancel` / `Start`) that
  says Wi-Fi goes off for 48 h. `End` (`maintenance.release`) is direct. Both emit
  `RuntimePlatformEffect::MaintenanceHold { active }` and toast `Live Show Mode: 48h` or
  `Live Show Mode off`.
- The board writes or removes `/var/lib/octessera/show-hold/until` (Unix expiry seconds). While that
  time is in the future and no more than about 48 h away, the images' `ExecCondition` drop-ins skip
  fstrim, logrotate, systemd-tmpfiles-clean, and the Raspberry network-health logger. The daily
  dpkg-db-backup timer is masked on both images.
- On both boards `octessera-show-hold.path` runs `octessera-show-hold-apply` whenever the file
  changes and at boot. While the mode is on it blocks the Wi-Fi radio (`rfkill block wlan`;
  Bluetooth is untouched), on Raspberry also stops `octessera-network-health.timer`, and schedules a
  transient timer that re-runs the script at expiry. End or expiry unblocks Wi-Fi and, on Raspberry,
  restarts the timer unless the Wi-Fi switch is off.
- Skipped jobs run at their next schedule. The hold survives reboots.

## Maintenance

### Updates

- `Check` emits `RuntimePlatformEffect::UpdateCheck` without confirmation. On desktop it opens the
  GitHub releases page; desktop has no `Apply` or `Rollback` rows.
- On both boards Check reports the newest published release that has this board's
  `octessera-<version>-<profile>-device-aarch64.zip` and `SHA256SUMS-<profile>-device.txt`
  pair, or `Up to date (v<installed>)`. Raspberry runs the updater through `sudo -n`; Orange
  sends the action over the root-owned update socket. Neither board uses the other's assets.
- These actions update only the runtime release. Armbian, kernel, device-tree, and image changes
  still need a reflash.
- Apply and Rollback confirm first. Apply downloads the release, verifies its checksum (a mismatch
  stops before anything changes), and replies `Installing v<version>; Octessera restarts in a
  moment`. Rollback replies `Going back to v<version>; ...`, or `No previous release to go back to`.
  Both refuse while a development build drop-in is active.
- The switch then restarts the runtime. If it is not active with its readiness marker within
  60 seconds, the previous release comes back automatically. The device keeps the current and the
  previous release and removes older ones.

### Backup / Restore

- Direct, unconfirmed action with stable key `system.backupRestore`.
- On Pi it opens the authenticated service at `http://<regular-ip>:8081`, using the usable regular
  `wlan0` IPv4 address, a generated 10-character code, and a 15-minute lifetime. The OLED card shows
  IP, port, code, expiry, and `> Stop service`. Back hides the card while the service keeps running;
  Stop closes it and revokes the code. The card ranks after confirmation dialogs and the setup
  portal, ahead of system info and help. Desktop has no Backup / Restore row.
- The `.oct` archive carries music, selected portable preferences, and optionally media. It is not a
  complete device backup: USB role, MIDI/audio endpoint IDs, and favourites stay local to the
  destination. Rolling `Saves > Default > Backups` are separate and patch-only.

### Reset

- `Load Empty` (`system.clearAll`) opens `Confirm Load Empty`; Cancel is a no-op. Confirm stops
  playback with the usual MIDI panic/note-safety path, loads an empty `none`-behavior patch,
  regenerates the preset draft name, marks the patch dirty for autosave, and keeps device/user
  preferences: brightness, ghost cells, numeric display, sleep/dim timers, master volume, Auto
  Save/Backups, MIDI setup/status/sync, sync source, audio output buffer frames, sample favourites,
  input-events-while-paused, aux auto-map, and the preset name list.
- `Load Factory` (`factory.load`) opens `Confirm Factory` / `Load factory settings?` and loads the
  factory payload on Confirm.

### Sys. Info

`Sys. Info` (`system.info`) opens a native loading popup and emits
`RuntimePlatformEffect::SystemInfoRequest`. Desktop and Pi return an asynchronously identified,
sanitized `RuntimeSystemInfo` with OS/version, Octessera version, primary IP/MAC when available,
hostname, and the explicit board profile. The desktop UI only renders the resulting snapshot.

## Panic, Basic Help, Reboot, Shutdown

- `Panic` (`midi.panic`) confirms with `Confirm MIDI` / `Send MIDI panic?`. `Basic Help` opens
  native help with the shortcut cheat sheet.
- `Reboot` and `Shutdown` confirm, then run the recovery save. They refuse with
  `System save pending, try again` while a System write or follow-up is pending.
- One board-neutral terminal lifecycle follows. It needs the save to succeed, stops non-terminal
  follow-ups and new input atomically, and independently attempts external MIDI panic and internal
  audio silence. Only after both succeed does it publish and physically acknowledge the final
  shutdown/reboot snapshot and LED/OLED state, then submit the board's fixed power request. Save,
  safety, acknowledgement, and power-submission failures are typed and logged and never resume
  normal follow-ups incorrectly.
- Toasts stay exactly `Rebooting` and `Shutting down`; the OLED frame is kept while native hardware
  teardown completes.
- Raspberry uses the exact noninteractive power commands allowed by
  `/etc/sudoers.d/octessera-shutdown`, with no broad `NOPASSWD` grant. Orange uses its fixed
  root-owned `/run/octessera-device-apply/reboot.sock` with exact `reboot\n` and `poweroff\n`
  requests and exact `accepted\n`/`rejected\n` responses. Orange reboot still validates persisted
  config in the root helper; poweroff does not depend on that validation.
- This contract covers the confirmed instrument-menu actions, not administrative
  `systemctl poweroff` or `systemctl reboot`.

## Orange runtime I/O

- Musical events, runtime audio commands, and silence route through the realtime engine at the
  shared 44.1 kHz runtime rate. Readiness depends on Jack; an absent, waiting, or faulted optional
  mirror is route-local and never blocks or replaces Jack.
- MIDI uses the native host adapter, including USB MIDI when the configured gadget port is present.
- The service polls NeoTrellis/NeoKey over `/dev/i2c-2` at about 10 ms and uses the native gpiocdev
  v2 encoder path for all four encoders. Orange HAL descriptors reverse each literal board A/B net at
  the event boundary so every encoder turns in the canonical direction without changing the shared
  quadrature decoding.
- AUX2 A/B offsets 227/269 are always requested. Switch offset 224 is omitted only while UART0 is
  active and is requested once the dedicated UART0-disabled input-routing overlay is applied.
  Lower-left grid and GRB output semantics are unchanged.
