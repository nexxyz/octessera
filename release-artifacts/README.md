# Release Artifacts

This directory contains files intended for builders and end users, not source-of-truth project files.

- `desktop/` — downloadable desktop builds when intentionally published.
- `pi/` — Pi images or Pi binary packages when intentionally published.
- `pcb/` — PCB fabrication exports such as Gerber zips.
- `enclosure/` — printable STL files and exported STEP files.

Versioned release mirrors live under `v<version>/`, for example `v0.5.1/pi/`.
Keep only publishable artifacts in versioned folders; temporary CI run imports or extracted images should not be committed.

Regenerate these files from the source tree before publishing a release.

## Enclosure board naming

Top enclosure artifact filenames include the full board name. The shorthand `rpi` means Raspberry Pi Zero 2 W and `opi` means Orange Pi Zero 2W; use those only in prose or table labels.

| Board | Top enclosure artifacts |
| --- | --- |
| Raspberry Pi Zero 2 W (`rpi`) | `case_top_two_level_cadquery_raspberry-pi-zero-2w.{step,stl}`; `case_top_two_level_raspberry-pi-zero-2w_multicolor.3mf` |
| Orange Pi Zero 2W (`opi`) | `case_top_two_level_cadquery_orange-pi-zero-2w.{step,stl}`; `case_top_two_level_orange-pi-zero-2w_multicolor.3mf` |
