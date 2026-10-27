# Octessera user manual

Octessera is a collection of small algorithmic musical world-bubbles. Each
layer runs its own little system (a Game of Life colony, a flock of boids, a
slow-growing crystal, a step sequencer you poke by hand), and you set them up,
nudge them, anchor them with a bit of sequencing and probability, and play
along. You aren't writing notes into a piano roll. You're co-writing with a
handful of tiny worlds that sometimes have opinions.

This page is the front door. Everything else is linked from here.

## 1. Try the simulator

The [desktop simulator](desktop-simulator.md) runs the same native engine as
the hardware. It needs no PCB, board, OLED or soldering iron, so it's the
cheapest way to find out whether you like this sort of thing.

**Next:** [learn the controls](controls-cheat-sheet.md)

## 2. Build the device

Pick a Raspberry Pi Zero 2 W or an Orange Pi Zero 2W, then use the matching
board image and enclosure top. Both work; if you're buying a board, the
[manual explains](hardware/assembly-manual.md#1-safety-and-board-choice) why
I'd go for the Orange.

- [Build and assembly manual](hardware/assembly-manual.md)
- [Safety and power](hardware/safety-and-power.md)

**Next:** [flash and first boot](hardware/flash-and-first-boot.md)

## 3. Flash it and set it up

- [Flash and first boot](hardware/flash-and-first-boot.md): the matching
  image, flashing, first boot and network setup.
- [Setup portal](hardware/setup-portal.md): change Wi-Fi, password, SSH or
  hostname later.
- [USB roles](hardware/usb-roles.md): what the USB port does with a computer,
  a keyboard or MIDI gear.

**Next:** [make music](#4-make-music)

## 4. Make music

The shipped patch is wired up and ready: Layer 1 runs `life` into Instrument 1,
a synth.

1. In **Build**, stay on Layer 1 and draw a few cells on the grid. Try swapping
   its behavior for `brain` or `raindrops` too.
2. Press **Play** (or **Space** in the simulator).
3. Hold **Fn** and press the bottom cell of the right-hand grid column to open
   **Play Mix**. Each column is an instrument; press higher for louder.
4. Press **Back** to leave Play Mix, then go and poke at the other layers.

**Save what you like.** Auto Save is off, so your edits live in memory until
you save them: `System > Saves > Default > Save Patch`, or a named preset under
`System > Saves > Library`. Reboot and Shutdown from the menu save a recovery
copy first. Pulling the plug doesn't, and the last half hour of tinkering goes
with it.

If you've changed things and want the starting point back,
`System > Maintenance > Reset > Load Factory` restores it (it asks first).

Where to go next:

- [Controls cheat sheet](controls-cheat-sheet.md): every button, combo and
  grid mode.
- [Behaviors and Play pages](behaviors-and-play.md): the 48 worlds and the
  performance pages.
- [Sound and Link](sound-and-link.md): how cell changes become notes,
  instruments and effects.
- [Recording](recording.md): capture audio or audio plus video.

**Something wrong?** Start with [troubleshooting](troubleshooting.md). Power
down before opening the case or moving wiring.

## Reference

### Contents

- **Playing:** [Controls cheat sheet](controls-cheat-sheet.md),
  [Behaviors and Play pages](behaviors-and-play.md),
  [Sound and Link](sound-and-link.md), [Recording](recording.md),
  [Bluetooth keyboards and speakers](bluetooth.md),
  [Data backup and restore](data-backup-restore.md),
  [Troubleshooting](troubleshooting.md).
- **Building:** [Assembly manual](hardware/assembly-manual.md),
  [Flash and first boot](hardware/flash-and-first-boot.md),
  [Setup portal](hardware/setup-portal.md), [Enclosure](hardware/enclosure.md),
  [Protective case (optional)](hardware/protective-case.md),
  [Safety and power](hardware/safety-and-power.md),
  [USB roles](hardware/usb-roles.md).
- **Printable sheets:** [Quick reference (PDF)](print/quick-reference.pdf),
  [Cell-to-audio flowchart](print/cell-to-audio-flow.svg). The
  [controls cheat sheet](controls-cheat-sheet.md) also prints fine.
- **On the instrument:** `System > Basic Help` shows the core shortcuts, and
  *Shift* + *Fn* + click *Main* opens help for whatever row is highlighted.

### Practical performance

These are planning targets for dense patches, not hard limits:

| Board and Perf. Mode | Synth voices | Sample voices | Bus FX | Global FX |
|---|---:|---:|---:|---:|
| Raspberry, Latency | 32 | 16 | 8 | 2 |
| Raspberry, Capacity | 48 | 32 | 8 | 2 |
| Orange, Latency | 48 | 24 | 8 | 2 |
| Orange, Capacity | 48 | 32 | 12 | 2 |

Voice counts are totals across all eight instrument slots. Behaviors, samples
and effects all eat into the headroom, and **System > Audio > Polyphony** can
steal older voices as load rises. The menu will happily let you fill all
twelve bus FX slots on either board; this table is the polite suggestion.
The synth column comes from a load test with Polyphony off and the factory
patch playing underneath: the Raspberry held 40 synth, FM and Plucked notes
cleanly in Latency mode and started dropping out at 48; in Capacity mode, and on
the Orange in both modes, it reached 64, the engine's synth voice limit, without
a dropout. The table keeps about a quarter of that as headroom. The sample
column is still an estimate.

How quick does it feel? With `System > Audio > Perf. Mode` set to Latency, the
audio buffering adds about 4 ms on the Orange and 9 ms on the Raspberry, plus a
couple of milliseconds to read the pad. Capacity renders ahead so it can share
the work across two cores, which brings the buffering to about 12 ms on the
Orange and 17 ms on the Raspberry. That's fine for most playing, though you may
feel it on fast finger drumming. The mode change applies after a restart.

### Playing in front of people

The board images skip the operating system's daily package-list and
manual-index jobs, plus a few other chores that do nothing useful on an
instrument. They never installed anything here anyway, but on the Raspberry
they made the audio stutter for a few seconds at a random time each day.

A handful of useful housekeeping jobs still run on a timer: log rotation, SD
card trim, temp-file cleanup and, on the Raspberry, a once-a-minute Wi-Fi
health check. Each has been run during playback without disturbing the sound,
but you can postpone them with **System > Live Show Mode > Start 48h** before
a show:

- It asks first, because it also switches Wi-Fi off. No SSH, no updates and no
  Wi-Fi setup until it ends, and nothing on the network can poke the board
  mid-song. Bluetooth keyboards and speakers keep working.
- Housekeeping is postponed for 48 hours, and a reboot doesn't cancel that.
  The jobs run at their next scheduled time afterwards.
- Wi-Fi comes back by itself when the 48 hours are up. **End** finishes early
  and brings Wi-Fi straight back.
- A couple of tiny chores always keep running because things break without
  them: saving the clock and, on the Orange, trimming the in-memory logs.

To keep Wi-Fi off for good, set **System > Connections > Wi-Fi > Wi-Fi** to
Off. It stays off through reboots until you switch it back On, which also
means no SSH and no updates. Your saved network stays put, so it reconnects
straight away when you turn it back on. It's a separate switch from Live Show
Mode: a show still takes Wi-Fi down while it runs, and ending a show never
turns Wi-Fi on if you'd switched it off. Opening the Setup Portal does turn it
back on, since that's what you asked for.

The Raspberry's four cores share one fairly slow memory bus, so anything heavy
happening next to the music can cause crackles even when it runs on another
core. Do maintenance before the show, not during it: updates, `apt` over SSH,
copying big files onto the board. The Orange has a lot more memory headroom
and shrugs most of this off, but the habit doesn't hurt.

One more thing: don't run `apt upgrade` on the board. Octessera boots its own
tuned kernel and device-tree setup, and a stock kernel or bootloader package
sneaking in can leave you with a board that boots but makes no sound, or
doesn't boot at all. The boards hold those packages so apt leaves them alone;
please don't go fishing with `--allow-change-held-packages`. Octessera updates
come through **System > Maintenance > Updates** or a fresh image.

### Updates

With Wi-Fi on, **System > Maintenance > Updates > Check** tells you whether a
newer Octessera is out for your board. **Apply** downloads it, checks that the
download arrived intact (if it didn't, nothing changes), and restarts the
instrument on the new version. Give it a minute. If the new version doesn't
come up properly, the board quietly switches back to the one you had, so a
bad update costs you a restart, not an evening.

**Rollback** goes back to the version you had before the last update, and
pressing it again goes forward again. The board keeps those two versions and
tidies away older ones. One honest caveat: pull the power during that minute
of restarting and the board may wake up on the new version without having
checked it. If that version then misbehaves, Rollback (or a fresh image) puts
things right.

Updates only replace the Octessera program itself. Your patches, samples and
settings stay where they are, and the operating system, kernel and boot setup
only change when you flash a new image.

### Samples

The bundled library is about 300 WAV files from the attribution-free
[stargate-sample-pack](https://github.com/stargatedaw/stargate-sample-pack).
To add your own WAVs:

| Where | Folder |
|---|---|
| Desktop, Windows | `%APPDATA%\octessera\samples` (the browser's `User data` favourite) |
| Desktop, Linux | `~/.local/share/octessera/samples` |
| Raspberry | `/home/pi/samples` (copy over SSH) |
| Orange | `/var/lib/octessera/samples` |
| OLED microSD card (SD2) | `octessera/samples` on the card |

SD2 is the optional card in the OLED board; the compute board's boot card is
SD1. Format it as FAT32 or exFAT and name the volume `OCTESSERA` exactly, or
the instrument ignores it. You can swap the card while the instrument is on;
give it a couple of seconds to show up. It appears in the sample browser as
the `SD card` favourite.

To copy files from a computer without pulling the card, use
**System > Connections > SD Card 2 > Start Transfer** in USB Gadget mode
(Raspberry) or on the Orange. USB audio, USB MIDI and recording need to be off
first. When you're done, eject the drive on the computer, then press **Back**
or click **Main** on the instrument to end the transfer.
