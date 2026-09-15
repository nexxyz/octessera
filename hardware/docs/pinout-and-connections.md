# Octessera pinout and connections

Technical reference for the Octessera PCB, Raspberry Pi Zero 2 W buses, control
inputs, display, and DAC connections. The Raspberry table below applies only to
the Raspberry Pi build.

## Raspberry Pi Zero 2 W

### Hardware summary

- Compute: Raspberry Pi Zero 2 W
- Grid: four NeoTrellis 4x4 boards chained as an 8x8 matrix over I2C bus 1
- Buttons: NeoKey 1x4 over I2C bus 1
- Display: SSD1351 128x128 RGB OLED over SPI
- Audio: PCM5102-class DAC over I2S
- Controls: four clickable rotary encoders wired directly to GPIO
- Power input: USB-C breakout feeding the board `+5V` rail

### Power

- The USB-C breakout feeds the shared `+5V` rail for the Pi, OLED, NeoKey,
  NeoTrellis connector, and DAC.
- `C1` is a `470uF` polarized capacitor across `+5V` and `GND`.
- The Raspberry Pi micro-USB power connector is not used by the Octessera PCB;
  power enters through the enclosure USB-C breakout.

### I2C bus 1

- `GPIO2` / physical pin 3: SDA
- `GPIO3` / physical pin 5: SCL
- NeoKey 1x4: `0x3F`
- NeoTrellis chain through `J1`: `0x2E`, `0x2F`, `0x30`, and `0x31`

NeoTrellis address order is left-to-right, top-to-bottom when viewing the play
surface:

| Position | Jumpers | Address |
|---|---|---:|
| upper left | none | `0x2E` |
| upper right | A0 | `0x2F` |
| lower left | A1 | `0x30` |
| lower right | A0 + A1 | `0x31` |

The NeoKey has A0, A1, A2, and A3 soldered. It has no A4 jumper, so its address
is `0x3F`.

### SPI bus 0

- `GPIO10` / physical pin 19: OLED MOSI
- `GPIO11` / physical pin 23: OLED SCLK
- `GPIO8` / physical pin 24: OLED CS
- `GPIO23` / physical pin 16: OLED D/C
- `GPIO16` / physical pin 36: OLED reset
- `GPIO9` / physical pin 21: OLED MISO / SD path
- `GPIO7` / physical pin 26: OLED microSD chip select

### I2S

- `GPIO18` / physical pin 12: DAC BCK
- `GPIO19` / physical pin 35: DAC LRCK / WSEL
- `GPIO21` / physical pin 40: DAC DIN

### Encoder wiring

| Ref | Role | A | B | Switch |
|---|---|---:|---:|---:|
| `SW1` | main | GPIO5 / pin 29 | GPIO6 / pin 31 | GPIO12 / pin 32 |
| `SW2` | aux1 | GPIO13 / pin 33 | GPIO25 / pin 22 | GPIO17 / pin 11 |
| `SW3` | aux2 | GPIO27 / pin 13 | GPIO4 / pin 7 | GPIO14 / pin 8 |
| `SW4` | aux3 | GPIO26 / pin 37 | GPIO24 / pin 18 | GPIO22 / pin 15 |

`SW3` uses `GPIO14` / physical pin 8, which is also Raspberry UART TX. The
image leaves the UART inactive (`enable_uart=0` and no serial-console kernel
token) so GPIO14 remains an encoder input. `GPIO20` is reserved for OLED
microSD card detect; keep it free from I2S overlays and encoder inputs.

### Other connections

The PCB uses a 1x5 right-angle male header at `J1`. A 5-wire female-to-female
Dupont cable connects it to the 5-pin header on the upper-left NeoTrellis board.

| J1 pin | Signal | Raspberry connection |
|---:|---|---|
| 1 | INT | `GPIO15` / physical pin 10 |
| 2 | VIN | `+5V` |
| 3 | GND | `GND` |
| 4 | SCL | `GPIO3` / physical pin 5 |
| 5 | SDA | `GPIO2` / physical pin 3 |

The NeoKey shares I2C bus 1 power and data with the NeoTrellis. Its `INT` is on
the same interrupt net as the NeoTrellis connector. The DAC is powered from
`+5V` and connects to the Pi I2S lines above.

## Orange Pi Zero 2W

Orange uses its H618 GPIO names, pin mappings, and device paths. Use the
[Orange Armbian bring-up notes](orange-pi-armbian-bringup.md) for the Orange
encoder connections and board-specific SPI, I2C, audio, and interrupt routing.
The Raspberry GPIO table above does not apply to Orange.

## PCB files

- Schematic: [`../pcb/octessera.kicad_sch`](../pcb/octessera.kicad_sch)
- Layout: [`../pcb/octessera.kicad_pcb`](../pcb/octessera.kicad_pcb)
