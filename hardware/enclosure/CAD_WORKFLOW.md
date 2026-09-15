# Enclosure CAD workflow

Use the CadQuery generator as the source of truth for the two-level faceplate.

## Source ownership

`generate_two_level_enclosure_cadquery.py` remains the preferred source-of-truth
entrypoint. Its domain modules keep the same solids and measurements while
making ownership explicit:

- `top_wave_geometry.py` owns the raised roof, shoulders, tier transition
  ramps, wave walls, and ventilation slots.
- `top_body_assembly.py` owns the tier plates, perimeter skirts, support
  assembly, and final body union/cut sequence.
- `top_faceplate_features.py` owns OLED, encoder, NeoKey, and NeoTrellis
  faceplate features.
- `top_branding_variants.py` owns branding solids and board-specific variants.
- `top_enclosure_export.py` owns branded model composition and STEP/STL export.
- `top_wall_port_geometry.py`, `top_wall_port_indent_geometry.py`, and
  `top_wall_port_recess_geometry.py` own reusable wall-port profiles, indent
  transitions, and recess solids.
- `top_wall_port_cutouts.py` owns the fixed v21 board/port layout policy and
  applies those reusable solids without changing their coordinates.

Fit-critical case, PCB, and port spans are sourced from the checked-in v21
parameters in `enclosure_params.json`. The fixed OLED microSD opening policy is
owned by `top_wall_port_cutouts.py`; do not change measured or checked-in layout
values without an intentional hardware-layout change.

## Edit loop

1. Edit `wave_guidance.py`.
   - It defines the raised Pi roof block, quarter-circle slope edges, and S-shaped ventilation slots.
   - Keep the raised block hollow below the roof slab for Pi airflow and top-side components.
   - Keep the S-shaped slot definitions parametric in this module.
2. Regenerate the model:

   ```sh
   python hardware/enclosure/generate_two_level_enclosure_cadquery.py
   ```

   On Windows, use the checked wrapper, or use the async wrapper for a detached
   export:

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File hardware/enclosure/generate_top_artifacts_async.ps1
   powershell -NoProfile -ExecutionPolicy Bypass -File hardware/enclosure/top_artifacts_async_status.ps1
   ```

   The async status command reads the worker log and reports the final state and
   generation sentinels. The blocking checked wrapper is available for manual
   terminal use:

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File hardware/enclosure/generate_top_artifacts_checked.ps1
   ```

   It runs generation and checks as child processes and prints completion
   sentinels after each step.

For branded top changes, prefer the async wrapper that also regenerates the flush multicolor 3MF:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File hardware/enclosure/generate_branded_top_artifacts_async.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File hardware/enclosure/branded_top_artifacts_async_status.ps1
```

The async worker runs the checked wrapper and writes the generation log.

The blocking checked wrapper is still available for manual terminal use:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File hardware/enclosure/generate_branded_top_artifacts_checked.ps1
```

Expected additional sentinels:

```text
__BRANDED_3MF_DONE__
__BRANDED_TOP_ARTIFACTS_DONE__
```

For this branded wrapper, `__BRANDED_TOP_ARTIFACTS_DONE__` is the final
sentinel.

3. Run the roof-wall checks:

   ```sh
   python hardware/enclosure/validate_wave_roof.py
   ```

4. Inspect or slice `../../release-artifacts/enclosure/stl/case_top_two_level_cadquery_raspberry-pi-zero-2w.stl` before using it for printing.

## Geometry change checklist

Use this checklist before changing generated solids, Z transitions, or board-adjacent surfaces:

- Map relevant feature coordinates from `enclosure_params.json` before editing.
- Decide which solid owns each top surface: tier 1, tier 2, shoulder, ramp, or support block.
- Keep local ramps and cut regions tightly bounded. Do not use the full case footprint as a local clipping solid.
- Avoid booleans where solids only touch at a face or edge. Use small overlaps when a union must be watertight.
- Check local component bounding boxes before export when adding a new loft, ramp, or support.
- Check the final model bounding box. Expected Z range is `9..26 mm` for the current top model.
- Review at least one CAD or slicer section for Z-transition changes.
- Never move physical port holes, cutouts, or indents unless the user explicitly
  asks to move the ports/cutouts themselves. Port geometry is hardware-layout
  critical. Icon/mark placement requests do not imply physical port movement.

## Bottom plate plan

The bottom artifact is a flat drill/alignment plate, not the final enclosure tray.

- Source: `generate_bottom_plate_cadquery.py`.
- Exports: `../../release-artifacts/enclosure/step/case_bottom_plate_cadquery.step` and `../../release-artifacts/enclosure/stl/case_bottom_plate_cadquery.stl`.
- Footprint: same rounded rectangle as the faceplate.
- Holes: one M3 clearance hole and bottom-side counterbore at each `faceplate_insert_pillars_v22` position.
- Guide walls: low inset perimeter ribs align the faceplate without forming a full tray.
- Scope exclusions for this step: no full-height side walls, no port cutouts, no PCB retention, no NeoTrellis retention, no internal towers.
- Validate with `validate_bottom_plate.py` after changing insert positions or bottom-plate dimensions.

## Required roof checks

`validate_wave_roof.py` checks the roof-wall geometry:

- the brown-edge wall must be vertical from the faceplate bottom to tier 1;
- the wall must have a finite bottom footprint;
- the generated model must be one valid solid;
- the parametric slot guides must parse from `wave_guidance.py`.

Run this script for every roof-wall change.

## Generated artifacts

Top enclosure artifact filenames include the full board name. Use `rpi` for Raspberry Pi Zero 2 W and `opi` for Orange Pi Zero 2W only as shorthand in prose; prefer full board names when space permits.

- `../../release-artifacts/enclosure/step/case_top_two_level_cadquery_raspberry-pi-zero-2w.step`: Raspberry Pi Zero 2 W CAD exchange artifact.
- `../../release-artifacts/enclosure/stl/case_top_two_level_cadquery_raspberry-pi-zero-2w.stl`: Raspberry Pi Zero 2 W printable/check-fit mesh.
- `../../release-artifacts/enclosure/3mf-multicolor/case_top_two_level_raspberry-pi-zero-2w_multicolor.3mf`: Raspberry Pi Zero 2 W multicolor top with flush markings on extruder 2.
- `../../release-artifacts/enclosure/step/case_top_two_level_cadquery_orange-pi-zero-2w.step`: Orange Pi Zero 2W CAD exchange artifact.
- `../../release-artifacts/enclosure/stl/case_top_two_level_cadquery_orange-pi-zero-2w.stl`: Orange Pi Zero 2W printable/check-fit mesh.
- `../../release-artifacts/enclosure/3mf-multicolor/case_top_two_level_orange-pi-zero-2w_multicolor.3mf`: Orange Pi Zero 2W multicolor top.

STEP, STL, and 3MF files are generated artifacts. Edit the CadQuery source, not
the exported files.

To revert generated top artifacts from automation, prefer the checked wrapper:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File hardware/enclosure/revert_top_artifacts_checked.ps1
```
