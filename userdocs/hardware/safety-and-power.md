# Safety and power

Read this page before first power-on, before connecting a host cable, and before
closing the enclosure.

## Power input

- Power the instrument through the enclosure USB-C power opening and its
  breakout. The breakout feeds the shared `+5V` rail.
- Use a dedicated regulated 5V/4A supply intended for Raspberry Pi 4-class
  systems. The documented GeeekPi 20W 5V/4A supply is suitable.
- On Raspberry, do not power the Pi through its micro-USB power connector. The
  enclosure covers it and it is not an intended input.
- `D1` `SA5.0A` is the 5V TVS diode for the shared rail. It protects against
  transients; it does not isolate VBUS or reverse current.

For technical wiring details, use the [pin and bus reference](../../hardware/docs/pinout-and-connections.md).

## USB data

For optional USB data connections, follow [USB roles](usb-roles.md) for the
role, cable, port, power, and no-backfeed instructions. Keep the instrument
powered through the enclosure breakout while a data cable is connected.

## Orientation and enclosure handling

- Check the NeoKey and NeoTrellis connector orientation before power. `INT`
  should be on the south side.
- Remove the selected board's boot microSD card and the OLED microSD card before
  putting the boards into the enclosure. They can catch on the case and break.
- Do not force a module, connector, top, pin, or screw. Stop and find the
  interference.

## Stop conditions

Disconnect power and stop if:

- the OLED is blank, flickering, or unstable;
- power is unstable, a board browns out, a host connection back-feeds power, or
  the board or breakout heats unexpectedly;
- a connection or connector orientation is uncertain; or
- the enclosure does not sit flat without force.

Continue with [troubleshooting](../troubleshooting.md), the [assembly
manual](assembly-manual.md), or [flash and first boot](flash-and-first-boot.md)
after the problem is understood.
