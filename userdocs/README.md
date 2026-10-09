# Octessera user manual

Octessera is a collection of small algorithmic musical world-bubbles. Set up a
few systems, nudge them, anchor them with a little sequencing, and play the
result together with the machine.

## 1. Try the simulator

Start with the [desktop simulator](desktop-simulator.md) if you want to make a
sound before building anything. It needs no PCB, board, OLED, or soldering iron.

**Next:** [Learn the controls](controls-cheat-sheet.md)

## 2. Build the device

Choose a Raspberry Pi Zero 2 W or Orange Pi Zero 2W. Use the matching board
image and enclosure top.

[Build and assembly manual](hardware/assembly-manual.md) · [Safety and
power](hardware/safety-and-power.md)

**Next:** [Flash and first boot](hardware/flash-and-first-boot.md)

## 3. Flash and set it up

[Flash and first-boot guide](hardware/flash-and-first-boot.md) — matching image,
flashing, first boot, and network setup.

[USB roles](hardware/usb-roles.md) — host-computer connections.

**Next:** [Make music](#4-make-music)

## 4. Make music

1. In **Build**, choose `life`, `brain`, or `raindrops` for a layer.
2. Draw a few cells on the grid.
3. In **Shape**, choose a **synth**.
4. Press **Play** or **Space**.
5. Use **Play Mix** to adjust the layer's level, then explore the other Play
   pages.

[Controls cheat sheet](controls-cheat-sheet.md) · [Behaviors and Play
pages](behaviors-and-play.md) · [Recording](recording.md)

**Next:** [Controls cheat sheet](controls-cheat-sheet.md)

**Something is wrong?** Start with [troubleshooting](troubleshooting.md). Power
down before opening the case or moving wiring.

## Reference

### Contents

- **Operation:** [Controls cheat sheet](controls-cheat-sheet.md), [Behaviors and
  Play pages](behaviors-and-play.md), [Recording audio and OLED](recording.md),
  [Data backup and restore](data-backup-restore.md),
  [Bluetooth keyboards and speakers](bluetooth.md), and
  [Troubleshooting](troubleshooting.md).
- **Build and hardware:** [Assembly manual](hardware/assembly-manual.md),
  [Flash and first boot](hardware/flash-and-first-boot.md),
  [Setup portal](hardware/setup-portal.md), [Enclosure](hardware/enclosure.md),
  [Protective case (optional)](hardware/protective-case.md),
  [Safety and power](hardware/safety-and-power.md), and
  [USB roles](hardware/usb-roles.md).
- **Quick reference:** [Printable quick reference](print/quick-reference.pdf).

### Practical performance

Use these as planning targets for dense patches:

| Mode | Synth voices | Sample voices | Bus FX | Global FX |
|---|---:|---:|---:|---:|
| Raspberry Latency | 16 | 16 | 8 | 2 |
| Raspberry Capacity | 32 | 32 | 8 | 2 |
| Orange Latency | 24 | 24 | 8 | 2 |
| Orange Capacity | 64 | 32 | 12 | 2 |

Voice counts are totals across all instrument slots, not per-instrument limits.
Adaptive voice stealing may reduce the active synth count as load rises. These
are practical targets rather than guarantees; behaviors, samples, and effects
all change the available headroom.

How quick does it feel? In Latency mode a grid press reaches your ears in about
6 ms on the Orange and 8 ms on the Raspberry. Capacity mode renders a little
ahead so it can spread the work over two cores, which puts it at about 13 to
15 ms: still fine for most playing, but you may feel it on fast finger drumming.

### Playing in front of people

Octessera boards skip the operating system's daily package-list and
manual-index jobs, plus a few other chores that do nothing useful on an
instrument. They never installed anything here anyway, but on the Raspberry
they made the audio stutter for a few seconds at a random time each day.

A handful of useful housekeeping jobs still run on a timer: log rotation, SD
card trim, temp-file cleanup, a package-database backup, and on the Raspberry
a once-a-minute Wi-Fi health log. Each of them has been run during playback
without disturbing the sound, but if you'd rather not find out the hard way,
use **System > Setup > Show Hold > Hold 48h** before a show. It postpones those
jobs for 48 hours (a reboot doesn't cancel it); they simply run at their next
scheduled time afterwards. **Release** ends the hold early. A couple of tiny
chores always keep running because things break without them: saving the
clock, and on the Orange trimming the in-memory logs.

The Raspberry's four cores share one fairly slow memory bus, so anything
heavy happening next to the music can still cause crackles even when it runs
on another core. Do your maintenance before the show, not during it: updates,
`apt` over SSH, copying big files onto the board. The Orange has a lot more
memory headroom and shrugs most of this off, but the same habit doesn't hurt.

One more thing: don't run `apt upgrade` on the board. Octessera boots its own
tuned kernel and device-tree setup, and a stock kernel or bootloader package
sneaking in can leave you with a board that boots but doesn't make a sound (or
doesn't boot at all). The boards hold those packages so apt leaves them alone,
but please don't go fishing with `--allow-change-held-packages`. Octessera
updates come through **System > Setup > Updates** or a fresh image.

### Samples

The default library has 320 media files: 318 WAV files available to the
sampler, plus two AIFF files outside the WAV-only browser. You can add your own
samples through the desktop sample browser or the board sample paths.

For the optional OLED microSD card, format it as FAT32 or exFAT and name the
volume `OCTESSERA`; the instrument only picks up a card with exactly that name.
This is SD2; the selected board's boot card is SD1. Put WAV files under
`octessera/samples`. You can swap the card while the instrument is on; give it
a couple of seconds to show up. If you use **System > SD Card 2 > Start
Transfer**, eject the host drive before
pressing **Back** or **Main** to stop the transfer.
