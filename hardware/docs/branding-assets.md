# Branding assets

This project uses the octessera mark and wordmark across hardware docs, PCB silkscreen, enclosure CAD, Pi splash screens, Raspberry Pi Imager metadata, and the desktop hardware simulator icon.

## Source assets

- `assets/octessera-mark.svg`: vector mark source.
- `assets/octessera-wordmark.svg`: grid-native vector wordmark source.
- `tools/assets/generate_pi_logo_pngs.py`: generates Pi splash and Imager PNG assets.
- `apps/desktop/src-tauri/icons/`: generated desktop simulator icon assets.
- `hardware/enclosure/branding_marking_cadquery.py`: converts the SVG mark and wordmark into CadQuery solids.
- `hardware/pcb/octessera.kicad_pcb`: contains the PCB silkscreen branding geometry.

Keep the SVGs as the source of truth. Do not hand-edit generated PNGs, STL files, STEP files, or 3MF files as source.

## Fill rules

The wordmark SVG is built from filled block paths. When rasterizing or converting it, use union-fill semantics: a point is filled if it is inside any wordmark path.

Do not use even-odd/parity fill for the current wordmark. Even-odd fill can cancel overlapping or touching block paths and can remove letter joins.

If a future wordmark uses compound paths with holes, update the source model explicitly instead of silently changing all converters to parity fill.

## Pi PNGs and initramfs

Run this after changing the mark or wordmark SVG:

```powershell
python tools/assets/generate_pi_logo_pngs.py
```

Generated PNGs:

- `assets/octessera-pi-manifest.png`: Raspberry Pi Imager icon.
- `assets/octessera-app-large.png`: desktop app icon source.
- `assets/octessera-pi-booting.png`: Pi boot splash.
- `assets/octessera-pi-sleeping.png`: sleep splash mark.
- `assets/octessera-pi-shutdown.png`: shutdown splash mark.
- `apps/desktop/src-tauri/icons/icon.png`: desktop hardware simulator icon.
- `apps/desktop/src-tauri/icons/icon.ico`: Windows desktop hardware simulator icon.

The Pi build embeds the PNGs through `apps/pi-zero/build.rs`, which writes RGB565 splash assets into Cargo `OUT_DIR`. Rebuild the Pi binary or Pi image/initramfs after changing these PNGs.

The Orange Armbian image keeps the SVG sources as `/usr/share/octessera/oled/`
assets and rasterizes the same mark/wordmark through its board-specific
`octessera-orange-oled-logo` utility for initramfs boot, sleep, resume, and
shutdown handoff. It does not use Raspberry `rppal`, BCM GPIOs, or the Pi
binary's Cargo-generated splash assets.

## Canonical OLED boot sweep

`resources/oled/boot-sweep-v1.json` is the visual contract for the Phase 5 boot
sweep on both fixed boards. It is strict: unknown and missing keys are rejected.
The contract describes a 128×128 physical post-rotation frame with rightward X
travel and bottom-to-top Y coordinates. Only source-white RGB565 pixels
(`FFFF`) may be recolored; every other source pixel is preserved.

The moving train is four 8 px bands, in order:

1. cyan (`07FF`)
2. yellow (`FFE0`)
3. green (`07E0`)
4. magenta (`F81F`)

The 32 px train leans +8 px toward the top-right using
`floor(row_y * 8 / 127)`. It travels from a bottom-row origin of `-40` to
`128` across 24 frames. Frames 0 and 23 are intentionally blank endpoint
frames. The cycle uses absolute one-second deadlines, wraps directly from
frame 23 to frame 0, and inserts no extra pause or cumulative-sleep drift.

This is implemented in the current source for Raspberry and Orange, but the
boot-layer inputs are constructor-required. Do not describe the sweep as
shipped or physically qualified until new constructor images have been built
and the mounted-image and hardware checks are complete.

## Enclosure branding

The enclosure top uses `hardware/enclosure/branding_marking_cadquery.py`.

- STEP/STL exports keep branding as raised separate solids.
- The branded multicolor 3MF keeps the branding flush with the top surface and assigns it to extruder 2.

Use the branded top wrapper after changing branding or top enclosure CAD:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File hardware/enclosure/generate_branded_top_artifacts_checked.ps1
```

Expected outputs:

- `release-artifacts/enclosure/step/case_top_two_level_cadquery_raspberry-pi-zero-2w.step`
- `release-artifacts/enclosure/stl/case_top_two_level_cadquery_raspberry-pi-zero-2w.stl`
- `release-artifacts/enclosure/3mf-multicolor/case_top_two_level_raspberry-pi-zero-2w_multicolor.3mf`
- ignored review images under `hardware/enclosure/review/`

Do not restore the old unbranded top 3MF; the top 3MF should be multicolor only.

## PCB branding

The PCB silkscreen branding lives in `hardware/pcb/octessera.kicad_pcb` on `F.SilkS`.

Use `assets/octessera-mark.svg` and `assets/octessera-wordmark.svg` as the basis when regenerating it. Preserve the manually tuned placement unless intentionally changing the PCB layout.

After editing generated PCB graphic primitives, check:

- parentheses are balanced;
- UUIDs are unique;
- no local absolute paths are introduced;
- `by nexxyz` remains present if the layout still uses the byline.

## Cleanup checklist

Before committing branding or hardware artifact changes, check:

- no local Windows absolute paths in tracked project files;
- KiCad libraries live under `hardware/pcb/kicad-libs/`;
- review images are under ignored `hardware/enclosure/review/`, not `release-artifacts/`;
- the old unbranded top 3MF is absent;
- generated Python `__pycache__/` directories are not staged;
- `git diff --check` passes.
