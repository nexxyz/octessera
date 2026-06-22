# Pi Zero Integration

This document describes the current Raspberry Pi Zero 2 W hardware target and native app integration status.

## Hardware Target

- Compute: Raspberry Pi Zero 2 W
- Grid: four NeoTrellis 4x4 boards chained as an 8x8 matrix over I2C bus 1
- Buttons: NeoKey 1x4 over I2C bus 1
- Display: SSD1351 128x128 RGB OLED over SPI
- Audio: PCM5102-class DAC through the ADA6250 connector over I2S
- Controls: four clickable direct-GPIO rotary encoders: one main and three aux controls
- Power input: USB-C breakout to the shared +5V rail
- Bulk power smoothing: `C1` `470uf` polarized capacitor across `+5V` and `GND`

Reference docs:

- `hardware/pinout-and-connections.md`
- `hardware/enclosure/README.md`

## Software Path

- App: `apps/pi-zero`
- HAL crate: `crates/hal`
- Runtime: `crates/playback-runtime::NativeRunner`
- Core behavior logic: `crates/platform-core`
- Audio engine: `crates/realtime-engine` through `crates/rodio-engine-source`

The Pi app is native Rust. It does not start a Node or TypeScript runtime.

The device is a dedicated appliance: the Pi app must launch automatically on boot through `cellsymphony.service`. Interactive SSH use is for setup, diagnostics, logs, and updates only.

## Implemented Host Integration

- Pi app instantiates `NativeRunner` directly.
- Host builds use HAL stubs by default.
- Real hardware builds use the `hardware-pi` feature.
- NeoTrellis input maps to native grid press/release messages.
- NeoTrellis LED output consumes runtime LED snapshots.
- NeoKey keys map to `button_a`, `button_s`, `button_shift`, and `button_fn`.
- NeoKey LEDs consume runtime modifier/transport/display state.
- Encoders map to `main`, `aux1`, `aux2`, and `aux3`.
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

## Pi OS Image Assumptions

The Rust app assumes the appliance image has already configured the Pi pin muxes. Use Raspberry Pi OS Lite and set `/boot/firmware/config.txt` to keep GPIO ownership aligned with the netlist:

```text
dtparam=audio=off

# I2S DAC: GPIO18 BCK, GPIO19 LRCK, GPIO21 DIN.
# Custom playback-only overlay keeps GPIO20 free for card detect.
dtoverlay=i2s-dac-no20

# OLED and OLED-module SD path.
dtparam=spi=on

# NeoTrellis and NeoKey.
dtparam=i2c_arm=on

# GPIO14 and GPIO15 are application GPIO, not UART.
enable_uart=0
```

On startup or during bring-up, verify the I2S/card-detect split:

```bash
pinctrl get 18 19 20 21
```

Expected ownership:

- `GPIO18`, `GPIO19`, and `GPIO21`: PCM/I2S alternate function.
- `GPIO20`: GPIO input for OLED microSD card detect.

## What Can Be Tested Before The PCB Exists

On this PC:

- Build the host-stub Pi app: `cargo build -p cellsymphony-pi`.
- Run native runtime/core tests that feed the Pi app: `cargo test -p platform-core -p playback-runtime`.
- Run Pi render tests: `cargo test -p cellsymphony-pi render`.
- Keep HAL pin constants, hardware docs, and the KiCAD netlist aligned by review.

On a Pi with Raspberry Pi OS Lite, over SSH, before the PCB/components exist:

- Apply the boot config above and reboot.
- Confirm interfaces are present: `ls /dev/i2c-1 /dev/spidev0.0`.
- Confirm pin muxes with `pinctrl get 2 3 7 8 9 10 11 14 15 18 19 20 21 23 16`.
- Confirm ALSA sees the DAC overlay after reboot: `aplay -l`.
- Build the hardware binary on-device if dependencies are installed: `cargo build --release -p cellsymphony-pi --features hardware-pi`.
- Run the app only as a negative smoke test until hardware is attached; missing I2C/OLED devices should fail clearly rather than silently falling back.

From this Windows development PC, run the SSH preflight check with:

```powershell
./tools/pi-preflight.ps1 -Target pi@192.168.0.211
```

## Fast Deploy For Bring-Up

Use two paths:

1. Bootstrap/image setup: keep `tools/deploy-pi.sh` or the image stage scripts responsible for OS packages, boot config, and enabling `cellsymphony.service` for automatic startup.
2. Fast iteration: add a small SSH deploy script that builds or copies a release binary to the Pi, installs it as `/opt/cellsymphony/cellsymphony-pi`, then runs `sudo systemctl restart cellsymphony` and tails `journalctl -u cellsymphony -f`.

The fast path should not edit boot config on every run. Treat boot config as image/bootstrap state.

Development fast deploy from Windows:

```powershell
./tools/deploy-pi-fast.ps1 -Target pi@192.168.0.211 -NoTail
```

By default the script syncs the current working tree to `/home/pi/cellsymphony-dev`, builds on the Pi with the real `hardware-pi` feature, installs the binary under `/opt/cellsymphony/releases/dev`, restarts `cellsymphony.service`, and optionally tails logs. Use `-LocalBinary <path>` only when you already have a Linux ARM binary built elsewhere.

Pi Zero 2 W builds are memory constrained. The script forces `CARGO_BUILD_JOBS=1`, but the first release build can still be slow. Use `-SyncOnly` to validate SSH/source sync without building, or use `-LocalBinary` once a cross-built Linux ARM binary is available.

Before the PCB and components are attached, use `-AllowServiceFailure` so a missing OLED/I2C device can fail clearly without failing the deploy script itself:

```powershell
./tools/deploy-pi-fast.ps1 -Target pi@192.168.0.211 -NoTail -AllowServiceFailure
```

## OTA Update Plan

Use a simple A/B binary swap rather than a package manager at first:

1. Upload a signed or checksummed release bundle containing `cellsymphony-pi`, version metadata, and optional resources.
2. Write it to `/opt/cellsymphony/releases/<version>/`.
3. Verify checksum and executable bit.
4. Update `/opt/cellsymphony/current` symlink atomically.
5. Restart `cellsymphony.service`.
6. Keep the previous symlink target for rollback.

Menu integration should be a native System action that emits a Pi-only platform effect, for example `system.updateCheck`, `system.updateApply`, and `system.rollback`. The Pi host adapter owns network/download/systemd behavior. The native runtime only owns menu state, confirmation, progress/status text, and platform-effect requests.

## Hardware Test Harness Plan

Add a System menu action such as `System > Diagnostics > Hardware Test`. It should enter a guided hardware-only diagnostic mode handled by the Pi host adapter and HAL, without running detailed musical behavior logic.

Suggested flow:

1. OLED: draw color bars, text rows, border, and brightness steps; ask user to confirm visibility.
2. NeoKey: ask user to press each key; light each key when detected.
3. Encoders: ask user to turn left/right and press each encoder; show detected direction/count on OLED.
4. NeoTrellis: ask user to press corners, then sweep all LEDs by color and row/column.
5. Audio: play a short DAC test tone through the realtime-engine/audio path.
6. SD card detect: show GPIO20 state with and without a card in the OLED module slot.
7. Summary: show pass/fail/manual-confirm results and write a diagnostic log.

Implementation boundary: `playback-runtime` should expose the menu action and platform effect; `apps/pi-zero` and `crates/hal` should own hardware probes. Desktop may display a simulated status only if needed, but must not duplicate hardware behavior.

## Bus And Pin Resource Notes

- SPI: OLED write path on MOSI/SCLK/CE0 plus DC/RST GPIO.
- I2C: NeoTrellis and NeoKey seesaw devices on bus 1.
- I2S: PCM5102-class DAC using GPIO18 BCK, GPIO19 LRCK, and GPIO21 DIN.
- GPIO9/SPI MISO is available for the OLED module SD/MISO path rather than for encoder input in the current schematic.
- GPIO14 can be claimed by UART TX if serial console is enabled; disable serial console for encoder reliability.
- GPIO20 is OLED microSD card detect and must stay GPIO input, not I2S.

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
- The enclosure intentionally exposes the USB-C breakout power input and covers the Pi micro-USB power connector.
- Power the device through the enclosure USB-C opening only; do not power the Pi through its own micro-USB port.
