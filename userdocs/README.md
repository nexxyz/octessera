# octessera user docs

Octessera is a little box of algorithmic music systems.

Instead of drawing fixed notes on a grid or a piano roll, you set up small self-contained systems: cellular automata, bouncing particles, raindrops, loops, keys, and shapes. Each one has its own rules. Each one produces music in a slightly different, slightly unpredictable way.

Then you nudge them. You anchor them with a bit of manual sequencing if you want. You add probability so the pattern breathes. You grab a Play page and play the machine in real time. The result is not only what you wrote, and not only what octessera generated. It is what the two of you found together.

## Start here

- The full hardware device can be built for well under €200 through suppliers such as Mouser, even after accounting for the small stuff: sockets, pin headers, wire, solder, screws, and a sensible amount of 3D-printing filament.
- [Build and assembly manual](hardware/assembly-manual.md) — parts, soldering, enclosure, and the bits where I try to keep you from breaking the same things I broke.
- [Controls cheat sheet](controls-cheat-sheet.md) — what the encoders, buttons, grid, modifiers, Play pages, and auto-maps do.
- [Behaviors and Play pages](behaviors-and-sparks.md) — the layer behaviors and live performance pages.
- [Pinout and connections](hardware/pinout-and-connections.md) — wiring and pin ownership.
- [Enclosure and print notes](hardware/enclosure.md) — case files, ports, power, and print-fit notes.
- [Orange Pi first boot setup](hardware/orange-pi-first-boot.md) — Wi-Fi and SSH setup for the Armbian image.

## Printable quick reference

- [Two-page controls, behaviors, Play, and flowchart PDF](print/quick-reference.pdf)
- HTML sources are in [`print/`](print/) if you want to print or tweak them yourself.

## OLED SD card samples

For the optional OLED microSD card, label the card `OCTESSERA_SD`. This is SD2. The Pi boot card is SD1. Octessera mounts SD2 at `SD card` and creates `octessera/samples` plus `octessera/saves`; put WAV samples under `octessera/samples`. If you use `System > USB > Start SD2 Xfer`, eject the drive on the host before pressing Back or Main to stop transfer. If no host is connected yet, Octessera waits until one appears and you can still cancel from the popup. Tiny storage goblin, ordinary safe-eject rules.

## Canonical specs

The friendly pages above are meant for humans at the workbench. The exact runtime contracts live in the source specs:

- [Menu and controls spec](../docs/menu-and-controls-spec.md)
- [Menu tree spec](../docs/menu-tree-spec.md)
- [Behavior source](../crates/platform-core/src/behaviors/)

If the friendly docs and the specs disagree, the specs win and the friendly docs need updating.
