# Release Artifacts

This directory contains generated deliverables for builders and end users, not
source-of-truth project files. The currently committed fabrication tree is:

- `pcb/gerber/` — Gerbers, drill files, the KiCad job file, and `gerber.zip`.
- `enclosure/stl/` — printable STL exports.
- `enclosure/step/` — CAD STEP exports.
- `enclosure/3mf-multicolor/` — multicolor-print exports.

Generated release surfaces such as `desktop/`, `pi/`, `v<version>/`, evidence,
and checksums are not a second source tree. Do not commit temporary CI imports
or extracted images. Regenerate committed fabrication exports from the source
tree before publishing.

## Legal and source companions

Release artifacts should link or ship the applicable [`NOTICE`](../NOTICE),
[`THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md),
[`samples/ATTRIBUTIONS.tsv`](../samples/ATTRIBUTIONS.tsv), and
[`hardware/ATTRIBUTIONS.md`](../hardware/ATTRIBUTIONS.md). The release policy
is [`docs/release-licensing.md`](../docs/release-licensing.md).

Attribution notices and pinned upstream source references are maintained with
the project. Octessera source, configuration, and image patches are in the
repository. Applicable source duties need review before any future public image
release; these records make no legal-compliance claim. The release workflow
checks the Windows portable notice bundle and puts the generated legal bundle in
the release evidence ZIP. Board device ZIPs carry exact root-level `LICENSE` and
`NOTICE` files.

## GitHub release surface

The committed release procedure and exact current 12-asset custom release
contract live in [`docs/development-workflows.md`](../docs/development-workflows.md).
It also explains the operational Imager manifest, supporting evidence ZIP,
global checksum, automatic GitHub source archives, and the legacy Raspberry
updater checksum. macOS distribution remains paused until it can be properly
signed and notarized, so it is not a published asset.

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
