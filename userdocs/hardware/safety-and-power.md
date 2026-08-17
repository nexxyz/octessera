# Safety and power

Use this page before first power-on, before connecting a host cable, and before
closing the enclosure. It is the short owner for the repeated power and fit
rules; the assembly, pinout, and board pages still carry their action-point
warnings.

## Power input

- Power the instrument through the enclosure USB-C power opening and its
  breakout. The breakout feeds the shared `+5V` rail.
- Use the enclosure USB-C breakout as the intended power input. Do not use
  another board power port unless the current selected-board wiring or bring-up
  instructions explicitly authorize it.
- On Raspberry, do not power the Pi through its micro-USB power connector. The
  enclosure covers that connector and it is not an intended input.
- Use a dedicated, regulated 5V supply rated for at least 4A. A Raspberry Pi
  power supply is a good fit; for example, the “GeeekPi for Raspberry Pi 4
  20W 5V 4A” supply includes a handy inline power switch. A 2A supply is likely
  marginal once the board and LEDs are running.

The two boards do not share port assumptions. Raspberry uses the fixed Pi
profile and wiring table. Orange uses a reviewed, board-specific Armbian
procedure for its power, USB-role, and pinmux checks; that procedure does not
prove that a selected build passed those gates. Do not translate Raspberry pin
numbers or connector roles to Orange by physical position.

## USB host connections

**USB Audio and USB MIDI are experimental local-bench paths, not public first-
release support.** Use them only with an authorized identity and after the exact
image and assembled board pass the electrical and manual FAT gates. The current
Linux Foundation VID/PID values are local-validation-only and are not a public
product identity; do not invent or publish replacement IDs. Defaults remain
disabled.

A normal host USB cable can send 5V back into a board that is already powered
through the enclosure input. Software cannot block that power while keeping USB
data. Use a data-only cable or a power-isolating adapter, and use the selected
build's host-data port only after that exact build passes its port-role, VBUS/CC,
and no-backfeed gates.

Before connecting a host, stop if the port role, VBUS/CC behavior, or no-backfeed
path is unclear. See the [pinout and connections](pinout-and-connections.md),
[Orange bring-up notes](../../hardware/docs/orange-pi-armbian-bringup.md), and
[board qualification page](board-qualification.md).

## Orientation and enclosure handling

- Check the NeoKey and NeoTrellis connector orientation before power. `INT`
  should be on the south side.
- Remove the selected board's microSD card and the OLED microSD card before
  putting the boards into the enclosure. They can catch on the case and break.
- Do not force a module, connector, top, pin, or screw. Stop and find the
  interference.

## Stop conditions

Stop the build or test and record what happened if:

- the OLED is blank, flickering, unstable, or has more than one writer;
- a diagnostic reports an actual hardware error;
- power is unstable, a board browns out, or a host connection back-feeds power;
- a board pin, port role, or connector orientation is uncertain; or
- the enclosure does not sit flat without force.

Continue with [troubleshooting](../troubleshooting.md), the [assembly
manual](assembly-manual.md), [enclosure notes](enclosure.md), or the [Raspberry
first-boot page](raspberry-pi-first-boot.md) or [Orange first-boot
page](orange-pi-first-boot.md) only after the unresolved gate is understood.

## Next links

- [Assembly manual](assembly-manual.md)
- [Raspberry first boot](raspberry-pi-first-boot.md)
- [Orange first boot](orange-pi-first-boot.md)
- [Pinout and connections](pinout-and-connections.md)
- [Board qualification and status](board-qualification.md)
- [Troubleshooting](../troubleshooting.md)
