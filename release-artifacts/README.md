# Release Artifacts

This directory contains generated deliverables for builders and end users, not
source-of-truth project files. The committed fabrication tree is:

- `pcb/gerber/` — Gerbers, drill files, the KiCad job file, and `gerber.zip`.
- `enclosure/stl/` — printable STL exports.
- `enclosure/step/` — CAD STEP exports.
- `enclosure/3mf-multicolor/` — multicolor-print exports.

Generated release surfaces such as `desktop/`, `pi/`, `v<version>/`, and
checksums are not a second source tree. Regenerate committed fabrication
exports from the source tree.

## Legal and source companions

Release artifacts link or ship the applicable [`NOTICE`](../NOTICE),
[`THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md),
[`samples/SOURCE.md`](../samples/SOURCE.md),
[`samples/MANIFEST.tsv`](../samples/MANIFEST.tsv), and
[`hardware/ATTRIBUTIONS.md`](../hardware/ATTRIBUTIONS.md).

The sample acknowledgement and CC0 text are maintained with the project.
Octessera source, configuration, and image patches remain in the repository.
Board device ZIPs carry root-level `LICENSE` and `NOTICE` files.

## Artifact-surface naming

The artifact surface is organized by deliverable rather than by source package:
`desktop/` contains Desktop Simulator builds, `pi/` contains shared hardware-host
runtime packages and board images, `pcb/` contains fabrication exports, and
`enclosure/` contains printable and STEP exports. Board-specific artifacts use the
canonical `raspberry-pi-zero-2w` or `orange-pi-zero-2w` profile name.

`octessera-pi` is the compatibility runtime filename used by both board variants;
it is not a board identity. Use the profile-qualified artifact name and metadata to
identify the target board.

## Enclosure board naming

Top enclosure artifact filenames include the full board name. The shorthand `rpi` means Raspberry Pi Zero 2 W and `opi` means Orange Pi Zero 2W; use those only in prose or table labels.

| Board | Top enclosure artifacts |
| --- | --- |
| Raspberry Pi Zero 2 W (`rpi`) | `case_top_two_level_cadquery_raspberry-pi-zero-2w.{step,stl}`; `case_top_two_level_raspberry-pi-zero-2w_multicolor.3mf` |
| Orange Pi Zero 2W (`opi`) | `case_top_two_level_cadquery_orange-pi-zero-2w.{step,stl}`; `case_top_two_level_orange-pi-zero-2w_multicolor.3mf` |
