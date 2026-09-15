# Enclosure

Complete [flash and first boot](flash-and-first-boot.md) while the assembly is
open, then fit the enclosure. Read [safety and power](safety-and-power.md)
before fitting or powering the case.

A separate [protective-case check-fit prototype](protective-case.md) is available
for the completed instrument. It is not drop-rated or transport-qualified.

Remove the selected board's boot microSD card and the OLED microSD card before
putting the device in the enclosure. They can catch on the case and break.

## Print choices

The enclosure is `247 x 140 mm`. The main PCB rail is `3.2 mm` high and the
NeoTrellis rail is `8.0 mm` high. Choose the top that matches the selected board:

- Raspberry Pi Zero 2 W: `case_top_two_level_cadquery_raspberry-pi-zero-2w.stl`
  or `case_top_two_level_raspberry-pi-zero-2w_multicolor.3mf`
- Orange Pi Zero 2W: `case_top_two_level_cadquery_orange-pi-zero-2w.stl` or
  `case_top_two_level_orange-pi-zero-2w_multicolor.3mf`

The bottom is `case_bottom_plate_cadquery.stl`. The printed encoder caps,
keycaps, standoffs, top pins, heat-set inserts, screws, and rubber feet are
listed in the [assembly manual](assembly-manual.md#3d-printed-and-mechanical-parts).
Test-fit printed parts before printing the full set; the fits are snug.

## Access and safety

The matching top provides openings for power, audio, storage, video, and the
board's other connectors. Before closing the case, check that the selected
board's openings line up with the top. The OLED microSD remains inside the case.

Power through the enclosure USB-C breakout. Do not use the Raspberry Pi's
covered micro-USB power connector. For optional USB connections, follow [USB
roles](usb-roles.md).

## Fit and assembly

The case captures the boards with rails, standoffs, and top pins rather than
screws through active hardware areas.

- The main PCB is located by tight rails and nubs.
- Lid capture ribs limit upward movement at the board edges.
- The NeoTrellis cluster is located by perimeter rails. Its left rail is broken
  for the `J1` connector path.
- NeoTrellis vertical retention uses the top faceplate, top pins, and edge
  capture ribs, not screws through the button field.

1. Place the bottom enclosure on the bench.
2. Put the PCB and NeoTrellis array onto the taller pillars.
3. Add the 18 separate standoff pillars:
   - use 8 of the 9.5mm pillars for the OLED and audio/DAC board;
   - use the 10 10mm pillars for the selected compute board, power breakout,
     and NeoKey;
   - use no separate pillars for the NeoTrellis array; its eight pins enter the
     integrated bottom pillars.
4. Press in all 26 top pins: 18 into the separate standoffs and 8 into the
   NeoTrellis integrated pillars. If a pin is loose, gently squeeze its ball
   with pliers or replace it rather than crushing it.
5. Place one silicone 4x4 keypad on each NeoTrellis board. Make the pads sit
   flat and line up with the 8x8 opening.
6. Optionally install the 8 M3 heat-set inserts into the underside of the top.
7. Lower the top from the left/west side first so the connectors pass through
   their openings, then lower the right/east side. Do not force it.
8. Install the four encoder knobs and check that they slide on with friction but
   can still be removed without force.
9. Turn the device over carefully. Optionally install the 8 bottom screws and
   add rubber feet or screw-hole covers.

The printed dowel/standoff and top-pin system can hold the case without screws.
Tighten screws gently. If the top does not sit flat, find the interference
instead of forcing it closed.
