# octessera user docs

Octessera is a collection of small algorithmic musical world-bubbles. Set up a
few systems, nudge them, anchor them with a little sequencing, and play the
result together with the machine.

## Release and build status

The [current release page](https://github.com/nexxyz/octessera/releases) owns
current versions, platform assets, formats, and checksums. Check it for the
desktop and board assets available for the release you selected; names and
formats may change. macOS distribution is paused until a signed and notarized
path is available; do not treat an old macOS asset as current.

Read the [release support matrix](release-support.md) before treating a download
as supported. Only an exact artifact and platform with a recorded manual FAT
result qualify; source/build checks alone do not.

Octessera documents two fixed compute-board paths: Raspberry Pi Zero 2 W and
Orange Pi Zero 2W. They share the native runtime, but their images, pinouts,
ports, and adapters are board-specific. Source and build checks are useful
evidence, not physical-board qualification.

The enclosure is currently an active v21 design and test-fit model, not a
production-final enclosure. Cost depends on the current BOM, suppliers,
shipping, taxes, and printing. There is no fixed price promise.

## Start by what you want to do

### I want to play now

Use the [hardware-free desktop simulator](desktop-simulator.md). Start with the
current [release page](https://github.com/nexxyz/octessera/releases), make a
first sound, and learn the controls without a PCB or board.

### I am building the instrument

Choose one of the two fixed board paths, then follow the [shared six-step build
journey](#shared-six-step-build-journey):

- [Raspberry Pi Zero 2 W first boot](hardware/raspberry-pi-first-boot.md)
- [Orange Pi Zero 2W first boot](hardware/orange-pi-first-boot.md)
- [Build and assembly manual](hardware/assembly-manual.md)

### I already built or flashed it

Start with [troubleshooting](troubleshooting.md), then use the matching
[Raspberry first-boot page](hardware/raspberry-pi-first-boot.md) or [Orange
first-boot page](hardware/orange-pi-first-boot.md), [setup portal
guide](hardware/setup-portal.md), or [board qualification and
status](hardware/board-qualification.md). Keep the boards accessible until the
open electrical checks pass.

### I want to learn the instrument

- [Controls cheat sheet](controls-cheat-sheet.md) — learn the five controls that
  get you moving, then keep the exact shortcut and overlay tables handy.
- [Behaviors and Play pages](behaviors-and-sparks.md) — start with a small patch,
  browse the behavior catalog, and perform with Play.

### I need a reference

- [Safety and power](hardware/safety-and-power.md) — the short owner page for
  power input, USB backfeed, orientation, and enclosure handling.
- [Pinout and connections](hardware/pinout-and-connections.md) — Raspberry
  wiring and the Orange routing warning.
- [Enclosure and print notes](hardware/enclosure.md) — board-specific openings
  and the current v21 test-fit model.
- [Setup portal](hardware/setup-portal.md) — open or reopen board setup.
- [Printable quick reference](#printable-quick-reference)

## Shared six-step build journey

The PCB and control surface are one handmade instrument; do not substitute a
board image, pin table, port role, or physical check from the other board.

### 1. Choose a board

- **Raspberry Pi Zero 2 W** — use the [Raspberry first-boot path](hardware/raspberry-pi-first-boot.md).
- **Orange Pi Zero 2W** — use the [Orange first-boot path](hardware/orange-pi-first-boot.md) and its Armbian checks.
- Read [board qualification and status](hardware/board-qualification.md) before calling a clean build a qualified instrument.

### 2. Parts and assembly

Use the [assembly manual](hardware/assembly-manual.md#bom) and [board-specific
pinout references](hardware/pinout-and-connections.md#board-profile-first) while
ordering parts, soldering, and checking the open assembly. Read [safety and
power](hardware/safety-and-power.md) before connecting power or a host cable.

### 3. Flash the selected board

Flash the matching image from the [current release page](https://github.com/nexxyz/octessera/releases).
The [assembly manual's flash step](hardware/assembly-manual.md#flash-the-selected-board-image)
links to both first-boot workflows and their image/checksum instructions.

### 4. Bench bring-up

Bring the device up while the boards are still accessible. Use the [Raspberry
first-boot page](hardware/raspberry-pi-first-boot.md), or the [Orange final
bench bring-up checklist](hardware/orange-pi-first-boot.md#oled-usb-and-final-bench-checks).
Stop at an unresolved physical gate; a successful source check is not permission
to close the case.

### 5. Enclosure

After the open electrical checks pass, use the [enclosure and print
notes](hardware/enclosure.md) and the fit sequence in the [assembly
manual](hardware/assembly-manual.md#enclosure-assembly). Remove the selected
board's microSD card and the OLED microSD card before putting the boards in the
case.

### 6. Final checks

Run the [final checks](hardware/assembly-manual.md#final-checks): power, display,
audio, every control, and access to the ports. If anything is unclear, use the
[symptom router](troubleshooting.md) before continuing.

## Samples and OLED SD storage

The repository's complete attribution inventory has 320 rows. The
sampler-loadable default library contains 318 WAV rows; two AIFF rows remain in
the inventory and are outside the WAV-only browser/decoder. The portable desktop
package and the constructors for both production images stage the complete
320-file artifact inventory. That is a build/staging contract, not a
physical-board FAT result. You can add your own samples through the desktop
host/sample browser or the board sample paths. First boot only seeds a missing
default and does not replace user samples.

For the optional OLED microSD card, label the card `OCTESSERA_SD`. This is SD2;
the selected board's boot card is SD1. Octessera mounts SD2 at `SD card` and
creates `octessera/samples` plus `octessera/saves`; put WAV samples under
`octessera/samples`. If you use `System > Audio & USB > Start SD2 Xfer`, eject
the drive on the host before pressing Back or Main to stop transfer. If no host
is connected yet, Octessera waits until one appears and you can still cancel
from the popup.

## Printable quick reference

- [Two-page controls, behaviors, Play, and signal-flow PDF](print/quick-reference.pdf)
- [Printable sources](print/) — HTML, CSS, and the signal-flow SVG.

## Canonical references

The friendly pages above are for people at the workbench. Exact runtime
contracts live in the source references:

- [Menu and controls spec](../docs/menu-and-controls-spec.md)
- [Menu tree spec](../docs/menu-tree-spec.md)
- [Behavior source](../crates/platform-core/src/behaviors/)
- [Project license](../LICENSE)
- [Samples attribution inventory](../samples/ATTRIBUTIONS.tsv)
- [Hardware attributions](../hardware/ATTRIBUTIONS.md)

If a friendly page and a canonical specification disagree, the specification
wins and the friendly page needs updating.
