# Troubleshooting

Find the row that matches what you're seeing and start with the first action.
Power down before opening the case, moving wiring or reseating a connector.
And take both microSD cards out before you open or close the case: they catch
on the enclosure and break.

## Sound and playing

| Symptom | First action | More |
|---|---|---|
| No sound from the device | Check that the layer's Link **Inst** points at an instrument that isn't `none`, that it has a level in Play Mix, and the cable. The shipped patch (`System > Maintenance > Reset > Load Factory`) is a known-good starting point. | [Sound and Link](sound-and-link.md), [behaviors](behaviors-and-play.md) |
| A `midi` instrument stays silent | Switch on its **MIDI > Enabled** and `System > MIDI > MIDI Active`, and check the channel. | [Sound and Link](sound-and-link.md#shape-what-it-sounds-like), [USB roles](hardware/usb-roles.md) |
| Notes hang on external gear | `System > Panic` sends all-notes-off. | |
| Crackles or short dropouts | Set `System > Audio > Polyphony` to `auto-hard` first (it keeps the low-latency feel), then thin out dense patches or switch **Perf. Mode** to Capacity. Avoid updates or SSH maintenance while playing. | [Practical performance](README.md#practical-performance), [playing in front of people](README.md#playing-in-front-of-people) |
| Controls or grid respond oddly | Look at the OLED for a Play page or assignment mode, then press **Back** or jump away with **Fn**. | [Controls cheat sheet](controls-cheat-sheet.md) |
| A keyboard does nothing | Set `System > Display > HDMI Video > Mode` to a graphical mode (the default `Terminal` ignores keyboards). On the Raspberry, a wired keyboard also needs USB Role Host. | [Controls cheat sheet](controls-cheat-sheet.md#usb-host-keyboard), [Bluetooth](bluetooth.md) |
| A sample slot shows `N/A-<name>` | That WAV isn't on this device. Copy it into the samples folder or pick a replacement. | [Samples](README.md#samples), [backup and restore](data-backup-restore.md) |
| The SD2 card doesn't show up | It must be FAT32 or exFAT and named exactly `OCTESSERA`. Give it a couple of seconds after inserting. | [Samples](README.md#samples) |
| Desktop simulator is silent | Check the computer's audio output, then press **Space** with the shipped patch loaded. | [Desktop simulator](desktop-simulator.md#make-a-first-sound) |

## Setup and network

| Symptom | First action | More |
|---|---|---|
| Setup hotspot or page doesn't appear | End Live Show Mode if it's on, then open `System > Connections > Wi-Fi > Setup Portal` again. Keep the device powered and don't interrupt an apply in progress. | [Setup portal](hardware/setup-portal.md) |
| Setup finished but the hotspot vanished | That's expected. Reconnect to your normal network; `System > Maintenance > Sys. Info` shows the new address. | [Setup portal](hardware/setup-portal.md) |
| No SSH or updates | Check `System > Connections > Wi-Fi > Wi-Fi` is On and Live Show Mode isn't running. | [Playing in front of people](README.md#playing-in-front-of-people) |
| USB audio or MIDI is missing | Check the data port, cable direction and enclosure power input. | [USB roles](hardware/usb-roles.md), [safety and power](hardware/safety-and-power.md#usb-data) |

## Hardware

| Symptom | First action | More |
|---|---|---|
| OLED is blank, flickering or unstable | Power down and check the display connections. | [Flash and first boot](hardware/flash-and-first-boot.md), [assembly manual](hardware/assembly-manual.md) |
| Brownout, reboot, heat or power instability | Power down and check the regulated supply, input, cables and ventilation. | [Safety and power](hardware/safety-and-power.md) |
| The case doesn't fit or a port is blocked | Don't force it. Remove the microSD cards, then check the fit and cable routing. | [Assembly manual](hardware/assembly-manual.md), [enclosure notes](hardware/enclosure.md) |

`System > Maintenance > Hardware Test` walks you through the grid LEDs, every
pad, the four keys, the encoders and a test tone, then tells you what it saw.
It's handy right after assembly, or when one pad seems to have gone quiet. If nothing here matches, stop before changing wiring and
go back to the [flash and first-boot guide](hardware/flash-and-first-boot.md),
[USB roles](hardware/usb-roles.md) or [safety and power](hardware/safety-and-power.md)
page that best fits the problem.
