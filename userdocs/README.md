# octessera user docs

Octessera is a little box of algorithmic music systems.

Instead of drawing fixed notes on a grid or a piano roll, you set up small self-contained systems: cellular automata, bouncing particles, raindrops, loops, keys, and shapes. Each one has its own rules. Each one produces music in a slightly different, slightly unpredictable way.

Then you nudge them. You anchor them with a bit of manual sequencing if you want. You add probability so the pattern breathes. You grab a Sparks page and play the machine in real time. The result is not only what you wrote, and not only what octessera generated. It is what the two of you found together.

## Start here

- [Build and assembly manual](hardware/assembly-manual.md) — parts, soldering, enclosure, and the bits where I try to keep you from breaking the same things I broke.
- [Controls cheat sheet](controls-cheat-sheet.md) — what the encoders, buttons, grid, modifiers, Sparks pages, and auto-maps do.
- [Behaviors and Sparks pages](behaviors-and-sparks.md) — the layer behaviors and live performance pages.
- [Pinout and connections](hardware/pinout-and-connections.md) — wiring and pin ownership.
- [Enclosure and print notes](hardware/enclosure.md) — case files, ports, power, and print-fit notes.

## Printable quick reference

- [Two-page controls, behaviors, Sparks, and flowchart PDF](print/quick-reference.pdf)
- HTML sources are in [`print/`](print/) if you want to print or tweak them yourself.

## Canonical specs

The friendly pages above are meant for humans at the workbench. The exact runtime contracts live in the source specs:

- [Menu and controls spec](../docs/menu-and-controls-spec.md)
- [Menu tree spec](../docs/menu-tree-spec.md)
- [Behavior source](../crates/platform-core/src/behaviors/)

If the friendly docs and the specs disagree, the specs win and the friendly docs need updating.
