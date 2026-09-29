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

Follow the [assembly manual's enclosure sequence](assembly-manual.md#6-enclosure)
after testing the electronics while open. The main PCB sits on the smaller
west-side pillars; the cabled NeoTrellis array sits on the taller east-side
pillars, held by eight top pins beneath its four silicone key membranes. Loose
standoffs pass through the main PCB into the west-side pillars. The plug-in
boards rest on those standoffs with their soldered header pins in the main-PCB
sockets; another 18 top pins pass through their mounting holes into the loose
standoffs.

Heat-set the inserts into the **empty top** before fitting it. Fasten the bottom
to the top with machine screws from underneath. Only then fit the four encoder
caps, followed by the NeoKey switches and their keycaps: the top receives and
friction-holds the switches and guides their pins into the breakout. For an
optional early key check, seat the top without screws and use it as the guide.
A bare-switch test without the top risks bending pins or breaking the NeoKey;
try it only if you accept that risk and have spares. Remove any test switches
through the guide before turning the unscrewed case over. Tighten screws gently;
if the top does not sit flat, find the interference instead of forcing it closed.
