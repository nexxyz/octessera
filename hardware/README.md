# Hardware

This is the home for Octessera's canonical hardware sources and committed
fabrication outputs. Building one? Start with the [assembly
manual](../userdocs/hardware/assembly-manual.md) instead; this tree is the
workshop behind it.

## What's in here

- [`hardware/pcb/`](pcb/) is the canonical KiCad source and library tree.
- [`hardware/enclosure/`](enclosure/) is the canonical CadQuery, generator, test, wrapper,
  and workflow source tree.
- [`hardware/docs/`](docs/) holds the technical references:
  - [Pinout and connections](docs/pinout-and-connections.md): PCB nets, buses,
    addresses and the `J1` cable.
  - [Manual hardware test suite](docs/manual-hardware-test-suite.md):
    Raspberry bench bring-up without the OLED.
  - [Orange Pi Armbian bring-up](docs/orange-pi-armbian-bringup.md), the
    [Orange production reference](docs/orange-pi-production-reference.md) and
    [Orange UART0 input routing](docs/orange-pi-input-routing.md).
  - [Branding assets](docs/branding-assets.md): logo sources and where they end
    up.

## PCB outputs

[`hardware/pcb/gerber/`](pcb/gerber/) contains the committed fabrication output: Gerbers,
drill files, the KiCad job file, and [`gerber.zip`](pcb/gerber/gerber.zip).

## Enclosure outputs

The enclosure source tree includes
[`CAD_WORKFLOW.md`](enclosure/CAD_WORKFLOW.md) and the CadQuery generators,
tests, and wrappers. Its committed generated output sets are:

- [`hardware/enclosure/step/`](enclosure/step/): exactly 18 STEP files;
- [`hardware/enclosure/stl/`](enclosure/stl/): exactly 18 printable STL files;
- [`hardware/enclosure/3mf-single-material/`](enclosure/3mf-single-material/): exactly 18 single-material 3MF files;
- [`hardware/enclosure/3mf-multicolor/`](enclosure/3mf-multicolor/): exactly 18 3MF files.

The multicolor set is a complete print set: 12 authored two-material designs
plus six one-material support pieces.

Protective-case filenames are canonical and statusless (`protective_case_*`).
The design remains an unqualified check-fit prototype, not a drop-rated,
waterproof, or transport-qualified product.

## Attribution and notices

Consult these companions as applicable:

- [`hardware/ATTRIBUTIONS.md`](ATTRIBUTIONS.md)
- [`NOTICE`](../NOTICE)
- [`THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md)
- [`samples/upstream/LICENSE`](../samples/upstream/LICENSE)

## Release assets

GitHub Desktop and Pi release assets are built by the release workflows and are
not stored here. This tree contains the canonical hardware sources and
committed fabrication outputs only.
