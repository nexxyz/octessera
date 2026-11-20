# Octessera user manual

Octessera is a collection of small algorithmic musical world-bubbles. Each
layer runs its own little system (a Game of Life colony, a flock of boids, a
slow-growing crystal, a step sequencer you poke by hand), and you set them up,
nudge them, anchor them with a bit of sequencing and probability, and play
along. You aren't writing notes into a piano roll. You're co-writing with a
handful of tiny worlds that sometimes have opinions.

This page is the front door. Everything else is linked from here, and the
[glossary](#glossary) explains the words the other pages take for granted.

## 1. Try the simulator

The [desktop simulator](desktop-simulator.md) runs the same native engine as
the hardware. It needs no PCB, board, OLED or soldering iron, so it's the
cheapest way to find out whether you like this sort of thing.

**Next:** [your first ten minutes](first-ten-minutes.md)

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

The shipped patch is wired up and ready. [Your first ten
minutes](first-ten-minutes.md) walks you through it: press Play, touch the
grid, wake the stars, ride the faders, change a sound and the tempo, then keep
it or throw it away and switch off.

Where to go next:

- [Controls cheat sheet](controls-cheat-sheet.md): the buttons and combos on
  one page, with the [controls reference](controls-reference.md) for the fine
  print.
- [Behaviors and Play pages](behaviors-and-play.md): about 50 worlds and the
  performance pages.
- [Sound and Link](sound-and-link.md): how tile changes become notes,
  instruments and effects.
- [Recording](recording.md): capture audio or audio plus video.

**Something wrong?** Start with [troubleshooting](troubleshooting.md). Power
down before opening the case or moving wiring.

## Reference

### Contents

- **Playing:** [First ten minutes](first-ten-minutes.md),
  [Controls cheat sheet](controls-cheat-sheet.md),
  [Controls reference](controls-reference.md),
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
  [Tile-to-audio flowchart](print/tile-to-audio-flow.svg). The
  [controls cheat sheet](controls-cheat-sheet.md) also prints fine.
- **On the instrument:** `System > Basic Help` shows the core shortcuts, and
  *Shift* + *Fn* + click *Main* opens help for whatever row is highlighted.

### Glossary

| Word | What it means here |
|---|---|
| Patch | Everything that makes the music right now: layers, Link, instruments, effects and Aux bindings. Device settings (display, MIDI, USB, audio) aren't part of it. |
| Preset | A patch saved under a name in `System > Saves > Library`. An instrument preset (`Preset > Load` on a Synth, FM or Plucked) only swaps that one sound. |
| Layer | One of eight little worlds, L1 to L8, each with its own behavior and its own Link settings. *Fn* + the left grid column picks one. |
| Slot | A numbered place for something: instrument slots I1 to I8, three effect slots per FX bus, eight sample slots per Sampler. |
| Main, Aux | *Main* is the big knob that moves through menus. *Aux 1* to *3* are the three smaller ones; each turn and click does whatever is bound to it on that page. |
| Bus | One of four shared FX chains (B1 to B4, `fxb1` to `fxb4` in Route). Any instrument routed to it goes through its three slots. |
| Duck | An effect that turns its bus down whenever another sound gets loud: the pumping trick. |
| Stamp | A Drum voice or Sampler sample written onto one tile, so that tile always plays it. |
| Toast | A short message that pops up on the screen for a moment. |
| Detent | One click of a knob as you turn it. |

### Care and housekeeping

- [Performance and live shows](performance-and-live.md): how many voices a
  patch can hold, how quick it feels, and Live Show Mode.
- [Updates](updates.md): Check, Apply and Rollback, and what each one keeps.
- [Samples](samples.md): adding your own WAVs and using the second SD card.
