# octessera hardware assembly manual

This guide builds the standalone Octessera instrument: PCB, soldered controls, plug-in modules, NeoTrellis grid, Raspberry Pi image, and enclosure.

The hardware is still being fit-tested. Check the current enclosure files before ordering printed parts, and do the slow fit checks before you close the box. Tiny clearances are where instruments learn humility.

If you are new here, start with the friendlier map in [`../README.md`](../README.md), then come back with a soldering iron and snacks.

## Source files

- Gerbers for PCB fabrication: [`../../release-artifacts/pcb/gerber/gerber.zip`](../../release-artifacts/pcb/gerber/gerber.zip)
- Schematic: [`../../hardware/pcb/octessera.kicad_sch`](../../hardware/pcb/octessera.kicad_sch)
- PCB layout: [`../../hardware/pcb/octessera.kicad_pcb`](../../hardware/pcb/octessera.kicad_pcb)
- Wiring reference: [`pinout-and-connections.md`](pinout-and-connections.md)
- Enclosure reference: [`enclosure.md`](enclosure.md)
- User docs home: [`../README.md`](../README.md)

## BOM

### PCB and electronics

| Qty | Item | Exact/current part | Notes |
|---:|---|---|---|
| 1 | Custom PCB | Fabricate from [`../../release-artifacts/pcb/gerber/gerber.zip`](../../release-artifacts/pcb/gerber/gerber.zip) | Order as a two-layer PCB unless the Gerber notes say otherwise. |
| 4 | NeoTrellis 4x4 driver PCB | [Adafruit `3954`](https://www.adafruit.com/product/3954), Mouser `485-3954` | Forms the 8x8 grid. |
| 4 | Silicone 4x4 keypad | [Adafruit `1611`](https://www.adafruit.com/product/1611), Mouser `485-1611` | One per NeoTrellis board. |
| 1 | NeoKey 1x4 QT | [Adafruit `4980`](https://www.adafruit.com/product/4980), Mouser `485-4980` | Holds the four Cherry MX keys. |
| 1 | Raspberry Pi Zero 2 W with header | [Raspberry Pi Zero 2 W](https://www.raspberrypi.com/products/raspberry-pi-zero-2-w/), Mouser `358-SC0721`, Raspberry Pi `SC0721` | Use the headered version. |
| 1 | SSD1351 OLED breakout with microSD holder | [Adafruit `1431`](https://www.adafruit.com/product/1431), Mouser `485-1431` | SPI display. |
| 1 | PCM5102 I2S DAC | [Adafruit `6250`](https://www.adafruit.com/product/6250), Mouser `485-6250` | Line/headphone output path. |
| 1 | USB-C power breakout | [Adafruit `4090`](https://www.adafruit.com/product/4090), Mouser `485-4090` | Power the device here, not through the Pi. |
| 4 | Horizontal rotary encoder with switch | [RS `781-6811`](https://at.rs-online.com/web/p/mechanische-drehgeber/7816811), Bourns `PEC12R-4225F-S0024` | The PCB uses four encoders. |
| 1 | Polarized capacitor | [`470uF`, 16V, radial, about 8x12mm](https://de.aliexpress.com/item/1005010415990713.html) | PCB footprint: `CP_Radial_D8.0mm_P3.50mm`. Any equivalent 470uF polarized radial capacitor with 3.5mm lead pitch and >5V rating is fine. |
| 1 | 1x5 right-angle female socket/header, 2.54mm pitch | [2.54mm right-angle female header strip](https://de.aliexpress.com/item/32896617287.html) | Cut to 5 pins for the NeoTrellis connector on the PCB. |
| 1 | 5-wire female-to-male Dupont cable, about 5cm | [Female-to-male Dupont jumper cable set](https://de.aliexpress.com/item/1005003683781229.html) | Use five adjacent leads, about 5cm long, to connect the NeoTrellis array to the PCB. |
| several | Low-profile female header/socket strips, 2.54mm pitch | [Round-pin 2.54mm header/socket strip](https://de.aliexpress.com/item/1005006673257121.html) or [round-pin 2.54mm header/socket strip](https://de.aliexpress.com/item/4001122376295.html) | Cut to length for Pi, OLED, DAC, power breakout, and other plug-in modules. Confirm the socket height before ordering. |
| 4 | Cherry MX-compatible key switches | [Cherry MX Black switches](https://www.amazon.de/-/en/CHERRY-Mechanical-Keyboard-Switches-without/dp/B0CBS4HJJR?th=1), or any MX-compatible switch | Install into the NeoKey after bring-up. |
| 1 | MicroSD card for Raspberry Pi | 16GB or larger recommended | Flash the release image. |
| 1 | USB-C power supply | Regulated 5V supply, 3A minimum; 4A recommended for extra LED headroom | Connect only to the USB-C breakout. A 2A supply is likely marginal once the Pi and LEDs are running together. |
| 1 | Audio cable/headphones/speaker | 3.5mm audio | Used for test and operation. |

### 3D printed and mechanical parts

The enclosure is printable, including the dowel/standoff and top-pin system that holds the boards and case together. Screws and heat-set inserts are still recommended for the tidiest, most travel-safe build, but they are optional rather than mandatory.

Note that there are a lot of parts that fit snugly into each other and into the components' mount holes, with rather tight tolerances. Make sure your printer is well calibrated, or test things out with partial prints ahead of time and scale e.g. pins and standoffs accordingly on X/Y axis to make them fit the pillars.

| Qty | Item | File/spec | Notes |
|---:|---|---|---|
| 1 | Enclosure top | `../../release-artifacts/enclosure/stl/case_top_two_level_cadquery.stl` | Generated from CadQuery. STEP file is also checked in. |
| 1 | Enclosure bottom | `../../release-artifacts/enclosure/stl/case_bottom_plate_cadquery.stl` | Current bottom plate with guide walls and screw holes. |
| 1 | Main encoder knob | Print `../../release-artifacts/enclosure/stl/encoder_cap_main_knurled_dots.stl` or use `../../release-artifacts/enclosure/3mf-multicolor/encoder_cap_main_knurled_dots_multicolor_flush.3mf` | Main encoder cap. Printed version uses the dot-ring marking. |
| 3 | Aux encoder knobs | Print `../../release-artifacts/enclosure/stl/encoder_cap_aux*_ribbed_dot*.stl` or use matching `../../release-artifacts/enclosure/3mf-multicolor/encoder_cap_aux*_multicolor_flush.3mf` | Aux 1/2/3 caps. Printed versions use one, two, and three dots. |
| 4 | MX keycaps | Print `../../release-artifacts/enclosure/stl/mx_keycap_*.stl`, use matching `../../release-artifacts/enclosure/3mf-multicolor/mx_keycap_*_multicolor_flush.3mf`, or use any MX-stem keycap | Four NeoKey caps: back, play, shift, and function/layer. Transparent filament for the cap body (so the color LEDs of the NeoKey can shine through) and dark filament for the symbols works well, either with a filament swap for the last layers or multi-material printing. |
| 8 | 9.5mm standoff pillar | Print `../../release-artifacts/enclosure/stl/standoff_pillar_9_5mm.stl` and matching `../../release-artifacts/enclosure/3mf-multicolor/standoff_pillar_9_5mm.3mf`, or use compatible purchased stackable PCB standoffs | Use for the OLED and audio/DAC board support locations. |
| 10 | 10mm standoff pillar | Print `../../release-artifacts/enclosure/stl/standoff_pillar_10mm.stl` and matching `../../release-artifacts/enclosure/3mf-multicolor/standoff_pillar_10mm.3mf`, or use compatible purchased stackable PCB standoffs | Use for the Raspberry Pi, power breakout, and NeoKey support locations. NeoTrellis array pins go straight into the bottom's integrated pillars, so they do not need separate standoff pillars. |
| 26 | Standoff top pin | Print `../../release-artifacts/enclosure/stl/standoff_top_pin_thin_base.stl` and matching `../../release-artifacts/enclosure/3mf-multicolor/standoff_top_pin_thin_base.3mf`, or use compatible purchased stackable PCB standoff pins | 4 Pi + 4 audio/DAC + 4 OLED/screen + 4 NeoKey + 2 power + 8 NeoTrellis array pins = 26 total. |
| 8 | Heat-set insert | M3x6x5 heat-set insert, such as the M3 size in this [heat-set insert kit](https://de.aliexpress.com/item/1005012199553197.html) | Recommended, but optional if the printed dowel/top-pin system grips well enough. Insert from the underside of the top. The smooth lead-in side should locate in the `4.6mm` pilot hole before heat-setting. |
| 8 | Screws | M3x8 socket-head cap screw, DIN 912 / ISO 4762 style | Recommended, but optional if the printed dowel/top-pin system grips well enough. Installed from the bottom. Use a head diameter no larger than `6.4mm` so it fits the counterbores. |
| 8 | Rubber feet or screw-hole plugs | Small adhesive feet | Optional, covers bottom screw holes and prevents sliding. |

Standoff STL attribution and purchase/source reference: the standoff models are based on [Stackable PCB Standoff by theduckom](https://www.printables.com/model/163087-stackable-pcb-standoff), licensed under [Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/).

### Tools and consumables

- Soldering iron and solder.
- Flush cutters.
- Optional: small screwdriver for bottom screws.
- Optional: heat-set insert tool.
- Optional: pliers for gently tuning top-pin fit.
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

The horizontal socket for the NeoTrellis array can be a tight fit near the case pillar that supports the NeoTrellis. Consider bending the socket legs slightly before soldering, or use a vertical connector if you prefer. If you plug the cable in before mounting the NeoTrellis array on the pillars, it will be very tight, but will fit (barely).

Do not install plug-in modules yet.

## Build the NeoTrellis array

The four NeoTrellis boards form one 8x8 grid.

1. Arrange the boards as viewed from the play surface:
   - upper left
   - upper right
   - lower left
   - lower right
2. Pretend they are already soldered together and turn them around like they are one plane already (meaning left and right switch places).
3. Solder the boards together with pin links between adjacent edges.
4. Add the external connector pins to the left side of the upper-left NeoTrellis board.
5. Set the NeoTrellis addresses (now that they are switched! This is looking at them from the bottom!):

   | Position | Jumpers | Address |
   |---|---|---:|
   | upper left | A0 | `0x2F` |
   | upper right | none | `0x2E` |
   | lower left | A0 + A1 | `0x31` |
   | lower right | A1 | `0x30` |

Use the reference photos to confirm the soldered address jumper pads:

- [NeoTrellis address jumper pads](images/assembly/neotrellis-address-jumpers.jpg). Note that there is solder on A1 on the upper left in the picture, but it is not shorted. I shorted it by mistake, separated the pads but and then not clean it up completely. So don't get confused by those drops of solder.

5. Set the NeoKey address by soldering A0, A1, A2, and A3. The expected address is `0x3F`.

- [NeoKey address jumper pads](images/assembly/neokey-address-jumpers.jpg)

The NeoKey and NeoTrellis connector are the two parts that are easiest to plug in backwards. Double-check their orientation before powering the device. The simplest check is that `INT` should always be on the south side.

## Flash the Raspberry Pi image

1. Download the latest release image from the project releases. It is named like:

   ```text
   octessera-<version>-pi-zero-2w.img.zip
   ```

2. Flash it to the Pi microSD card with Raspberry Pi Imager.
3. Configure WiFi, SSH, hostname, and locale in Raspberry Pi Imager if you need network access.
4. Insert the microSD card into the Raspberry Pi.

You can flash the downloaded `.img.zip` directly with Raspberry Pi Imager by choosing **Use custom** and selecting the ZIP. You do not need to extract the ZIP first.

The release also includes Raspberry Pi Imager metadata in two places:

- inside the image ZIP as `os_list.rpi-imager-manifest`;
- next to the image ZIP as `octessera-<version>-pi-zero-2w.rpi-imager-manifest`.

If you want octessera to appear as a custom OS entry in Raspberry Pi Imager, use the standalone `.rpi-imager-manifest` release asset as Imager's custom repository manifest. Loading the manifest/custom image this way lets Raspberry Pi Imager configure locale, WiFi, SSH, hostname, and user settings before flashing.

In Raspberry Pi Imager, open **App Options**, press **Edit** next to **Content Repository**, then choose one of these options:

- **Use custom file**: select the downloaded `.rpi-imager-manifest` file.
- **Use custom URL**: paste the manifest URL from the GitHub release.

Then press **Apply and Restart**. After Imager restarts, the octessera image will show up in the **OS** list after you have selected your device type.

Reference screenshots:

- [Raspberry Pi Imager options](images/assembly/rpi-imager-options.png)
- [Content Repository setting](images/assembly/rpi-content-repository.png)
- [Custom repository source](images/assembly/rpi-repository-source.png)

You can also start Imager with the manifest URL from the command line, for example:

```powershell
rpi-imager --repo "https://github.com/nexxyz/octessera/releases/download/v<version>/octessera-<version>-pi-zero-2w.rpi-imager-manifest"
```

Use the actual release tag and version from the release page. The embedded manifest inside the ZIP is for packaged metadata; do not extract the ZIP just to load the manifest.

For manual Pi setup and developer deploy/update workflows, see [`../../docs/development-workflows.md`](../../docs/development-workflows.md).

## First electrical assembly

1. Insert the Raspberry Pi, OLED, DAC, USB-C power breakout, and NeoKey into their sockets.
2. Connect the NeoTrellis array to the PCB with the short 5-wire female-to-male Dupont cable.
3. Install the Cherry MX switches into the NeoKey.
4. Add the four keycaps. Use either printed keycaps from `release-artifacts/enclosure/stl/` or purchased MX-stem keycaps. Install them before running diagnostics so the key checks match the assembled control feel.
5. Connect audio output to headphones, speakers, or a mixer.
6. Connect power to the USB-C breakout.

Do not power the device from the Raspberry Pi power connector.

Before applying power, check the NeoKey and NeoTrellis connector orientation again. `INT` should be on the south side.

Wait for the Pi to boot. First boot can take a while, but you should soon see the splashscreen.

Once it is up, check that none of the elements report an error. The OLED, NeoKey, and NeoTrellis can flash magenta in case of an electrical issue. The top NeoKey should light up magenta; that is intended because it is the Back button.

Then turn the main encoder to select the System menu, and click the main encoder to enter it. In the System menu, scroll to and run the hardware diagnostics before assembling the enclosure. Complete the guided checks for display, grid, keys, encoders, and audio while the boards are still accessible. The diagnostics can also detect inconsistent button or encoder signals. A couple of warnings are usually okay, but if the diagnostics report actual errors, double-check your soldering and components.

If the hardware does not come up, stop and use the manual hardware checks in [`../../hardware/docs/manual-hardware-test-suite.md`](../../hardware/docs/manual-hardware-test-suite.md).

## Enclosure assembly

Only assemble the enclosure after the electrical test and System-menu diagnostics pass.

Before you start with the enclosure, you need to remove the keyswitches from the NeoKey, but you can leave the keycaps on.
Remove the Raspberry Pi microSD card and OLED microSD card first. They can catch on the enclosure and break during installation.

1. Place the bottom enclosure on the bench.
2. Put the PCB and NeoTrellis array onto the taller pillars.
3. Add the 18 separate standoff pillars between the bottom supports and the plug-in modules:
   - Use 8 of the 9.5mm standoff pillars for the OLED and audio/DAC board.
   - Use the 10 pieces of 10mm standoff pillars for the Raspberry Pi, power breakout, and NeoKey.
   - Printing the 9.5mm, and 10mm support parts in different colors makes them easier to tell apart during assembly.
   - For each plug-in module, remove the module, add the standoff, then reinstall the module. The standoffs' pins might take a little force to go through the component's mount holes and into the pillars, but if you are firm but gentle it will work.
   - You do not need separate standoff pillars for the NeoTrellis array. Its eight pins go straight into the integrated bottom pillars.
4. Press in all 26 top pins:
   - 18 pins go into the separate standoff pillars.
   - 8 pins go into the NeoTrellis array's integrated bottom pillars. It might be a bit tight due to the female connectors on the bottom, since the pillars are flared a bit for stability, but it will end up fitting nicely.
   - During final assembly, a small drop of glue on the top-pin can keep pins from falling out if the device is turned upside down.
   - The dowel/top-pin system can hold the enclosure together on its own. If a pin does not secure enough, use pliers to gently squeeze the ball at the end of the pin for a tighter fit. Gently is the magic word here; the tiny mushroom is brave, not immortal.
5. Place one silicone 4x4 keypad on each NeoTrellis board. Make sure the pads sit flat and line up with the 8x8 opening before closing the case.
6. Optional but recommended: insert the M3 heat-set inserts into the underside of the enclosure top with a soldering iron or insert tool.
7. Place the enclosure top over the assembly. It is easiest to slip the top on from the left/west side first so the ports pass through their holes, then lower the right/east side. The guide walls and standoff pins should locate the parts without forcing them.
8. Install the four printed encoder knobs from `release-artifacts/enclosure/stl/` or the matching multicolor 3MF files. Check the fit before pressing them fully down: they should slide on with a decent amount of friction, but not so tightly that removal would require force. If the fit is too tight, scale the encoder cap models up slightly in the X/Y plane before printing another set.
9. Turn the device over carefully.
10. Optional but recommended: install the bottom screws into the recessed holes and tighten them into the heat inserts. If the printed pins already hold everything securely and the device will live a gentle bench life, you can leave the screws out.
11. Add rubber feet or screw-hole covers if desired.

Tighten screws gently. If the top does not sit flat, stop and find the interference instead of forcing the case closed.

## Final checks

1. Connect power through the USB-C breakout.
2. Wait for boot.
3. Confirm input from all encoders, NeoKey switches, and NeoTrellis buttons.
4. Confirm the OLED is readable.
5. Confirm audio output from the DAC.
6. Confirm the Pi microSD, OLED microSD, audio, USB-C power, Pi mini-HDMI, and Pi USB data openings are accessible.
