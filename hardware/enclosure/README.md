# Enclosure

This is the enclosure and mechanical reference for the Cell Symphony hardware target.

Use it with [`../pinout-and-connections.md`](../pinout-and-connections.md) for wiring and [`../pi-bring-up.md`](../pi-bring-up.md) for Pi setup.

## Current Version

The current enclosure data in `enclosure_params.json` is the `v21` set.

- Case size: `247 x 140 mm`
- Main PCB rail height: `3.2 mm`
- NeoTrellis rail height: `8.0 mm`

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
- `case_top_v21_PRINT_FACE_DOWN_use_this_for_printing.stl` is the intended lid print orientation artifact
- `case_top_v21_ASSEMBLY_CHECK_catch_rims_on_underside.stl` is the alignment inspection artifact

The checked-in STL files in this directory are:

- `case_top.stl`
- `case_bottom.stl`
- `tpu_foot_strip_1.stl`
- `tpu_foot_strip_2.stl`
- `neotrellis_cutout_test_coupon.stl`

## Source of Truth

- Parameters: [`enclosure_params.json`](enclosure_params.json)
- Enclosure layout image: [`layout.png`](layout.png)
- Printable meshes: `hardware/enclosure/*.stl`
