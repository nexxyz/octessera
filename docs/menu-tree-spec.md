# Menu Tree Spec

This file is part of the authoritative menu/control spec rooted at `menu-and-controls-spec.md`. Keep it in sync with native menu tree changes.

This is the canonical index for the full menu tree. The split-out section files below are canonical parts of this spec and exist to keep each prose file navigable.

## Menu Tree (Full)

### Root Menu

```
Root (group)
├── Build (group)
├── Link (group)
├── Shape (group)
├── Play (group)
├── [spacer] (visual separator)
└── System (group)
```

## Split-out sections

- [Build](menu-tree/worlds.md)
- [Link](menu-tree/pulses.md)
- [Shape](menu-tree/tones.md)
- [Play](menu-tree/sparks.md)
- [System](menu-tree/system.md)

Short breadcrumb forms use `B`, `L`, `S`, and `P` for Build, Link, Shape, and Play.

The System section puts the direct Save Current and dynamic Load Preset shortcut group, followed
by Master Vol, Panic, Sys. Info, and Basic Help. Recording, Notes, MIDI, Audio, UI, Saves,
Setup, Reset, Reboot, and Shutdown follow in that order. SD Card 2 and HDMI Video are
board-only groups and are omitted on desktop; USB MIDI is a board-only child of MIDI.

The root Load Preset group contains a `(none)` refresh action and one confirmed action
for each named preset. The same named list remains available under
`System > Saves > Library > Load`.

`System > Notes` owns `Note Length`, `Vel Scale`, and `Vel Curve` with their
existing keys and semantics. `System > Audio` owns the supported USB and HDMI
audio mirrors, `Perf. Mode`, `Polyphony`, and `Engine`. `Perf. Mode` is visible
on Raspberry and Orange and displays compact `Lat | Cap` labels for the full
`Latency | Capacity` choices; desktop hides it and
all Jack/USB/HDMI/SD2 device controls. `Polyphony` keeps the existing
`sound.voiceStealingMode` values `fixed12`, `fixed16`, `auto-soft`,
`auto-balanced`, `auto-hard`, and `none`. `Engine` contains CPU Warn % and Bus
Idle with their existing keys and values, plus Buf Frames where the
performance-mode capability is unavailable.

`System > MIDI` contains Enabled, board-only USB MIDI, MIDI Out, MIDI In, and Sync / Clock in
that order. Enabled is the runtime MIDI gate and selects no port or device. USB MIDI enables
the bidirectional class device after restart, auto-routes outbound gadget MIDI, and leaves
inbound selection under MIDI In. `System > SD Card 2` contains Start Transfer and Stop Transfer;
it exposes the second card to a USB host only while conflicting USB audio, MIDI, and recording
are inactive. HDMI Video contains the existing Mode, Bars per cycle, and Grid Lines controls.
USB and HDMI audio mirror the canonical Jack mix and do not replace it; HDMI audio remains
separate from HDMI video. Restart-sensitive edits use the native Save Setting flow shared with
Audio.

Aggregate audio-load and voice-steal status is separate from the red persistent-
worker CPU icon. The icon is at `(117,5)` and requires valid persistent
`high_cpu_steady` evidence; inline or missing evidence hides it. An active
missed-quantum flash takes priority in that CPU slot and inverts the CPU glyph
white/black; the red CPU icon is hidden until the flash clears. The yellow save
icon at `(107,5)` may coexist with either CPU-slot state. A matching newly missed quantum repeats the
previous final master quantum once; subsequent pending-recovery refills are
silent. `missedQuantumFlash` stays true for five emitted seconds, clears on the
exact emitted-frame crossing, and resets on a later miss. The existing OLED
presentation structure and coordinates are unchanged.

The System section's `HDMI Video` group displays `Terminal` for the stored/runtime
value `none`; its `Bars per cycle` row is conditional on `cycle-behaviors`.
See the split-out tree for the framebuffer ownership and snapshot semantics.

Within `System > Setup`, Configure WiFi, Backup / Restore, Updates, and
Hardware Test retain their existing action keys. `Configure WiFi` uses stable
key `system.configureWifi`. After confirmation, native runtime stops and resets
playback, sends MIDI panic/note cleanup, never auto-resumes, and emits the typed
setup portal effect. The setup modal reports `starting`, `portal_ready` with
the four-character code and `192.168.42.1` for 10 minutes, `finalizing`,
`succeeded`, `failed`, `timed_out`, or desktop `unsupported`.

`System > Setup > Backup / Restore` is a direct, unconfirmed action with stable
key `system.backupRestore`. On Pi it opens the existing authenticated service
on `http://<regular-ip>:8081` using a generated 10-character code and a
15-minute lifetime. Desktop is unsupported, and the action is separate from
rolling `System > Saves > Default > Backups`.
