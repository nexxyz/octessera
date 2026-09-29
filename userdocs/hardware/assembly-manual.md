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

Read [safety and power](safety-and-power.md) before handling power, a host
cable, or the enclosure. Power the finished instrument through the enclosure
USB-C power breakout. Do not power the Raspberry Pi through its micro-USB power
connector.

## 2. Parts and tools

### PCB and electronics

| Qty | Item | Part | Notes |
|---:|---|---|---|
| 1 | Custom PCB | Fabricate from [`../../hardware/pcb/gerber/gerber.zip`](../../hardware/pcb/gerber/gerber.zip) | Order as a two-layer PCB unless the Gerber notes say otherwise. |
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
| 4 | Cherry MX-compatible key switches | [Cherry MX Black switches](https://www.amazon.de/-/en/CHERRY-Mechanical-Keyboard-Switches-without/dp/B0CBS4HJJR?th=1), or any MX-compatible switch | Fit through the enclosure top into the NeoKey after closing the case. |
| 1 | MicroSD card for the selected board | 16GB or larger recommended | Flash the matching board image. Raspberry and Orange images are separate. |
| 1 | USB-C power supply | Dedicated regulated 5V/4A supply intended for Raspberry Pi 4-class systems | The documented GeeekPi 20W 5V/4A example is acceptable. Connect only to the USB-C breakout. |
| 1 | USB data cable and adapter (optional) | Suitable cable and adapter for the selected USB operation | See [USB roles](usb-roles.md) before connecting it. |
| 1 | Audio cable/headphones/speaker | 3.5mm audio | Use for audio output. |

### 3D-printed and mechanical parts

The enclosure uses a printed dowel, standoff, and top-pin system. This assembly
uses heat-set inserts and screws to join the two shells. The fits are snug, so
calibrate the printer or test partial prints before printing the full set.

The STEP, STL, single-material 3MF, and multicolor 3MF folders each contain a
complete 18-part set. The multicolor set combines 12 authored two-material
designs with six one-material support pieces.

| Qty | Item | File/spec | Notes |
|---:|---|---|---|
| 1 | Enclosure top | Board-specific `../../hardware/enclosure/stl/case_top_two_level_cadquery_<board>.stl`, matching single-material `../../hardware/enclosure/3mf-single-material/case_top_two_level_cadquery_<board>.3mf`, or branded multicolor `../../hardware/enclosure/3mf-multicolor/case_top_two_level_<board>_multicolor.3mf` | The generated tops are named for Raspberry Pi Zero 2 W and Orange Pi Zero 2W. Fit the exact board before ordering a final print. |
| 1 | Enclosure bottom | `../../hardware/enclosure/stl/case_bottom_plate_cadquery.stl` | Bottom plate with guide walls and screw holes. |
| 1 | Main encoder knob | Print `../../hardware/enclosure/stl/encoder_cap_main_knurled_dots.stl`, use `../../hardware/enclosure/3mf-single-material/encoder_cap_main_knurled_dots.3mf`, or use `../../hardware/enclosure/3mf-multicolor/encoder_cap_main_knurled_dots_multicolor_flush.3mf` | Main encoder cap. The multicolor version uses the dot-ring marking. |
| 3 | Aux encoder knobs | Print `../../hardware/enclosure/stl/encoder_cap_aux*_ribbed_dot*.stl`, use matching `../../hardware/enclosure/3mf-single-material/encoder_cap_aux*.3mf`, or use matching `../../hardware/enclosure/3mf-multicolor/encoder_cap_aux*_multicolor_flush.3mf` | Aux 1/2/3 caps. Multicolor versions use one, two, and three dots. |
| 4 | MX keycaps | Print `../../hardware/enclosure/stl/mx_keycap_*.stl`, use matching `../../hardware/enclosure/3mf-single-material/mx_keycap_*.3mf`, use matching `../../hardware/enclosure/3mf-multicolor/mx_keycap_*_multicolor_flush.3mf`, or use any MX-stem keycap | Four NeoKey caps: back, play, shift, and function/layer. Transparent filament for the cap body lets the color LEDs shine through. |
| 8 | 9.5mm standoff pillar | Print `../../hardware/enclosure/stl/standoff_pillar_9_5mm.stl` and matching `../../hardware/enclosure/3mf-single-material/standoff_pillar_9_5mm.3mf`, or use compatible purchased stackable PCB standoffs | Use for the OLED and audio/DAC board support locations. |
| 10 | 10mm standoff pillar | Print `../../hardware/enclosure/stl/standoff_pillar_10mm.stl` and matching `../../hardware/enclosure/3mf-single-material/standoff_pillar_10mm.3mf`, or use compatible purchased stackable PCB standoffs | Use for the selected compute board, power breakout, and NeoKey support locations. NeoTrellis array pins go straight into the bottom's integrated pillars. |
| 26 | Standoff top pin | Print `../../hardware/enclosure/stl/standoff_top_pin_thin_base.stl` and matching `../../hardware/enclosure/3mf-single-material/standoff_top_pin_thin_base.3mf`, or use compatible purchased stackable PCB standoff pins | 4 compute-board + 4 audio/DAC + 4 OLED/screen + 4 NeoKey + 2 power + 8 NeoTrellis array pins = 26 total. |
| 8 | Heat-set insert | M3x6x5 heat-set insert, such as the M3 size in this [heat-set insert kit](https://de.aliexpress.com/item/1005012199553197.html) | Install in the empty top from its underside; the smooth lead-in side locates in the `4.6mm` pilot hole. |
| 8 | Screws | M3x8 socket-head cap screw, DIN 912 / ISO 4762 style | Install from the bottom; head diameter must be no larger than `6.4mm`. |
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

1. Solder the four rotary encoders into `SW1` through `SW4`.
2. Solder the low-profile sockets for the selected compute board, OLED, DAC,
   USB-C power breakout, and other socketed modules.
3. Solder the 1x5 right-angle male header at `J1` for the NeoTrellis cable.
4. Solder `D1`, the `SA5.0A` TVS diode. Put the banded cathode in `K/+5V` and
   the unbanded anode in `A/GND`. Keep the body and leads clear of nearby pads
   and metal parts.
5. Solder `C1`, the `470uF` polarized capacitor, matching the PCB polarity
   markings. The legs may remain a little long so the capacitor can bend
   sideways; prevent the legs from touching pads or metal.

Aim the horizontal NeoTrellis header toward the cable path and check the PCB
connector indicator. Solder the mating pin headers onto the plug-in compute
board and breakouts before fitting them. Their pins must point into the main
PCB's sockets when the modules sit on their standoffs. Do not install the
modules yet.

### NeoTrellis and NeoKey

The four NeoTrellis boards form one 8x8 grid.

1. Arrange the boards as upper-left, upper-right, lower-left, and lower-right
   when viewed from the play surface.
2. Turn the whole arrangement around as one plane before soldering. The address
   table below uses positions seen from the bottom after that flip, so left and
   right are swapped relative to the play surface.
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

   On the play surface, the resulting order is upper-left none/`0x2E`,
   upper-right A0/`0x2F`, lower-left A1/`0x30`, and lower-right
   A0+A1/`0x31`.

   See the [NeoTrellis address jumper photo](images/assembly/neotrellis-address-jumpers.jpg).

6. Set the NeoKey address by soldering A0, A1, A2, and A3. Its address is
   `0x3F`. See the [NeoKey address jumper photo](images/assembly/neokey-address-jumpers.jpg).

The NeoKey and NeoTrellis connector are easy to plug in backwards. Before
powering the device, check that `INT` is on the south side.

## 4. Open assembly

1. Insert the selected compute board, OLED, DAC, USB-C power breakout, and
   NeoKey into their sockets for the open-air first-boot test.
2. Connect `J1` on the main PCB to the five-pin header on the upper-left
   NeoTrellis board with the five female-to-female Dupont leads. Match `INT`,
   `VIN`, `GND`, `SCL`, and `SDA` by their labels and the [J1 pin
   reference](../../hardware/docs/pinout-and-connections.md#other-connections);
   do not assume the cable's colors or physical pin order match at both ends.
   Check the NeoTrellis orientation before applying power.
3. If your build includes the optional board antenna, connect it now and secure
   it where it cannot touch contacts, cover pads, or be pinched.
4. Connect headphones, speakers, or a mixer to the audio output.
5. Keep the assembly open and continue with [flash and first boot](flash-and-first-boot.md).

## 5. Flash and setup

Complete [flash and first boot](flash-and-first-boot.md) before installing the
enclosure. Keep the boards accessible and check what does not need the silicone
keys or the NeoKey switches yet:

- All four encoders turn and click.
- The OLED is readable.
- The NeoTrellis cable is seated at both ends, with the five signals matched as
  in [open assembly](#4-open-assembly).

Do not close the enclosure if an open-air check fails. The full grid and synth
audio checks belong in the [final check](#7-final-check) once the silicone keys
and guided NeoKey switches are fitted. If you can test either safely while the
assembly is open, do; neither requires a risky bare-switch test. For the normal
open-air test, leave the NeoKey switches off:
the enclosure top guides their pins and holds them by friction. A bare switch
test without that guide is possible if you have spare switches or a spare
NeoKey and accept the risk of bent pins or a damaged breakout; we recommend
waiting until the top is in place.

## 6. Enclosure

Power off and disconnect the cables. Remove the selected compute board's boot
microSD card and the OLED microSD card; they can catch on the case and break.
Take the plug-in boards out of their sockets after the first-boot check.
Reinsert the cards after closing the case if their openings require it.

1. Place the bottom enclosure on the bench. Seat the main PCB over the smaller
   integrated pillars on the west half, inside its locating rails.
2. Place the joined NeoTrellis array on the taller integrated pillars on the
   east half. Route its five-wire cable through the break in the left rail and
   connect the upper-left array header to `J1` if you unplugged it after the
   open-air test. Match the five signals as in [open
   assembly](#4-open-assembly); keep the cable clear of posts and pin holes.
3. Fix the NeoTrellis array to the taller pillars with its eight top pins. Use
   gentle pressure. If a pin is loose, gently squeeze its ball with pliers or
   replace it rather than crushing it.
4. Put one silicone 4x4 key membrane on each NeoTrellis board. Seat all four
   flat to form the 8x8 playing surface.
5. Push the 18 loose standoffs down through the main PCB mounting holes and
   into the smaller west-side pillars. Use eight 9.5mm standoffs for the OLED
   and audio/DAC board, and ten 10mm standoffs for the selected compute board,
   USB-C power breakout, and NeoKey.
6. Set the plug-in boards on those standoffs. Guide their pre-soldered header
   pins into the matching main-PCB sockets; do not bend or force the pins.
7. Pass the remaining 18 top pins through the mounting holes of the plug-in
   boards and press them into the loose standoffs. Check that every board sits
   flat and its header is fully seated.
8. Heat-set the eight M3 inserts into the wall pillars of the **empty top**
   from its underside, then let the top cool. Lower the top onto the assembly
   from the west side first so the ports pass through their openings, then
   lower the east side. Do not force it.

   If you want to try the NeoKey switches before fastening the case, leave the
   top unscrewed but fully seated and use its openings to guide the switches
   straight into the breakout. Reinsert the flashed boot card only if its slot
   is accessible without moving the top. Check all four presses only if you can
   power the stable assembly without pinching a cable. Power off and unplug it;
   with the top and NeoKey supported, withdraw the switches straight through
   the same guide holes. Remove the card before turning the unscrewed case over.
   If the slot is not accessible, skip this test and use the final check instead.

9. Turn the device over carefully and fasten the bottom to the top with the
   eight M3 machine screws from underneath. Tighten gently; add the optional
   rubber feet or screw-hole covers afterward.
10. Press the four encoder caps onto their shafts. Finally, fit the four Cherry
    MX switches into the NeoKey through the top openings, then press the four
    keycaps onto the switches. The top friction-holds the switches and guides
    their pins.

Tighten screws gently. If the top does not sit flat, find the interference
instead of forcing the case closed.

## 7. Final check

1. Reinsert the selected board's boot microSD card and the OLED microSD card if
   you removed them for enclosure assembly.
2. Connect power through the enclosure USB-C breakout and wait for boot.
3. Confirm that the enclosure has not blocked any controls or openings. If
   enclosure work disturbed a connection, reopen it and repeat the acceptance
   gate before use.
4. Press each of the four NeoKey switches now that the top holds them in place.
   If one does not respond, power off and reopen the case to check its seating
   and connection rather than forcing the switch deeper.
5. Check all 64 NeoTrellis cells through the silicone keys, then start a synth
   and confirm that the DAC produces audio. If either fails, power off and
   reopen the case rather than pressing harder on the playing surface.
6. Confirm that the compute-board microSD, OLED microSD, audio, USB-C power,
   and video openings are accessible. See [enclosure](enclosure.md) for the
   matching board top.

If anything is unclear, stop and use [troubleshooting](../troubleshooting.md)
before continuing.

## Source and manufacturing references

- Gerbers: [`../../hardware/pcb/gerber/gerber.zip`](../../hardware/pcb/gerber/gerber.zip)
- Schematic: [`../../hardware/pcb/octessera.kicad_sch`](../../hardware/pcb/octessera.kicad_sch)
- PCB layout: [`../../hardware/pcb/octessera.kicad_pcb`](../../hardware/pcb/octessera.kicad_pcb)
- Pin and bus reference: [`../../hardware/docs/pinout-and-connections.md`](../../hardware/docs/pinout-and-connections.md)
- Enclosure: [`enclosure.md`](enclosure.md)
- Standoff attribution: [Stackable PCB Standoff by theduckom](https://www.printables.com/model/163087-stackable-pcb-standoff), licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
