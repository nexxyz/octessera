# octessera hardware assembly manual

Follow these stages in order. Keep the assembly open through [flash and first
boot](flash-and-first-boot.md); close the enclosure only after the first-boot
checks pass.

## 1. Safety and board choice

Choose one compute board before ordering parts:

- Raspberry Pi Zero 2 W
- Orange Pi Zero 2W

Choose the board, then use its matching image in [flash and first boot](flash-and-first-boot.md)
and its matching enclosure top. The normal user journey is otherwise shared.

For an Orange Pi, the exact board/header power mapping and no-backfeed path must
be established on the bare board before connecting it to the Octessera PCB. If
either is unclear, stop; see the [Orange Armbian bring-up procedure](../../hardware/docs/orange-pi-armbian-bringup.md)
for the detailed reference.

Read [safety and power](safety-and-power.md) before handling power, a host
cable, or the enclosure. Power the finished instrument through the enclosure
USB-C power breakout. Do not power the Raspberry Pi through its micro-USB power
connector.

## 2. Parts and tools

### PCB and electronics

| Qty | Item | Part | Notes |
|---:|---|---|---|
| 1 | Custom PCB | Fabricate from [`../../release-artifacts/pcb/gerber/gerber.zip`](../../release-artifacts/pcb/gerber/gerber.zip) | Order as a two-layer PCB unless the Gerber notes say otherwise. |
| 4 | NeoTrellis 4x4 driver PCB | [Adafruit `3954`](https://www.adafruit.com/product/3954), Mouser `485-3954` | Forms the 8x8 grid. |
| 4 | Silicone 4x4 keypad | [Adafruit `1611`](https://www.adafruit.com/product/1611), Mouser `485-1611` | One per NeoTrellis board. |
| 1 | NeoKey 1x4 QT | [Adafruit `4980`](https://www.adafruit.com/product/4980), Mouser `485-4980` | Holds the four Cherry MX keys. |
| 1 | Compute board | [Raspberry Pi Zero 2 W](https://www.raspberrypi.com/products/raspberry-pi-zero-2-w/) or Orange Pi Zero 2W | Choose one board. |
| 1 | Wi-Fi antenna (Orange Pi only, optional) | Orange Pi Zero 2W-compatible antenna | Connect it before final assembly. Secure it with non-conductive tape on a free PCB area where it cannot cover pads, touch exposed contacts, be pinched, or strain the connector. |
| 1 | SSD1351 OLED breakout with microSD holder | [Adafruit `1431`](https://www.adafruit.com/product/1431), Mouser `485-1431` | SPI display. |
| 1 | PCM5102 I2S DAC | [Adafruit `6250`](https://www.adafruit.com/product/6250), Mouser `485-6250` | Line/headphone output path. |
| 1 | USB-C power breakout | [Adafruit `4090`](https://www.adafruit.com/product/4090), Mouser `485-4090` | Power the device here, not through the compute board. |
| 4 | Horizontal rotary encoder with switch | [RS `781-6811`](https://at.rs-online.com/web/p/mechanische-drehgeber/7816811), Bourns `PEC12R-4225F-S0024` | The PCB uses four encoders. |
| 1 | Polarized capacitor | [`470uF`, 16V, radial, about 8x12mm](https://de.aliexpress.com/item/1005010415990713.html) | PCB footprint: `CP_Radial_D8.0mm_P3.50mm`. An equivalent 470uF polarized radial capacitor with 3.5mm lead pitch and >5V rating is fine. |
| 1 | 5V TVS diode | `SA5.0A`, axial DO-15, `Diode_THT:D_DO-15_P10.16mm_Horizontal` | Required PCB part `D1`. The banded cathode goes to `K/+5V`; the unbanded anode goes to `A/GND`. |
| 1 | 1x5 right-angle male header, 2.54mm pitch | Any standard breakaway right-angle male pin header | Cut to 5 pins for the NeoTrellis connector. |
| 1 | 5-wire female-to-female Dupont cable, about 10cm | Any 2.54mm female-to-female jumper cable set | Use five adjacent leads for the NeoTrellis array. |
| 25 pins | Straight male pin header, 2.54mm pitch | Any standard breakaway male pin header strip | Use 20 pins to link the four NeoTrellis boards and 5 pins for the external connection point on the upper-left board. |
| several | Low-profile female header/socket strips, 2.54mm pitch | [Round-pin 2.54mm header/socket strip](https://de.aliexpress.com/item/1005006673257121.html) or [round-pin 2.54mm header/socket strip](https://de.aliexpress.com/item/4001122376295.html) | Cut to length for the selected compute board, OLED, DAC, power breakout, and other plug-in modules. Confirm socket height before ordering. |
| 4 | Cherry MX-compatible key switches | [Cherry MX Black switches](https://www.amazon.de/-/en/CHERRY-Mechanical-Keyboard-Switches-without/dp/B0CBS4HJJR?th=1), or any MX-compatible switch | Install into the NeoKey. |
| 1 | MicroSD card for the selected board | 16GB or larger recommended | Flash the matching board image. Raspberry and Orange images are separate. |
| 1 | USB-C power supply | Dedicated regulated 5V/4A supply intended for Raspberry Pi 4-class systems | The documented GeeekPi 20W 5V/4A example is acceptable. Connect only to the USB-C breakout. |
| 1 | USB data cable and adapter (optional) | Suitable cable and adapter for the selected USB operation | See [USB roles](usb-roles.md) before connecting it. |
| 1 | Audio cable/headphones/speaker | 3.5mm audio | Use for audio output. |

### 3D-printed and mechanical parts

The enclosure uses a printed dowel, standoff, and top-pin system. Screws and
heat-set inserts are recommended but optional. The fits are snug, so calibrate
the printer or test partial prints before printing the full set.

The STEP, STL, single-material 3MF, and multicolor 3MF folders each contain a
complete 18-part set. The multicolor set combines 12 authored two-material
designs with six one-material support pieces.

| Qty | Item | File/spec | Notes |
|---:|---|---|---|
| 1 | Enclosure top | Board-specific `../../release-artifacts/enclosure/stl/case_top_two_level_cadquery_<board>.stl`, matching single-material `../../release-artifacts/enclosure/3mf-single-material/case_top_two_level_cadquery_<board>.3mf`, or branded multicolor `../../release-artifacts/enclosure/3mf-multicolor/case_top_two_level_<board>_multicolor.3mf` | The generated tops are named for Raspberry Pi Zero 2 W and Orange Pi Zero 2W. Fit the exact board before ordering a final print. |
| 1 | Enclosure bottom | `../../release-artifacts/enclosure/stl/case_bottom_plate_cadquery.stl` | Bottom plate with guide walls and screw holes. |
| 1 | Main encoder knob | Print `../../release-artifacts/enclosure/stl/encoder_cap_main_knurled_dots.stl`, use `../../release-artifacts/enclosure/3mf-single-material/encoder_cap_main_knurled_dots.3mf`, or use `../../release-artifacts/enclosure/3mf-multicolor/encoder_cap_main_knurled_dots_multicolor_flush.3mf` | Main encoder cap. The multicolor version uses the dot-ring marking. |
| 3 | Aux encoder knobs | Print `../../release-artifacts/enclosure/stl/encoder_cap_aux*_ribbed_dot*.stl`, use matching `../../release-artifacts/enclosure/3mf-single-material/encoder_cap_aux*.3mf`, or use matching `../../release-artifacts/enclosure/3mf-multicolor/encoder_cap_aux*_multicolor_flush.3mf` | Aux 1/2/3 caps. Multicolor versions use one, two, and three dots. |
| 4 | MX keycaps | Print `../../release-artifacts/enclosure/stl/mx_keycap_*.stl`, use matching `../../release-artifacts/enclosure/3mf-single-material/mx_keycap_*.3mf`, use matching `../../release-artifacts/enclosure/3mf-multicolor/mx_keycap_*_multicolor_flush.3mf`, or use any MX-stem keycap | Four NeoKey caps: back, play, shift, and function/layer. Transparent filament for the cap body lets the color LEDs shine through. |
| 8 | 9.5mm standoff pillar | Print `../../release-artifacts/enclosure/stl/standoff_pillar_9_5mm.stl` and matching `../../release-artifacts/enclosure/3mf-single-material/standoff_pillar_9_5mm.3mf`, or use compatible purchased stackable PCB standoffs | Use for the OLED and audio/DAC board support locations. |
| 10 | 10mm standoff pillar | Print `../../release-artifacts/enclosure/stl/standoff_pillar_10mm.stl` and matching `../../release-artifacts/enclosure/3mf-single-material/standoff_pillar_10mm.3mf`, or use compatible purchased stackable PCB standoffs | Use for the selected compute board, power breakout, and NeoKey support locations. NeoTrellis array pins go straight into the bottom's integrated pillars. |
| 26 | Standoff top pin | Print `../../release-artifacts/enclosure/stl/standoff_top_pin_thin_base.stl` and matching `../../release-artifacts/enclosure/3mf-single-material/standoff_top_pin_thin_base.3mf`, or use compatible purchased stackable PCB standoff pins | 4 compute-board + 4 audio/DAC + 4 OLED/screen + 4 NeoKey + 2 power + 8 NeoTrellis array pins = 26 total. |
| 8 | Heat-set insert | M3x6x5 heat-set insert, such as the M3 size in this [heat-set insert kit](https://de.aliexpress.com/item/1005012199553197.html) | Recommended but optional. Insert from the underside of the top; the smooth lead-in side locates in the `4.6mm` pilot hole. |
| 8 | Screws | M3x8 socket-head cap screw, DIN 912 / ISO 4762 style | Recommended but optional. Install from the bottom; head diameter must be no larger than `6.4mm`. |
| 8 | Rubber feet or screw-hole plugs | Small adhesive feet | Optional. Covers bottom screw holes and prevents sliding. |

### Tools and consumables

- Soldering iron and solder.
- Flush cutters.
- Multimeter.
- BalenaEtcher or another suitable image flasher.
- Optional: small screwdriver, heat-set insert tool, pliers, continuity
  tester, tweezers, helping hands, and magnifier.

## 3. Soldering

### Main PCB

Before soldering, inspect the PCB, sort the sockets and modules, and mark the
correct side of every module. Keep each module oriented as it will sit in the
enclosure. Headers on the wrong side are difficult to fix.

1. Solder the low-profile sockets for the selected compute board, OLED, DAC,
   USB-C power breakout, and other socketed modules.
2. Solder the 1x5 right-angle male header for the NeoTrellis connector.
3. Solder `D1`, the `SA5.0A` TVS diode. Put the banded cathode in `K/+5V` and
   the unbanded anode in `A/GND`. Keep the body and leads clear of nearby pads
   and metal parts.
4. Solder `C1`, the `470uF` polarized capacitor, matching the PCB polarity
   markings. The legs may remain a little long so the capacitor can bend
   sideways; prevent the legs from touching pads or metal.
5. Solder the four rotary encoders into `SW1` through `SW4`.

Aim the horizontal NeoTrellis header toward the cable path and check the PCB
connector indicator. Do not install plug-in modules yet.

### NeoTrellis and NeoKey

The four NeoTrellis boards form one 8x8 grid.

1. Arrange the boards as upper-left, upper-right, lower-left, and lower-right
   when viewed from the play surface.
2. Turn the arrangement around as one plane before soldering; left and right
   swap when viewed from the bottom.
3. Solder 20 straight male header pins between adjacent boards.
4. Add 5 straight male header pins as the external connection point on the
   left side of the upper-left board.
5. Viewed from the bottom, set the addresses as follows:

   | Position | Jumpers | Address |
   |---|---|---:|
   | upper left | A0 | `0x2F` |
   | upper right | none | `0x2E` |
   | lower left | A0 + A1 | `0x31` |
   | lower right | A1 | `0x30` |

   See the [NeoTrellis address jumper photo](images/assembly/neotrellis-address-jumpers.jpg).

6. Set the NeoKey address by soldering A0, A1, A2, and A3. Its address is
   `0x3F`. See the [NeoKey address jumper photo](images/assembly/neokey-address-jumpers.jpg).

The NeoKey and NeoTrellis connector are easy to plug in backwards. Before
powering the device, check that `INT` is on the south side.

## 4. Open assembly

1. Insert the selected compute board, OLED, DAC, USB-C power breakout, and
   NeoKey into their sockets.
2. Connect the NeoTrellis array to the PCB with the 5-wire female-to-female
   Dupont cable.
3. Install the Cherry MX switches into the NeoKey and add the four keycaps.
4. If your build includes the optional board antenna, connect it now and secure
   it where it cannot touch contacts, cover pads, or be pinched.
5. Connect headphones, speakers, or a mixer to the audio output.
6. Keep the assembly open and continue with [flash and first boot](flash-and-first-boot.md).

## 5. Flash and setup

Complete [flash and first boot](flash-and-first-boot.md) before installing the
enclosure. Confirm that the OLED, grid, keys, encoders, and audio respond while
the boards are still accessible.

## 6. Enclosure

Remove both the selected compute board's boot microSD card and the OLED microSD
card before putting the boards into the enclosure. They can catch on the case
and break. Remove the NeoKey switches before fitting the case; you can leave
their keycaps on. Reinsert the cards after the case is closed if the openings
require it.

1. Place the bottom enclosure on the bench. It can also hold the upside-down
   NeoTrellis array while you solder the four boards together.
2. Put the PCB and NeoTrellis array onto the taller pillars.
3. Add the 18 separate standoff pillars between the bottom supports and the
   plug-in modules:
   - use 8 of the 9.5mm pillars for the OLED and audio/DAC board;
   - use the 10 10mm pillars for the selected compute board, power breakout,
     and NeoKey;
   - use no separate pillars for the NeoTrellis array; its eight pins enter the
     integrated bottom pillars.
4. Press in all 26 top pins: 18 into the separate standoffs and 8 into the
   NeoTrellis integrated pillars. Use gentle pressure. If a pin is loose,
   gently squeeze its ball with pliers or replace it rather than crushing it.
5. Place one silicone 4x4 keypad on each NeoTrellis board. Make the pads sit
   flat and line up with the 8x8 opening.
6. Optionally install the 8 M3 heat-set inserts into the underside of the top.
7. Lower the top from the left/west side first so the ports pass through their
   openings, then lower the right/east side. Do not force it.
8. Install the four encoder knobs and check that they slide on with friction
   but can still be removed without force.
9. Turn the device over carefully. Optionally install the 8 bottom screws and
   add rubber feet or screw-hole covers.

Tighten screws gently. If the top does not sit flat, find the interference
instead of forcing the case closed.

## 7. Final check

1. Reinsert the selected board's boot microSD card and the OLED microSD card if
   you removed them for enclosure assembly.
2. Connect power through the enclosure USB-C breakout and wait for boot.
3. Confirm that all four encoders, four NeoKey switches, and all 64 NeoTrellis
   cells respond.
4. Confirm that the OLED is readable and audio comes from the DAC.
5. Confirm that the compute-board microSD, OLED microSD, audio, USB-C power,
   and video openings are accessible. See [enclosure](enclosure.md) for the
   matching board top.

If anything is unclear, stop and use [troubleshooting](../troubleshooting.md)
before continuing.

## Source and manufacturing references

- Gerbers: [`../../release-artifacts/pcb/gerber/gerber.zip`](../../release-artifacts/pcb/gerber/gerber.zip)
- Schematic: [`../../hardware/pcb/octessera.kicad_sch`](../../hardware/pcb/octessera.kicad_sch)
- PCB layout: [`../../hardware/pcb/octessera.kicad_pcb`](../../hardware/pcb/octessera.kicad_pcb)
- Pin and bus reference: [`../../hardware/docs/pinout-and-connections.md`](../../hardware/docs/pinout-and-connections.md)
- Enclosure: [`enclosure.md`](enclosure.md)
- Standoff attribution: [Stackable PCB Standoff by theduckom](https://www.printables.com/model/163087-stackable-pcb-standoff), licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
