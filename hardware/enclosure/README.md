# Enclosure

This is the enclosure and mechanical reference for the Cell Symphony hardware target.

Use it with [`../docs/pinout-and-connections.md`](../docs/pinout-and-connections.md) for wiring and [`../docs/pi-bring-up.md`](../docs/pi-bring-up.md) for Pi setup.

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

After changing the roof or guidance SVG, run the validation script:

```sh
python hardware/enclosure/validate_wave_roof.py
```

It writes:

- `../../release-artifacts/enclosure/case_top_two_level_cadquery.step`
- `../../release-artifacts/enclosure/case_top_two_level_cadquery.stl`

The script requires Python with `cadquery` available:

```sh
python -m pip install -r hardware/enclosure/requirements.txt
```

This model keeps OLED and encoders on the lower deck, raises the NeoKeys and
8x8 NeoTrellis field, and uses a guidance-driven roof/shoulder over the Pi area.
See [`CAD_WORKFLOW.md`](CAD_WORKFLOW.md) for the edit and validation loop.

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

## Mechanical Strategy

The current enclosure captures the boards without running screws through active hardware areas.

- Case screws do not pass through the PCBs or component fields.
- Heat-set insert bosses are integrated into the outer locator rail regions.
- The main PCB is located laterally by tight rails and nubs.
- Lid capture ribs limit upward movement at safe board-edge regions.
- The NeoTrellis cluster is located by perimeter rails.
- The NeoTrellis left rail is broken for the `J1` / connector path clearance.
- NeoTrellis vertical retention is handled by the screwed-down top faceplate and edge capture ribs, not by screws through the button field.

## Printing Notes

Current enclosure notes from the parameter source:

- No OLED top-edge / top-plate hole above the display
- Case height reduced to `140 mm`; width remains `247 mm`
- The checked-in release artifact is the current generated faceplate mesh:
  `../../release-artifacts/enclosure/case_top_two_level_cadquery.stl`.

## Source of Truth

- Parameters: [`enclosure_params.json`](enclosure_params.json)
- Enclosure layout image: [`layout.png`](layout.png)
- Wave/slot guidance SVG: [`wave_curve_guidance.svg`](wave_curve_guidance.svg)
- Parametric generator: [`generate_two_level_enclosure_cadquery.py`](generate_two_level_enclosure_cadquery.py)
- CAD workflow and checks: [`CAD_WORKFLOW.md`](CAD_WORKFLOW.md)
- Roof-wall validation: [`validate_wave_roof.py`](validate_wave_roof.py)
- Generated review images: [`../../release-artifacts/enclosure/current_wave_top_view.svg`](../../release-artifacts/enclosure/current_wave_top_view.svg) and [`../../release-artifacts/enclosure/current_wave_top_view.png`](../../release-artifacts/enclosure/current_wave_top_view.png)
