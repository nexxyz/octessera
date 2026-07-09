# Branding assets

This project uses the Octessera mark and wordmark across hardware docs, PCB silkscreen, enclosure CAD, Pi splash screens, Raspberry Pi Imager metadata, and the desktop hardware simulator icon.

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
- `assets/octessera-pi-booting.png`: Pi boot splash.
- `assets/octessera-pi-sleeping.png`: sleep splash mark.
- `assets/octessera-pi-shutdown.png`: shutdown splash mark.
- `apps/desktop/src-tauri/icons/icon.png`: desktop hardware simulator icon.
- `apps/desktop/src-tauri/icons/icon.ico`: Windows desktop hardware simulator icon.

The Pi build embeds the PNGs through `apps/pi-zero/build.rs`, which writes RGB565 splash assets into Cargo `OUT_DIR`. Rebuild the Pi binary or Pi image/initramfs after changing these PNGs.

## Enclosure branding

The enclosure top uses `hardware/enclosure/branding_marking_cadquery.py`.

- STEP/STL exports keep branding as raised separate solids.
- The branded multicolor 3MF keeps the branding flush with the top surface and assigns it to extruder 2.

Use the branded top wrapper after changing branding or top enclosure CAD:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File hardware/enclosure/generate_branded_top_artifacts_checked.ps1
```

Expected outputs:

- `release-artifacts/enclosure/step/case_top_two_level_cadquery.step`
- `release-artifacts/enclosure/stl/case_top_two_level_cadquery.stl`
- `release-artifacts/enclosure/3mf-multicolor/case_top_two_level_branded_multicolor.3mf`
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
