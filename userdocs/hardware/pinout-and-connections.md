# Pinout and Connections

This page is profile-aware. The detailed assignments below are the Raspberry Pi
Zero 2 W profile; the Orange Pi Zero 2W path has its own physical pin and device
checks and must not borrow Raspberry numbers.

Use it while assembling the device. It is deliberately plain: this is where the electricity gets fewer jokes and more exact pins. For the user-facing docs home, see [`../README.md`](../README.md). For build order and board setup, see [`assembly-manual.md`](assembly-manual.md). For the case and port layout, see [`enclosure.md`](enclosure.md).

## Board profile first

Before wiring, choose the exact profile in [`../../docs/board-profiles.md`](../../docs/board-profiles.md).
For Orange, use the [Orange Armbian bring-up notes](../../hardware/docs/orange-pi-armbian-bringup.md)
and the board's [official product page](http://www.orangepi.org/html/hardWare/computerAndMicrocontrollers/details/Orange-Pi-Zero-2W.html)
or [H618 user manual](https://orangepi.net/wp-content/uploads/2023/10/OrangePi_Zero2w_H618_User-Manual_v1.1.pdf).
Do not translate this Raspberry table by physical position.

## Raspberry Pi Zero 2 W

The following power, bus, encoder, and connector assignments are for the
Raspberry build only.

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
- If you use the Pi USB data/gadget port with a host computer, use a data-only cable or another hardware method that blocks the cable's 5V line. Software cannot make a Pi Zero accept USB data while electrically refusing USB power.
- The USB-C breakout feeds the shared `+5V` rail for the Pi, OLED, NeoKey, NeoTrellis connector, and DAC.
- `C1` is a `470uf` polarized capacitor across `+5V` and `GND` to add bulk supply smoothing on the main power rail.

## USB Audio and MIDI

The Raspberry image exposes USB audio and MIDI through the Pi data port. Audio-only, MIDI-only, and combined modes keep the existing product names: `Octessera Line In`, `Octessera MIDI`, and `Octessera Audio + MIDI`.

MIDI and combined modes require the image's ConfigFS MIDI function to provide an `interface_string` attribute. The gadget setup writes exactly `Octessera MIDI` (14 bytes, with no trailing newline), checks the size and byte-for-byte readback, and binds the USB device only after those checks pass. If the attribute is missing or cannot be written, read back, or bound, setup fails closed, attempts cleanup, and reports cleanup failure instead of presenting the kernel's generic `MIDI function` label.

Use a data-only cable or a power-isolating adapter when connecting the Pi data port to a host while the instrument is powered through the enclosure USB-C input.

## Bus Allocation

### I2C bus 1

- `GPIO2` / physical pin 3: SDA
- `GPIO3` / physical pin 5: SCL
- Devices on this bus:
  - NeoKey 1x4 at `0x3F`
  - NeoTrellis chain via `J1` at `0x2E`, `0x2F`, `0x30`, and `0x31`

NeoTrellis address order is left-to-right, top-to-bottom when viewing the play surface:

| Position | Jumpers | Address |
|---|---|---:|
| upper left | none | `0x2E` |
| upper right | A0 | `0x2F` |
| lower left | A1 | `0x30` |
| lower right | A0 + A1 | `0x31` |

The NeoKey has A0, A1, A2, and A3 soldered. It has no A4 jumper, so its address is `0x3F`.

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

- `SW3` switch uses `GPIO14` / physical pin 8, which is also Raspberry UART TX.
  The image therefore declares the UART inactive (`enable_uart=0`, with no
  serial-console kernel token) so GPIO14 remains an input for reliable encoder
  input. There is no post-boot UART release step.
- `GPIO20` is reserved for OLED microSD card detect; keep it free from I2S overlays and encoder inputs.

## Other Connections

### NeoTrellis connector `J1`

The PCB uses a 1x5 right-angle male header at `J1`. A 5-wire female-to-female Dupont cable connects it to the 5-pin male header on the upper-left NeoTrellis board.

- Pin 1: INT -> `GPIO15` / physical pin 10
- Pin 2: VIN -> `+5V`
- Pin 3: GND -> `GND`
- Pin 4: SCL -> `GPIO3` / physical pin 5
- Pin 5: SDA -> `GPIO2` / physical pin 3

### NeoKey

- Shares I2C bus 1 power and data with NeoTrellis
- Address: `0x3F` with A0, A1, A2, and A3 soldered
- INT is tied into the same interrupt net as the NeoTrellis connector in the current schematic

### DAC

- Powered from `+5V`
- Connected to Pi I2S for audio output

## Source of Truth

- Schematic: [`../../hardware/pcb/octessera.kicad_sch`](../../hardware/pcb/octessera.kicad_sch)
- Netlist: [`../../hardware/pcb/octessera.net`](../../hardware/pcb/octessera.net)
- HAL pin mapping: [`../../crates/hal/src/pinmap.rs`](../../crates/hal/src/pinmap.rs)

## Orange Pi Zero 2W

There is no second, guessed wiring table here. Use the exact Orange [board
profile](../../docs/board-profiles.md), [Armbian bring-up notes](../../hardware/docs/orange-pi-armbian-bringup.md),
and the [Orange encoder mapping](../../hardware/docs/orange-pi-armbian-bringup.md#direct-encoder-mapping).
That source records the H618 gpiochip offsets, reviewed SPI/I2C paths, and
candidate physical connections.

The following physical checks remain unresolved and must be done on the named
board and image before the shared PCB/harness is treated as qualified:

- I2S DAC pins and the live ALSA card path.
- USB-C port role, VBUS/CC behavior, UDC presence, and no-backfeed behavior.
- Live GPIO/pinmux and interrupt ownership for the control surface.
- The complete OLED, NeoTrellis, NeoKey, encoder, audio, MIDI/USB, and enclosure
  fit checks.

If any of those checks is unclear, stop. The Raspberry values above are not an
Orange fallback.
