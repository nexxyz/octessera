# Safety and power

Read this page before you first power the instrument, before you plug a
computer or USB device into it, and before you close the case. It is short, and
it is the one page I would rather you did not skim.

Octessera is a hobby build on an open PCB. Nothing here is dangerous mains
work, but a reversed part or a second power source can still cook a board.

## Power input

- **Power the instrument only through the enclosure's USB-C power opening.**
  That port is the USB-C breakout on the main PCB, and it feeds the shared
  `+5V` rail for the compute board, OLED, NeoKey, NeoTrellis grid and DAC.
- **Use a dedicated, regulated 5V/4A supply** of the kind made for Raspberry
  Pi 4-class boards. A GeeekPi 20W 5V/4A supply is a known-good example.
- **Do not plug a power supply into the compute board's own power connector.**
  On the Raspberry Pi, the micro-USB power connector is covered by the
  enclosure and is not an input for this build.
- **Never connect two power supplies at once,** and never rely on a cable or
  adapter to stop power flowing backwards (back-feed).
- `D1` (`SA5.0A`) is a 5V TVS diode across the shared rail. It soaks up short
  voltage spikes. It does not isolate a USB port or block reverse current.

For the wiring itself, see the [pin and bus
reference](../../hardware/docs/pinout-and-connections.md).

## Polarity

Two parts on the main PCB only go in one way round:

| Part | Correct orientation | If it is reversed |
|---|---|---|
| `C1`, `470uF` capacitor | Match the `+` / stripe to the PCB polarity marking. | An electrolytic capacitor fitted backwards can overheat, bulge or vent. |
| `D1`, `SA5.0A` TVS diode | Banded cathode in `K/+5V`, unbanded anode in `A/GND`. | The diode conducts straight across the rail, so the supply sees something close to a short. |

The NeoKey and the NeoTrellis cable are the other classic trap: both are easy
to plug in backwards. Before power, check that `INT` is on the south side and
that each of the five NeoTrellis leads lands on the matching label at both
ends.

## Static, heat and hands

- Touch something grounded before handling the bare boards, and hold them by
  their edges. Static is invisible and the modules do not forgive it.
- Do all soldering with the instrument unplugged. A hot iron near a live rail is
  how small mistakes become expensive ones.
- Power down and unplug before moving wiring, reseating a module, or opening the
  case.

## USB data

USB data cables are optional. When you use one, follow [USB roles](usb-roles.md)
for the right role, cable, port and adapter. Keep the instrument powered through
the enclosure USB-C breakout while the data cable is connected: the data cable
is not a second power supply.

## microSD cards and the enclosure

- Take the compute board's boot microSD card and the OLED microSD card out
  before fitting the boards into the enclosure or taking the case apart. They
  stick out, catch on the case and snap.
- Do not force a module, connector, top, pin or screw. If something resists,
  stop and find out what it is touching.

## Stop conditions

Unplug the power and stop if:

- the OLED is blank, flickering or unstable;
- power is unstable, a board browns out or reboots, a connected host back-feeds
  power, or any board or the breakout gets unexpectedly hot;
- you are not sure about a connection or a connector's orientation; or
- the enclosure does not sit flat without force.

Once you understand what went wrong, carry on with
[troubleshooting](../troubleshooting.md), the [assembly
manual](assembly-manual.md), or [flash and first boot](flash-and-first-boot.md).
