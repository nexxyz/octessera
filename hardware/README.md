# Hardware

This is the home for Octessera's canonical hardware sources and committed
fabrication outputs.

## Navigate the hardware tree

- [`hardware/docs/`](docs/) contains build, bring-up, and hardware reference documents.
- [`hardware/pcb/`](pcb/) is the canonical KiCad source and library tree.
- [`hardware/enclosure/`](enclosure/) is the canonical CadQuery, generator, test, wrapper,
  and workflow source tree.
- The [hardware build entry](../userdocs/hardware/assembly-manual.md) walks through
  the shared assembly path.

## PCB outputs

[`hardware/pcb/gerber/`](pcb/gerber/) contains the committed fabrication output: Gerbers,
drill files, the KiCad job file, and [`gerber.zip`](pcb/gerber/gerber.zip).

## Enclosure outputs

The enclosure source tree includes
[`CAD_WORKFLOW.md`](enclosure/CAD_WORKFLOW.md) and the CadQuery generators,
tests, and wrappers. Its committed generated output sets are:

- [`hardware/enclosure/step/`](enclosure/step/) — exactly 18 STEP files;
- [`hardware/enclosure/stl/`](enclosure/stl/) — exactly 18 printable STL files;
- [`hardware/enclosure/3mf-single-material/`](enclosure/3mf-single-material/) — exactly 18 single-material 3MF files;
- [`hardware/enclosure/3mf-multicolor/`](enclosure/3mf-multicolor/) — exactly 18 3MF files.

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
- [`samples/SOURCE.md`](../samples/SOURCE.md)

## Release assets

GitHub Desktop and Pi release assets are built by the release workflows and are
not stored here. This tree contains the canonical hardware sources and
committed fabrication outputs only.
