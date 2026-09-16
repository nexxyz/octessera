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

4. Inspect or slice `stl/case_top_two_level_cadquery_raspberry-pi-zero-2w.stl` before using it for printing.

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
- Exports: `step/case_bottom_plate_cadquery.step` and `stl/case_bottom_plate_cadquery.stl`.
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

- `step/case_top_two_level_cadquery_raspberry-pi-zero-2w.step`: Raspberry Pi Zero 2 W CAD exchange artifact.
- `stl/case_top_two_level_cadquery_raspberry-pi-zero-2w.stl`: Raspberry Pi Zero 2 W printable/check-fit mesh.
- `3mf-single-material/case_top_two_level_cadquery_raspberry-pi-zero-2w.3mf`: Raspberry Pi Zero 2 W single-material top.
- `3mf-multicolor/case_top_two_level_raspberry-pi-zero-2w_multicolor.3mf`: Raspberry Pi Zero 2 W multicolor top with flush markings on extruder 2.
- `step/case_top_two_level_cadquery_orange-pi-zero-2w.step`: Orange Pi Zero 2W CAD exchange artifact.
- `stl/case_top_two_level_cadquery_orange-pi-zero-2w.stl`: Orange Pi Zero 2W printable/check-fit mesh.
- `3mf-single-material/case_top_two_level_cadquery_orange-pi-zero-2w.3mf`: Orange Pi Zero 2W single-material top.
- `3mf-multicolor/case_top_two_level_orange-pi-zero-2w_multicolor.3mf`: Orange Pi Zero 2W multicolor top.

STEP, STL, and 3MF files are generated artifacts. Edit the CadQuery source, not
the exported files.

To revert generated top artifacts from automation, prefer the checked wrapper:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File hardware/enclosure/revert_top_artifacts_checked.ps1
```

## Protective-case check-fit source

Octessera adapts and modifies **Simple and Light Parametric BOX - CadQuery** by
Andy Wings / `@WingsWorld_2406962`. The retained source is
`upstream/andy_wings_parametric_box.py`; its source links and modification notes
are in `upstream/README.md`. Its license is **[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/)**.

`protective_case_params.json` owns the case dimensions, source normalization,
face-down reference-top transform, corner restraints, mating interface, foam
pads, latch dimensions, branding dimensions, and eight output paths. The main
owners are:

- `generate_protective_case_cadquery.py`: composition and export;
- `protective_case_geometry.py`: shared parameters and transforms;
- `protective_case_device_fit.py`: Raspberry/Orange reference-top fit checks;
- `protective_case_mating_interface.py`: tongue and receiver geometry;
- `protective_case_corner_restraints.py`: wall-connected shelf supports;
- `protective_case_bottom_latches.py`: mirrored upstream Clip profiles;
- `protective_case_foam_landing_pads.py`: wall pad solids;
- `protective_case_device_orientation.py`: tub-floor orientation guide;
- `protective_case_branding.py`: debossed and multicolor markings;
- `validate_protective_case.py`: source and exported-package validation.

The configured source box is `255.6 x 148.4 x 62.85 mm` with `2.2 mm` walls.
Its outer top is Z`62.85`, its interior ceiling is Z`60.65`, and the seam
remains Z`54.85`. The deep source half becomes the tub and the shallow half
becomes the lid. Project code retains the upstream shell, hinge, and Clip
profiles while adding the normalized mating interface, reversed latch
ownership, restraints, pads, and branding.

The instrument placement is CAD-derived, not physically measured. The
parameter-owned transform rotates the checked-in top 180 degrees around Y, then
translates it by `[250.8, 4.2, 42.0] mm`. Fit validation builds both fixed top
variants: `raspberry-pi-zero-2w` and `orange-pi-zero-2w`.

Each ruled shelf support ends at `nominal_shelf_base_z`; the shelf begins
`0.10 mm` below it. This positive-volume overlap is required for a watertight
union. Do not replace it with a face-touching boolean. The shelves own Z support;
replaceable closed-cell foam strips own XY retention.

The deep tub uses a centered logo and floor orientation guide. The shallow lid
rotates the canonical logo and wordmark solids together by 180 degrees. Deboss
and multicolor depth are both `0.40 mm`. STEP stays in assembly coordinates;
STL and 3MF are exterior-down. The STEP, STL, single-material 3MF, and
multicolor 3MF folders each contain 18 parts. The multicolor set has 12
authored two-material designs plus six one-material support parts. Board-specific
single-material tops coexist with branded multicolor tops. Single-material 3MF
files belong in `3mf-single-material/`. The six support packages are identical
copies in both 3MF folders; the other `3mf-multicolor/` entries are authored
two-material packages.
All protective-case artifact and embedded part names use statusless
`protective_case_` names.

Run source checks before generation:

```powershell
python -B -m unittest discover -s hardware/enclosure -p "test_protective_case*.py"
python -B -m unittest discover -s hardware/enclosure -p "test_cad_source_structure.py"
python hardware/enclosure/validate_protective_case.py
node tools/quality/quality-audit.mjs
```

Generate the eight artifacts through the detached checked route:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File hardware/enclosure/generate_protective_case_artifacts_async.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File hardware/enclosure/protective_case_artifacts_async_status.ps1
```

The worker log must contain:

```text
__PROTECTIVE_CASE_GENERATION_DONE__
__PROTECTIVE_CASE_VALIDATION_DONE__
```

This remains a check-fit prototype. Inspect a slicer section through the shelf
join and physically test the disconnected instrument, openings, foam, mating
interface, hinges, pins, latches, and branding before use.
