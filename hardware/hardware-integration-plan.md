# Pi Zero Integration

This document describes the current Raspberry Pi Zero 2 W hardware target and native app integration status.

## Hardware Target

- Compute: Raspberry Pi Zero 2 W
- Grid: four NeoTrellis 4x4 boards chained as an 8x8 matrix over I2C bus 1
- Buttons: NeoKey 1x4 over I2C bus 1
- Display: SSD1351 128x128 RGB OLED over SPI
- Audio: PCM5102-class DAC through the ADA6250 connector over I2S
- Controls: four clickable direct-GPIO rotary encoders

## Software Path

- App: `apps/pi-zero`
- HAL crate: `crates/hal`
- Runtime: `crates/playback-runtime::NativeRunner`
- Core behavior logic: `crates/platform-core`
- Audio engine: `crates/realtime-engine` through `crates/rodio-engine-source`

The Pi app is native Rust. It does not start a Node or TypeScript runtime.

## Implemented Host Integration

- Pi app instantiates `NativeRunner` directly.
- Host builds use HAL stubs by default.
- Real hardware builds use the `hardware-pi` feature.
- NeoTrellis input maps to native grid press/release messages.
- NeoTrellis LED output consumes runtime LED snapshots.
- NeoKey keys map to `button_a`, `button_s`, `button_shift`, and `button_fn`.
- NeoKey LEDs consume runtime modifier/transport/display state.
- Encoders map to `main`, `aux1`, `aux2`, `aux3`, and `aux4`.
- Encoder GPIO uses stateful quadrature decoding.
- Host adapter handles default/preset storage effects, MIDI list/select/panic/output, sample listing, musical events, Dance FX audio commands, and playback/MIDI status updates.

## Hardware Validation Still Required

- Physical Pi app boot and runtime smoke test.
- OLED orientation, color order, clipping, and brightness validation.
- NeoTrellis coordinate orientation and LED priority validation.
- NeoKey physical key order and LED color validation.
- Encoder direction and push debounce validation.
- I2S DAC/ALSA output validation.
- Sample preview and sample-bank sync validation on the Pi audio path.

Tracked current work lives in `docs/open-work.md`.

## Build Commands

Host-stub build:

```bash
cargo build -p cellsymphony-pi
```

Real hardware build on Pi or configured cross environment:

```bash
cargo build -p cellsymphony-pi --features hardware-pi
```

On non-Pi cross hosts, the hardware build requires an ARM Linux sysroot and cross `pkg-config` setup for ALSA.

## Bus And Pin Resource Notes

- SPI: OLED write path on MOSI/SCLK/CE0 plus DC/RST GPIO.
- I2C: NeoTrellis and NeoKey seesaw devices on bus 1.
- I2S: PCM5102-class DAC using BCK/LRCK/DIN.
- GPIO9 is reused for SW5 channel B because the OLED path is write-only and SPI MISO is unused.
- GPIO14 can be claimed by UART TX if serial console is enabled; disable serial console for encoder reliability.

## Bring-Up Checklist

1. Verify +5V and +3.3V rails under load.
2. Detect all NeoTrellis and NeoKey I2C devices.
3. Initialize OLED and render a runtime snapshot frame.
4. Initialize DAC and produce audio output through the realtime engine path.
5. Verify each encoder direction and push switch.
6. Verify all grid coordinates and LED colors.
7. Verify transport timing and MIDI clock behavior.
8. Verify preset/default storage and sample browser paths.

## Mechanical Notes

- KiCAD source remains in `hardware/KiCAD`.
- Frontplate dimensions are documented in `hardware/enclosure-frontplate-revA-dimensions.md`.
