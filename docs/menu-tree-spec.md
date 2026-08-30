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

The System section's `Audio / USB` group contains the independent Jack Audio,
USB Audio, HDMI Audio, USB MIDI, Save / Reboot, Start SD2 Xfer, and Stop SD2
Xfer rows. See the split-out tree for the stable order and restart semantics.

The System section's `HDMI` group displays `Terminal` for the stored/runtime
value `none`; its `Bars per cycle` row is conditional on `cycle-behaviors`.
See the split-out tree for the framebuffer ownership and snapshot semantics.

Within `System`, `Configure WiFi` and `Backup / Restore` are actions between `Info` and `Basic Help`. `Configure WiFi` uses stable key `system.configureWifi`. After confirmation, native runtime stops and resets playback, sends MIDI panic/note cleanup, never auto-resumes, and emits the typed setup portal effect. The setup modal reports `starting`, `portal_ready` with the four-character code and `192.168.42.1` for 10 minutes, `finalizing`, `succeeded`, `failed`, `timed_out`, or desktop `unsupported`. Browser Applying is provisional and an AP disconnect is expected; the OLED result is authoritative. Success needs only a usable global `wlan0` IPv4 address, not Internet access, a default route, DNS, or ICMP. Success and timeout cards auto-hide, failure remains dismissible, and a new `Open Portal` action retries. Configure WiFi does not start or advertise Backup / Restore.

`System > Backup / Restore` is a direct, unconfirmed action with stable key `system.backupRestore`. On Pi it opens the existing authenticated service on `http://<regular-ip>:8081` using a generated 10-character code and a 15-minute lifetime. It selects a usable regular `wlan0` IPv4 address; no address means typed unavailable with no bind or retry. Reopening keeps the same URL, code, and remaining lifetime. The OLED shows the IP, port, code, expiry, and `> Stop service`; Back hides the card while the service continues, and Stop closes it and revokes the code. Expiry and authentication revocation close it automatically. Desktop is unsupported, and the action is separate from rolling `System > Saves > Default > Backups`.
