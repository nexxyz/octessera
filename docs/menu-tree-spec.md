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

Within `System`, `Configure WiFi` is an action between `Info` and `Basic Help`. Its stable key is `system.configureWifi`. After confirmation, native runtime stops and resets playback, sends MIDI panic/note cleanup, never auto-resumes, and emits the typed setup portal effect. The setup modal reports `starting` while the hotspot starts, `portal_ready` with the four-character code and `192.168.42.1` for 30 minutes, `finalizing` while settings apply, `succeeded` without a reboot, `failed` with possible partial settings, `timed_out` when the portal closes, and desktop `unsupported`; Hide does not cancel it, and retry requires a new action.
