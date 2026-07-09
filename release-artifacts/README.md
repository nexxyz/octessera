# Release Artifacts

This directory contains files intended for builders and end users, not source-of-truth project files.

- `desktop/` — downloadable desktop builds when intentionally published.
- `pi/` — Pi images or Pi binary packages when intentionally published.
- `pcb/` — PCB fabrication exports such as Gerber zips.
- `enclosure/` — printable STL files and exported STEP files.

Versioned release mirrors live under `v<version>/`, for example `v0.5.1/pi/`.
Keep only publishable artifacts in versioned folders; temporary CI run imports or extracted images should not be committed.

Regenerate these files from the source tree before publishing a release.
