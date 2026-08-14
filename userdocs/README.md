# octessera user docs

Octessera is a little box of algorithmic music systems.

Instead of drawing fixed notes on a grid or a piano roll, you set up small self-contained systems: cellular automata, bouncing particles, raindrops, loops, keys, and shapes. Each one has its own rules. Each one produces music in a slightly different, slightly unpredictable way.

Then you nudge them. You anchor them with a bit of manual sequencing if you want. You add probability so the pattern breathes. You grab a Play page and play the machine in real time. The result is not only what you wrote, and not only what octessera generated. It is what the two of you found together.

## Start here

- [Hardware-free desktop simulator](desktop-simulator.md) — download a release build or run from a checkout, make a first patch, and see what the simulator cannot qualify.
- The full hardware device can be built for well under €200 through suppliers such as Mouser, even after accounting for the small stuff: sockets, pin headers, wire, solder, screws, and a sensible amount of 3D-printing filament.
- [Board qualification and status](hardware/board-qualification.md) — see what source and build checks prove, and what still needs a real board on the bench.
- [Build and assembly manual](hardware/assembly-manual.md) — parts, soldering, enclosure, and the bits where I try to keep you from breaking the same things I broke.
- [Controls cheat sheet](controls-cheat-sheet.md) — what the encoders, buttons, grid, modifiers, Play pages, and auto-maps do.
- [Behaviors and Play pages](behaviors-and-sparks.md) — the layer behaviors and live performance pages.
- [Pinout and connections](hardware/pinout-and-connections.md) — wiring and pin ownership.
- [Enclosure and print notes](hardware/enclosure.md) — case files, ports, power, and print-fit notes.
- [Orange Pi first boot setup](hardware/orange-pi-first-boot.md) — Wi-Fi and SSH setup for the Armbian image.
- [Raspberry Pi first boot and OLED handoff](hardware/raspberry-pi-first-boot.md) — constructor boot behavior, terminal welcome, and serial ownership.
- [Open or reopen the full setup portal](hardware/setup-portal.md) — the menu-driven setup flow for either board.

## Build journey

Choose one of the two fixed board paths, then follow the shared build order. The
PCB and control surface are one handmade instrument; the board profile, image,
pinout, and physical checks are not interchangeable.

### 1. Choose a board

- **Raspberry Pi Zero 2 W** — follow the [Raspberry first-boot path](hardware/raspberry-pi-first-boot.md).
- **Orange Pi Zero 2W** — follow the [Orange first-boot path](hardware/orange-pi-first-boot.md) and its Armbian checks.
- Read the [board qualification and status page](hardware/board-qualification.md) before treating a clean build as a qualified instrument.

### 2. Parts and assembly

Use the [assembly manual](hardware/assembly-manual.md#bom) and [board-specific
pinout references](hardware/pinout-and-connections.md#board-profile-first) while
ordering parts, soldering, and checking the open assembly.

### 3. Flash the selected board

Flash the matching image for the board you chose. The [assembly manual's flash
step](hardware/assembly-manual.md#flash-the-selected-board-image) links to both
first-boot workflows and their image/checksum instructions.

### 4. Bench bring-up

Bring the device up while the boards are still accessible. Use the [Raspberry
first-boot page](hardware/raspberry-pi-first-boot.md), or the [Orange final
bring-up checklist](hardware/orange-pi-first-boot.md#final-bench-bring-up-checklist).
Stop at an unresolved physical gate; do not let a successful source check bully
you into closing the case.

### 5. Enclosure

After the open electrical checks pass, use the [enclosure and print
notes](hardware/enclosure.md) and the fit sequence in the [assembly
manual](hardware/assembly-manual.md#enclosure-assembly). Remove both microSD
cards before the boards go into the case.

### 6. Final checks

Run the [final checks](hardware/assembly-manual.md#final-checks): power, display,
audio, every control, and access to the ports.

## Printable quick reference

- [Two-page controls, behaviors, Play, and flowchart PDF](print/quick-reference.pdf)
- HTML sources are in [`print/`](print/) if you want to print or tweak them yourself.

## OLED SD card samples

For the optional OLED microSD card, label the card `OCTESSERA_SD`. This is SD2. The Pi boot card is SD1. Octessera mounts SD2 at `SD card` and creates `octessera/samples` plus `octessera/saves`; put WAV samples under `octessera/samples`. If you use `System > Audio & USB > Start SD2 Xfer`, eject the drive on the host before pressing Back or Main to stop transfer. If no host is connected yet, Octessera waits until one appears and you can still cancel from the popup. Tiny storage goblin, ordinary safe-eject rules.

## Canonical specs

The friendly pages above are meant for humans at the workbench. The exact runtime contracts live in the source specs:

- [Menu and controls spec](../docs/menu-and-controls-spec.md)
- [Menu tree spec](../docs/menu-tree-spec.md)
- [Behavior source](../crates/platform-core/src/behaviors/)

If the friendly docs and the specs disagree, the specs win and the friendly docs need updating.

## Attribution and release notes

- [Project license](../LICENSE) — original Octessera material and scope.
- [Samples attribution inventory](../samples/ATTRIBUTIONS.tsv) — pinned media paths, hashes, and upstream terms.
- [Hardware attributions](../hardware/ATTRIBUTIONS.md) — enclosure and PCB source notes.
- [Release licensing and source policy](../docs/release-licensing.md) — what a public board-image release must carry.
