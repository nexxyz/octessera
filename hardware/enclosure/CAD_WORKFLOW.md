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

4. Inspect `current_wave_top_view.png` for plan-view clearances.
5. Inspect or slice `case_top_two_level_cadquery.stl` before using it for printing.

## Required roof checks

`validate_wave_roof.py` checks the failure mode that caused slicer artifacts:

- the brown-edge wall must be vertical from the faceplate bottom to tier 1;
- the wall must have a finite bottom footprint;
- the generated model must be one valid solid;
- the slot guides must parse from the SVG.

Do not accept a roof-wall change until this script passes.

## Generated artifacts

- `case_top_two_level_cadquery.step`: preferred CAD exchange artifact.
- `case_top_two_level_cadquery.stl`: current printable/check-fit mesh.
- `current_wave_top_view.png`: plan-view review image.

STEP and STL files are generated artifacts. Do not review their full text diffs.
