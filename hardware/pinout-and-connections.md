# Pinout and Connections

This document describes the current Cell Symphony hardware wiring for the Raspberry Pi Zero 2 W target.

## Hardware Summary

- Compute: Raspberry Pi Zero 2 W
- Grid: four NeoTrellis 4x4 boards chained as an 8x8 matrix over I2C bus 1
- Buttons: NeoKey 1x4 over I2C bus 1
- Display: SSD1351 128x128 RGB OLED over SPI
- Audio: PCM5102-class DAC over I2S
- Controls: four clickable rotary encoders wired directly to GPIO: one main encoder and three aux encoders
- Power input: USB-C breakout feeding the board +5V rail

## Power

- Power the device through the enclosure USB-C power port only.
- Do not power the device from the Raspberry Pi micro-USB power port.
- The enclosure intentionally covers the Pi micro-USB power port so it cannot be used by mistake.
- The USB-C breakout feeds the shared `+5V` rail for the Pi, OLED, NeoKey, NeoTrellis connector, and DAC.
- `C1` is a `470uf` polarized capacitor across `+5V` and `GND` to add bulk supply smoothing on the main power rail.

## Bus Allocation

### I2C bus 1

- `GPIO2` / physical pin 3: SDA
- `GPIO3` / physical pin 5: SCL
- Devices on this bus:
  - NeoKey 1x4
  - NeoTrellis chain via `J1`

### SPI bus 0

- `GPIO10` / physical pin 19: OLED MOSI
- `GPIO11` / physical pin 23: OLED SCLK
- `GPIO8` / physical pin 24: OLED CS
- `GPIO23` / physical pin 16: OLED D/C
- `GPIO16` / physical pin 36: OLED reset
- `GPIO9` / physical pin 21: wired to OLED MISO / SD path on the module footprint
- `GPIO7` / physical pin 26: OLED microSD chip select on the module footprint

### I2S

- `GPIO18` / physical pin 12: DAC BCK
- `GPIO19` / physical pin 35: DAC LRCK / WSEL
- `GPIO21` / physical pin 40: DAC DIN

## Encoder Wiring

The runtime and HAL agree on four physical encoders total.

| Ref | Role | A | B | Switch |
|---|---|---:|---:|---:|
| `SW1` | main | GPIO5 / pin 29 | GPIO6 / pin 31 | GPIO12 / pin 32 |
| `SW2` | aux1 | GPIO13 / pin 33 | GPIO25 / pin 22 | GPIO17 / pin 11 |
| `SW3` | aux2 | GPIO27 / pin 13 | GPIO4 / pin 7 | GPIO14 / pin 8 |
| `SW4` | aux3 | GPIO26 / pin 37 | GPIO24 / pin 18 | GPIO22 / pin 15 |

Notes:

- `SW3` switch uses `GPIO14`, so disable the serial console on UART TX for reliable encoder input.
- `GPIO20` is reserved for OLED microSD card detect; keep it free from I2S overlays and encoder inputs.

## Other Connections

### NeoTrellis connector `J1`

- Pin 1: INT -> `GPIO15` / physical pin 10
- Pin 2: VIN -> `+5V`
- Pin 3: GND -> `GND`
- Pin 4: SCL -> `GPIO3` / physical pin 5
- Pin 5: SDA -> `GPIO2` / physical pin 3

### NeoKey

- Shares I2C bus 1 power and data with NeoTrellis
- INT is tied into the same interrupt net as the NeoTrellis connector in the current schematic

### DAC

- Powered from `+5V`
- Connected to Pi I2S for audio output

## Source of Truth

- Schematic: `hardware/KiCAD/cellSymphony.kicad_sch`
- Netlist: `hardware/KiCAD/cellSymphony.net`
- HAL pin mapping: `crates/hal/src/pinmap.rs`
