# Enclosure

The enclosure is a printed two-part shell: a bottom plate and a two-level top
that carries the grid, keys, knobs and screen. The fitting procedure lives in
the [assembly manual](assembly-manual.md#6-enclosure); this page covers what to
print, which opening is which, and how the case holds everything.

Fit the enclosure only after [flash and first boot](flash-and-first-boot.md)
has passed with the case open, and read [safety and power](safety-and-power.md)
before fitting or powering it.

> **Before the case goes on or comes off:** take out the compute board's boot
> microSD card and the OLED microSD card. They catch on the case and break.

Looking for a box to carry the finished instrument in? There is a separate
[protective case](protective-case.md) check-fit prototype. It is not drop-rated
or transport-qualified.

## Print choices

The enclosure is `247 x 140 mm`. The main PCB locating rail is `3.2 mm` high and
the NeoTrellis rail is `8.0 mm` high. Print the top that matches your board:

| Board | Top (STL) | Top (branded multicolor 3MF) |
|---|---|---|
| Raspberry Pi Zero 2 W | `case_top_two_level_cadquery_raspberry-pi-zero-2w.stl` | `case_top_two_level_raspberry-pi-zero-2w_multicolor.3mf` |
| Orange Pi Zero 2W | `case_top_two_level_cadquery_orange-pi-zero-2w.stl` | `case_top_two_level_orange-pi-zero-2w_multicolor.3mf` |

Single-material 3MF versions of both tops are in
`../../hardware/enclosure/3mf-single-material/`. The bottom is
`case_bottom_plate_cadquery.stl`. The knobs, keycaps, standoffs, top pins,
inserts, screws and feet are listed in the [assembly
manual](assembly-manual.md#3d-printed-and-mechanical-parts).

The fits are snug. Test-print a part or two before printing the full set.

## Openings

Each opening has a small icon beside it, so you can find your way round the
case without this page. Compass directions match the assembly manual: the main
PCB and compute board sit at the west end, the grid at the east end.

| Wall | Opening | Icon |
|---|---|---|
| West | 3.5mm audio output | Headphones |
| West | USB-C power input | Lightning bolt |
| West | Compute board boot microSD (SD1) | SD card, one dot |
| South | Mini-HDMI video and audio | Monitor |
| South | Compute board USB data port | USB (Orange top: plus one dot) |
| South | Orange Pi USB host port (Orange top only) | USB, two dots |
| North | OLED microSD (SD2) | SD card, two dots |

The Raspberry Pi's micro-USB power connector is deliberately covered: power goes
in through the USB-C opening only. For USB data connections, see [USB
roles](usb-roles.md).

## How the case holds the boards

The case captures the boards with rails, standoffs and top pins, not with
screws through the electronics.

- The main PCB is located by tight rails and nubs, and lid capture ribs limit
  upward movement at the board edges.
- The NeoTrellis array is located by perimeter rails. The left rail has a gap
  for the `J1` cable.
- The NeoTrellis array sits on taller built-in pillars on the east side, held by
  eight top pins, with the top faceplate and edge ribs keeping it down. No
  screws go through the button field.
- On the west side, loose standoffs pass through the main PCB into smaller
  built-in pillars. The plug-in modules rest on those standoffs with their
  header pins in the main-PCB sockets, and 18 more top pins hold them in place.
- Eight M3 screws from underneath join the bottom to heat-set inserts in the
  top.
- The NeoKey switches go in last, through the closed top, which guides their
  pins and holds them by friction.

If the top does not sit flat, find what it is touching instead of forcing it.
