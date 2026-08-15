# Release Artifacts

This directory contains files intended for builders and end users, not source-of-truth project files.

- `desktop/` — downloadable desktop builds when intentionally published.
- `pi/` — Pi images or Pi binary packages when intentionally published.
- `pcb/` — PCB fabrication exports such as Gerber zips.
- `enclosure/` — printable STL files and exported STEP files.

Versioned release mirrors live under `v<version>/`, for example `v0.5.1/pi/`.
Keep only publishable artifacts in versioned folders; temporary CI run imports or extracted images should not be committed.

Regenerate these files from the source tree before publishing a release.

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

The publisher keeps exactly 13 custom assets at the release root: the Windows
installer and portable ZIP, unsigned macOS DMG, Ubuntu DEB and AppImage,
Raspberry image ZIP, operational `.rpi-imager-manifest`, Raspberry updater ZIP,
the legacy `SHA256SUMS-raspberry-pi-zero-2w-device.txt`, Orange image and
standalone-manual ZIP, `octessera-<version>-release-evidence.zip`, and
`SHA256SUMS.txt`. GitHub also shows its automatic source ZIP and tar archives;
those are not custom assets and are not included in `SHA256SUMS.txt`.

`SHA256SUMS.txt` covers the other 12 custom root assets. The Raspberry Imager
manifest is operational metadata for Imager, while the release evidence ZIP is
supporting build material rather than another install payload. The legacy
Raspberry device checksum remains at the root only for existing installed
updater clients.

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
