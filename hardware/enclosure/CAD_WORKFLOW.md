# Enclosure CAD workflow

The enclosure CAD is under construction. Use the CadQuery generator as the source of truth for the current two-level faceplate.

## Edit loop

1. Edit `wave_curve_guidance.svg`.
   - Brown paths define the canonical lower roof edge.
   - The high edge is generated as an even offset from the brown curve.
   - `#333` guide lines define ventilation slots.
2. Regenerate the model:

   ```sh
   python hardware/enclosure/generate_two_level_enclosure_cadquery.py
   ```

3. Run the roof-wall validation:

   ```sh
   python hardware/enclosure/validate_wave_roof.py
   ```

4. Inspect `../../release-artifacts/enclosure/current_wave_top_view.png` for plan-view clearances.
5. Inspect or slice `../../release-artifacts/enclosure/case_top_two_level_cadquery.stl` before using it for printing.

## Geometry change checklist

Use this checklist before changing generated solids, Z transitions, or board-adjacent surfaces:

- Map relevant feature coordinates from `enclosure_params.json` before editing.
- Decide which solid owns each top surface: tier 1, tier 2, shoulder, ramp, or support block.
- Keep local ramps and cut regions tightly bounded. Do not use the full case footprint as a local clipping solid.
- Avoid booleans where solids only touch at a face or edge. Use small overlaps when a union must be watertight.
- Check local component bounding boxes before export when adding a new loft, ramp, or support.
- Check the final model bounding box. Expected Z range is `9..26 mm` for the current top model.
- Review both the plan image and at least one CAD or slicer section for Z-transition changes.

## Bottom plate plan

The first bottom artifact is only a flat drill/alignment plate. It is not the final enclosure tray.

- Source: `generate_bottom_plate_cadquery.py`.
- Exports: `../../release-artifacts/enclosure/case_bottom_plate_cadquery.step` and `../../release-artifacts/enclosure/case_bottom_plate_cadquery.stl`.
- Footprint: same rounded rectangle as the faceplate.
- Holes: one M3 clearance hole and bottom-side counterbore at each `faceplate_insert_pillars_v22` position.
- Guide walls: low inset perimeter ribs align the faceplate without forming a full tray.
- Scope exclusions for this step: no full-height side walls, no port cutouts, no PCB retention, no NeoTrellis retention, no internal towers.
- Validate with `validate_bottom_plate.py` after changing insert positions or bottom-plate dimensions.

## Required roof checks

`validate_wave_roof.py` checks the failure mode that caused slicer artifacts:

- the brown-edge wall must be vertical from the faceplate bottom to tier 1;
- the wall must have a finite bottom footprint;
- the generated model must be one valid solid;
- the slot guides must parse from the SVG.

Do not accept a roof-wall change until this script passes.

## Generated artifacts

- `../../release-artifacts/enclosure/case_top_two_level_cadquery.step`: preferred CAD exchange artifact.
- `../../release-artifacts/enclosure/case_top_two_level_cadquery.stl`: current printable/check-fit mesh.
- `../../release-artifacts/enclosure/current_wave_top_view.svg`: plan-view review source image.
- `../../release-artifacts/enclosure/current_wave_top_view.png`: plan-view review image.

STEP and STL files are generated artifacts. Do not review their full text diffs.
