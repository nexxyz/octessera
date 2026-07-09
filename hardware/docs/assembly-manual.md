# Cell Symphony Hardware Assembly Manual

This guide builds the standalone Cell Symphony instrument: PCB, soldered controls, plug-in modules, NeoTrellis grid, Raspberry Pi image, and enclosure.

The hardware is still being fit-tested. Check the current enclosure files before ordering printed parts.

## Source files

- Gerbers for PCB fabrication: [`../../release-artifacts/pcb/gerber/gerber.zip`](../../release-artifacts/pcb/gerber/gerber.zip)
- Schematic: [`../pcb/cellSymphony.kicad_sch`](../pcb/cellSymphony.kicad_sch)
- PCB layout: [`../pcb/cellSymphony.kicad_pcb`](../pcb/cellSymphony.kicad_pcb)
- Wiring reference: [`pinout-and-connections.md`](pinout-and-connections.md)
- Pi setup and bring-up: [`pi-bring-up.md`](pi-bring-up.md)
- Enclosure reference: [`../enclosure/README.md`](../enclosure/README.md)

## BOM

### PCB and electronics

| Qty | Item | Exact/current part | Notes |
|---:|---|---|---|
| 1 | Custom PCB | Fabricate from [`../../release-artifacts/pcb/gerber/gerber.zip`](../../release-artifacts/pcb/gerber/gerber.zip) | Order as a two-layer PCB unless the Gerber notes say otherwise. |
| 4 | NeoTrellis 4x4 driver PCB | [Mouser `485-3954`](https://www.mouser.com/ProductDetail/Adafruit/3954), Adafruit `3954` | Forms the 8x8 grid. |
| 4 | Silicone 4x4 keypad | [Mouser `485-1611`](https://www.mouser.com/ProductDetail/Adafruit/1611), Adafruit `1611` | One per NeoTrellis board. |
| 1 | NeoKey 1x4 QT | [Mouser `485-4980`](https://www.mouser.com/ProductDetail/Adafruit/4980), Adafruit `4980` | Holds the four Cherry MX keys. |
| 1 | Raspberry Pi Zero 2 W with header | [Mouser `358-SC0721`](https://www.mouser.com/ProductDetail/Raspberry-Pi/SC0721), Raspberry Pi `SC0721` | Use the headered version. |
| 1 | SSD1351 OLED breakout with microSD holder | [Mouser `485-1431`](https://www.mouser.com/ProductDetail/Adafruit/1431), Adafruit `1431` | SPI display. |
| 1 | PCM5102 I2S DAC | [Mouser `485-6250`](https://www.mouser.com/ProductDetail/Adafruit/6250), Adafruit `6250` | Line/headphone output path. |
| 1 | USB-C power breakout | [Mouser `485-4090`](https://www.mouser.com/ProductDetail/Adafruit/4090), Adafruit `4090` | Power the device here, not through the Pi. |
| 5 | Horizontal rotary encoder with switch | [Mouser `652-PEC12R-4225F-S24`](https://www.mouser.com/ProductDetail/Bourns/PEC12R-4225F-S0024), Bourns `PEC12R-4225F-S0024` | The PCB uses four encoders; buy one spare. |
| 1 | Main encoder knob | Print `../../release-artifacts/enclosure/stl/encoder_cap_main_knurled_dots.stl`, use `../../release-artifacts/enclosure/3mf-multicolor/encoder_cap_main_knurled_dots_multicolor_flush.3mf`, or buy [Mouser `450-BA600`](https://www.mouser.com/ProductDetail/Eagle-Plastic-Devices/450-BA600) | Main encoder cap. Printed version uses the dot-ring marking. |
| 3 | Aux encoder knobs | Print `../../release-artifacts/enclosure/stl/encoder_cap_aux*_ribbed_dot*.stl`, use matching `../../release-artifacts/enclosure/3mf-multicolor/encoder_cap_aux*_multicolor_flush.3mf`, or buy [Mouser `450-BA600`](https://www.mouser.com/ProductDetail/Eagle-Plastic-Devices/450-BA600) or [Mouser `485-5530`](https://www.mouser.com/ProductDetail/Adafruit/5530) | Aux 1/2/3 caps. Printed versions use one, two, and three dots. |
| 2 | STEMMA QT / Qwiic JST-SH cable, 200mm | [Mouser `485-4401`](https://www.mouser.com/ProductDetail/Adafruit/4401), Adafruit `4401` | For module bring-up or alternate I2C wiring. |
| 1 | Polarized capacitor | [`470uF`, 16V, radial, about 8x12mm](https://de.aliexpress.com/item/1005010415990713.html) | PCB footprint: `CP_Radial_D8.0mm_P3.50mm`. Any equivalent 470uF polarized radial capacitor with 3.5mm lead pitch and >5V rating is fine. |
| 1 | 1x5 right-angle female socket/header, 2.54mm pitch | [2.54mm right-angle female header strip](https://de.aliexpress.com/item/32896617287.html) | Cut to 5 pins for the NeoTrellis connector on the PCB. |
| 1 | 5-wire female-to-male Dupont cable, about 5cm | [Female-to-male Dupont jumper cable set](https://de.aliexpress.com/item/1005003683781229.html) | Use five adjacent leads, about 5cm long, to connect the NeoTrellis array to the PCB. |
| several | Low-profile female header/socket strips, 2.54mm pitch | [Round-pin 2.54mm header/socket strip](https://de.aliexpress.com/item/1005006673257121.html) or [round-pin 2.54mm header/socket strip](https://de.aliexpress.com/item/4001122376295.html) | Cut to length for Pi, OLED, DAC, power breakout, and other plug-in modules. Confirm the socket height before ordering. |
| 4 | Cherry MX-compatible key switches | [Mouser `540-MX1A-G1NW`](https://www.mouser.com/ProductDetail/CHERRY/MX1A-G1NW), Cherry MX Black, or any MX-compatible switch | Install into the NeoKey after bring-up. |
| 4 | MX keycaps | Print `../../release-artifacts/enclosure/stl/mx_keycap_*.stl`, use matching `../../release-artifacts/enclosure/3mf-multicolor/mx_keycap_*_multicolor_flush.3mf`, buy [Mouser `540-G99-1779ZUB`](https://www.mouser.com/ProductDetail/CHERRY/G99-1779ZUB), or use any MX-stem keycap | Four NeoKey caps: back, play, shift, and function/layer. |
| 1 | MicroSD card for Raspberry Pi | 16GB or larger recommended | Flash the release image. |
| 1 | USB-C power supply | 5V supply with enough current for Pi + LEDs | Connect only to the USB-C breakout. |
| 1 | Audio cable/headphones/speaker | 3.5mm audio | Used for test and operation. |

### 3D printed and mechanical parts

| Qty | Item | File/spec | Notes |
|---:|---|---|---|
| 1 | Enclosure top | `../../release-artifacts/enclosure/stl/case_top_two_level_cadquery.stl` | Generated from CadQuery. STEP file is also checked in. |
| 1 | Enclosure bottom | `../../release-artifacts/enclosure/stl/case_bottom_plate_cadquery.stl` | Current bottom plate with guide walls and screw holes. |
| 18 | Standoff pillar | Print `../../release-artifacts/enclosure/stl/standoff_pillar_9mm.stl` and matching `../../release-artifacts/enclosure/3mf-multicolor/standoff_pillar_9mm.3mf`, or use compatible purchased stackable PCB standoffs | For Pi, OLED, DAC/audio, power breakout, and NeoKey support locations. NeoTrellis array pins go straight into the bottom's integrated pillars, so they do not need separate standoff pillars. |
| 26 | Standoff top pin | Print `../../release-artifacts/enclosure/stl/standoff_top_pin_thin_base.stl` and matching `../../release-artifacts/enclosure/3mf-multicolor/standoff_top_pin_thin_base.3mf`, or use compatible purchased stackable PCB standoff pins | 4 Pi + 4 audio/DAC + 4 OLED/screen + 4 NeoKey + 2 power + 8 NeoTrellis array pins = 26 total. |
| 8 | Heat-set insert | [M3 heat-set insert](https://de.aliexpress.com/item/1005012199553197.html), about `4.0-4.2mm` outer diameter and `5-6mm` long | Insert from the underside of the top. The linked kit includes multiple sizes; use the M3 inserts that fit the `4.2mm` pilot holes. |
| 8 | Screws | M3 x 8mm socket-head cap screw, DIN 912 / ISO 4762 style | Installed from the bottom. Use a head diameter no larger than `6.4mm` so it fits the counterbores. |
| 8 | Rubber feet or screw-hole plugs | Small adhesive feet | Optional, covers bottom screw holes and prevents sliding. |

Standoff STL attribution and purchase/source reference: the standoff models are based on [Stackable PCB Standoff by theduckom](https://www.printables.com/model/163087-stackable-pcb-standoff), licensed under [Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/).

### Tools and consumables

- Soldering iron and solder.
- Flush cutters.
- Small screwdriver and a longish bit for bottom screws.
- Heat-set insert tool or soldering iron tip for M3 inserts.
- Multimeter.
- Raspberry Pi Imager.
- Optional: continuity tester, tweezers, helping hands, and magnifier.

## Before soldering

1. Inspect the PCB for visible manufacturing defects.
2. Confirm the PCB matches the current Gerber zip.
3. Sort sockets, headers, and modules before soldering.
4. Keep every module oriented the same way it will sit in the enclosure.
5. Mark the correct side of every module before soldering headers.

Headers on the wrong side are difficult to fix. Check the silkscreen, enclosure orientation, and module footprint before soldering. Best double-check every header before soldering it: the correct side matters for the PCB sockets, Pi, OLED, DAC/audio breakout, power breakout, NeoKey, and NeoTrellis connector.

Reference photos:

- [Soldered PCB overview](images/assembly/soldered-pcb.jpg)
- [Raspberry Pi Zero 2 W header side](images/assembly/pi-zero-2w.jpg)
- [OLED header side](images/assembly/oled.jpg)
- [Audio breakout header side](images/assembly/audio-breakout.jpg)
- [Power breakout header side](images/assembly/power-breakout.jpg)

## Solder the main PCB

Solder low-profile sockets and headers first. They define module height and alignment.

1. Solder the low-profile sockets for the Raspberry Pi, OLED, DAC, USB-C power breakout, and any other socketed modules.
2. Solder the 1x5 right-angle female socket for the NeoTrellis connector.
3. Solder `C1`, the `470uF` polarized capacitor. Match polarity to the PCB markings.
4. Solder the four rotary encoders into `SW1` through `SW4`.

You can leave the capacitor legs a little longer than usual so the capacitor can bend sideways if you need to save vertical space. Keep polarity correct and make sure the legs cannot short against nearby pads or metal parts.

The horizontal socket for the NeoTrellis array can be a tight fit near the case pillar that supports the NeoTrellis. Consider bending the socket legs slightly before soldering, or use a vertical connector if your build has enough space. If you plug the cable in before mounting the NeoTrellis array on the pillars, it will be very tight, but will fit (barely).

Do not install plug-in modules yet.

## Build the NeoTrellis array

The four NeoTrellis boards form one 8x8 grid.

1. Arrange the boards as viewed from the play surface:
   - upper left
   - upper right
   - lower left
   - lower right
2. Solder the boards together with pin links between adjacent edges.
3. Add the external connector pins to the left side of the upper-left NeoTrellis board.
4. Set the NeoTrellis addresses:

   | Position | Jumpers | Address |
   |---|---|---:|
   | upper left | none | `0x2E` |
   | upper right | A0 | `0x2F` |
   | lower left | A1 | `0x30` |
   | lower right | A0 + A1 | `0x31` |

Use the reference photos to confirm the soldered address jumper pads:

- [NeoTrellis address jumper pads](images/assembly/neotrellis-address-jumpers.jpg)

5. Set the NeoKey address by soldering A0, A1, A2, and A3. Leave A4 open. The expected address is `0x3F`.

- [NeoKey address jumper pads](images/assembly/neokey-address-jumpers.jpg)

The NeoKey and NeoTrellis connector are the two parts that are easiest to plug in backwards. Double-check their orientation before powering the device. The simplest check is that `INT` should always be on the south side.

## Flash the Raspberry Pi image

1. Download the latest release image from the project releases. It is named like:

   ```text
   CellSymphony-<version>-pi-zero-2w.img.zip
   ```

2. Flash it to the Pi microSD card with Raspberry Pi Imager.
3. Configure WiFi, SSH, hostname, and locale in Raspberry Pi Imager if you need network access.
4. Insert the microSD card into the Raspberry Pi.

For manual Pi setup and diagnostics, see [`pi-bring-up.md`](pi-bring-up.md).

## First electrical assembly

1. Insert the Raspberry Pi, OLED, DAC, USB-C power breakout, and NeoKey into their sockets.
2. Connect the NeoTrellis array to the PCB with the short 5-wire female-to-male Dupont cable.
3. Install the Cherry MX switches into the NeoKey.
4. Add the four keycaps. Use either printed keycaps from `release-artifacts/enclosure/stl/` or purchased MX-stem keycaps.
5. Connect audio output to headphones, speakers, or a mixer.
6. Connect power to the USB-C breakout.

Do not power the device from the Raspberry Pi power connector.

Before applying power, check the NeoKey and NeoTrellis connector orientation again. `INT` should be on the south side.

Wait for the Pi to boot. First boot can take a while. Then check:

- NeoKey LEDs/input.
- NeoTrellis LEDs/input.
- OLED output.
- Encoder turns and presses.
- Audio output.

Then run the hardware diagnostics from the System menu before assembling the enclosure. Complete the guided checks for display, grid, keys, encoders, and audio while the boards are still accessible.

If the hardware does not come up, stop and use the bring-up checks in [`pi-bring-up.md`](pi-bring-up.md).

## Enclosure assembly

Only assemble the enclosure after the electrical test and System-menu diagnostics pass.

1. Place the bottom enclosure on the bench.
2. Put the PCB and NeoTrellis array onto the bottom.
   - Remove the Raspberry Pi microSD card and OLED microSD card first. They can catch on the enclosure and break during installation.
3. Add the 18 separate standoff pillars between the bottom supports and the plug-in modules:
   - Use the printed standoff pillars or compatible purchased stackable standoffs.
   - For each plug-in module, remove the module, add the standoff, then reinstall the module.
   - Do not add separate standoff pillars for the NeoTrellis array. Its eight pins go straight into the integrated bottom pillars.
4. Press in all 26 top pins:
   - 18 pins go into the separate standoff pillars.
   - 8 pins go into the NeoTrellis array's integrated bottom pillars.
5. Insert the M3 heat-set inserts into the underside of the enclosure top with a soldering iron or insert tool.
6. Place the enclosure top over the assembly. The guide walls and standoff pins should locate the parts without forcing them.
7. Install the four encoder knobs. Use printed knobs from `release-artifacts/enclosure/stl/` or purchased D-shaft knobs.
8. Turn the device over carefully.
9. Install the bottom screws into the recessed holes and tighten them into the heat inserts.
10. Add rubber feet or screw-hole covers if desired.

Tighten screws gently. If the top does not sit flat, stop and find the interference instead of forcing the case closed.

## Final checks

1. Connect power through the USB-C breakout.
2. Wait for boot.
3. Confirm input from all encoders, NeoKey switches, and NeoTrellis buttons.
4. Confirm the OLED is readable.
5. Confirm audio output from the DAC.
6. Confirm the Pi microSD, OLED microSD, audio, USB-C power, Pi mini-HDMI, and Pi USB data openings are accessible.

## Things to verify before ordering in quantity

- Confirm the selected M3 heat-set inserts fit the `4.2mm` pilot holes and that M3 x 8mm socket-head screws reach the inserts cleanly after printing.
- Final fit of the printed bottom, top, standoff pillars, top pins, keycaps, and encoder knobs.
- Whether the generic sockets you buy match the intended low profile.
- Whether the 5cm Dupont cable has enough slack after the NeoTrellis array is installed.
- Port alignment after printing with your printer and slicer.
