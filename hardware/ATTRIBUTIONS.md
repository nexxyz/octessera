# Hardware attributions and source notes

This document covers known hardware references in the Octessera PCB, enclosure,
and installed-library workflow. It is descriptive and does not imply
endorsement by any referenced project or manufacturer.

## Enclosure standoff

The standoff is based on [Stackable PCB Standoff by theduckom](https://www.printables.com/model/163087-stackable-pcb-standoff),
licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
Octessera modified it into parametric dimensions with Octessera-specific pins
and sockets, and exported the resulting enclosure artifacts.

## Adafruit modules and converted footprints

The PCB references these Adafruit products. The product pages are the official
product references; the linked directories are the official Adafruit CAD-parts
library where the product directory is available.

| Product | Official product page | Official design reference |
| --- | --- | --- |
| NeoKey 1x4 QT, product 4980 | [adafruit.com/product/4980](https://www.adafruit.com/product/4980) | [Adafruit CAD Parts: 4980 NeoKey 1x4 QT](https://github.com/adafruit/Adafruit_CAD_Parts/tree/main/4980%20NeoKey%201x4%20QT) |
| NeoTrellis 4x4, product 3954 | [adafruit.com/product/3954](https://www.adafruit.com/product/3954) | [Adafruit CAD Parts: 3954 Adafruit NeoTrellis](https://github.com/adafruit/Adafruit_CAD_Parts/tree/main/3954%20Adafruit%20NeoTrellis) |
| PCM5102 I2S DAC, product 6250 | [adafruit.com/product/6250](https://www.adafruit.com/product/6250) | [Adafruit CAD Parts library](https://github.com/adafruit/Adafruit_CAD_Parts) |
| 1.5 inch SSD1351 OLED breakout, product 1431 | [adafruit.com/product/1431](https://www.adafruit.com/product/1431) | [Adafruit CAD Parts library](https://github.com/adafruit/Adafruit_CAD_Parts) |
| USB Type-C downstream breakout, product 4090 | [adafruit.com/product/4090](https://www.adafruit.com/product/4090) | [Adafruit CAD Parts: 4090 USB C Breakout](https://github.com/adafruit/Adafruit_CAD_Parts/tree/main/4090%20USB%20C%20Breakout) |

The user confirmed that the checked-in footprints for these modules were
converted from upstream CAD. Adafruit's [Eagle library](https://github.com/adafruit/Adafruit-Eagle-Library)
contains a public-domain statement for that library; this document does not
generalize that statement to the products, their product files, or unknown
third-party files. The exact terms for the product CAD files remain unresolved.
Until those terms are confirmed, do not publish standalone copies of the CAD or
library archives.

The `adafruit-modules` identifiers used by the KiCad project distinguish
referenced installed KiCad library/model content from a vendored general-purpose
library archive. This repository does not claim a blanket license for installed
KiCad models or for any library content not expressly covered above.

## Raspberry Pi CAD reference

The Raspberry Pi Zero 2 W part reference points to the
[SnapEDA Raspberry Pi Zero 2 W part](https://www.snapeda.com/parts/RASPBERRY%20PI%20ZERO%202%20W/Raspberry+Pi/view-part/?ref=eda).
SnapEDA's externally documented terms identify the part as CC BY-SA 4.0 with
Design Exception 1.0. See the [CC BY-SA 4.0 license](https://creativecommons.org/licenses/by-sa/4.0/)
and the exact SnapEDA source link above. This is a descriptive source
reference, not an endorsement by Raspberry Pi or SnapEDA.

Raspberry Pi is a trademark of Raspberry Pi Ltd. References to Raspberry Pi,
Raspberry Pi OS, pi-gen, Armbian, Adafruit, SnapEDA, and KiCad here and in the
hardware documentation are descriptive references to components, software, or
source material, not claims of affiliation or endorsement.
