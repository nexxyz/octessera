# Enclosure

This is the enclosure and mechanical reference for the Octessera hardware target.

Use it with [`assembly-manual.md`](assembly-manual.md) for build order and [`pinout-and-connections.md`](pinout-and-connections.md) for wiring. For the user-facing docs home, start at [`../README.md`](../README.md).

This is the part where the instrument becomes an object you can pick up. Print carefully, test-fit patiently, and take out the SD card before you put the device in the enclosure, or it may break. Ask me how I found that one out.

## Current Status

The enclosure is under construction. The current parameter data in
`enclosure_params.json` is the `v21` set, but the generated two-level faceplate
is still an active design model, not a production-ready enclosure release.

- Case size: `247 x 140 mm`
- Main PCB rail height: `3.2 mm`
- NeoTrellis rail height: `8.0 mm`

## Two-Level CadQuery Model

Generate the current two-level model with CadQuery/OpenCascade:

```sh
python hardware/enclosure/generate_two_level_enclosure_cadquery.py
```

After changing the roof or parametric wave guidance, run the validation script:

```sh
python hardware/enclosure/validate_wave_roof.py
```

It writes:

- `../../release-artifacts/enclosure/step/case_top_two_level_cadquery.step`
- `../../release-artifacts/enclosure/stl/case_top_two_level_cadquery.stl`

The script requires the enclosure Python dependencies:

```sh
python -m pip install -r hardware/enclosure/requirements.txt
```

This model keeps OLED and encoders on the lower deck, raises the NeoKeys and
8x8 NeoTrellis field, and uses a parametric raised roof/shoulder over the Pi area.
See [`../../hardware/enclosure/CAD_WORKFLOW.md`](../../hardware/enclosure/CAD_WORKFLOW.md) for the edit and validation loop. For branded top artifacts, prefer:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File hardware/enclosure/generate_branded_top_artifacts_checked.ps1
```

Branding source and cleanup rules are documented in [`../../hardware/docs/branding-assets.md`](../../hardware/docs/branding-assets.md).

The STEP file is the preferred generated artifact. This is still not a
production lid: it does not yet recreate the full underside lip, catch rims,
board capture ribs, or bottom mating interface. Promote it to the production top
only after validating that underside interface, board capture, connector
clearance, slicer output, and the measured component height stack.

## External Access

The current case exposes these ports:

- Left side: audio 3.5mm
- Left side: USB-C power
- Left side: Pi microSD
- Bottom side: Pi mini-HDMI
- Bottom side: Pi USB data

The OLED microSD is not exposed as a case-edge port in the current `v21` entry.

## Power Rule

- Power the device through the enclosure USB-C power opening.
- Do not power the Raspberry Pi through its own micro-USB power connector.
- The Pi micro-USB power connector is intentionally covered by the enclosure and is not meant to be used.
- The exposed Pi USB data port can still receive 5V from a normal host cable. There is no software setting that blocks that power while keeping USB data alive; use a data-only cable, a powered hub/splitter that isolates power, or a hardware power-path fix if you need pins-only power.

## Mechanical Strategy

The current enclosure captures the boards without running screws through active hardware areas.

- Case screws do not pass through the PCBs or component fields.
- Heat-set insert bosses are integrated into the outer locator rail regions. Screws and inserts are recommended for a robust portable build, but the printed dowel/standoff and top-pin system is intended to be strong enough to hold the enclosure together without them.
- The main PCB is located laterally by tight rails and nubs.
- Lid capture ribs limit upward movement at safe board-edge regions.
- The NeoTrellis cluster is located by perimeter rails.
- The NeoTrellis left rail is broken for the `J1` / connector path clearance.
- NeoTrellis vertical retention is handled by the top faceplate, top pins, and edge capture ribs, not by screws through the button field.
- If a printed top pin is a little too loose, gently squeeze the ball at the end with pliers to make it grip tighter. Sneak up on the fit; crushed pins are less charming than snug ones.

## Printing Notes

Current enclosure notes from the parameter source:

- No OLED top-edge / top-plate hole above the display
- Case height reduced to `140 mm`; width remains `247 mm`
- The checked-in release artifact is the current generated faceplate mesh:
  `../../release-artifacts/enclosure/stl/case_top_two_level_cadquery.stl`.

## Source of Truth

- Parameters: [`../../hardware/enclosure/enclosure_params.json`](../../hardware/enclosure/enclosure_params.json)
- Enclosure layout image: [`../../hardware/enclosure/layout.png`](../../hardware/enclosure/layout.png)
- Wave/slot guidance: [`../../hardware/enclosure/wave_guidance.py`](../../hardware/enclosure/wave_guidance.py)
- Parametric generator: [`../../hardware/enclosure/generate_two_level_enclosure_cadquery.py`](../../hardware/enclosure/generate_two_level_enclosure_cadquery.py)
- Standoff pillar generator: [`../../hardware/enclosure/generate_standoff_pillars.py`](../../hardware/enclosure/generate_standoff_pillars.py)
- Standoff top-pin generator: [`../../hardware/enclosure/generate_standoff_top_pins.py`](../../hardware/enclosure/generate_standoff_top_pins.py)
- CAD workflow and checks: [`../../hardware/enclosure/CAD_WORKFLOW.md`](../../hardware/enclosure/CAD_WORKFLOW.md)
- Branded top artifact wrapper: [`../../hardware/enclosure/generate_branded_top_artifacts_checked.ps1`](../../hardware/enclosure/generate_branded_top_artifacts_checked.ps1)
- Roof-wall validation: [`../../hardware/enclosure/validate_wave_roof.py`](../../hardware/enclosure/validate_wave_roof.py)
