# Raspberry Pi Bring-Up

This is the current end-user bring-up guide for the Raspberry Pi Zero 2 W hardware target.

Use it with:

- [`pinout-and-connections.md`](pinout-and-connections.md) for wiring and pin ownership.
- [`../enclosure/README.md`](../enclosure/README.md) for case, ports, and power rules.
- [`../../docs/menu-and-controls-spec.md`](../../docs/menu-and-controls-spec.md) for runtime controls and OLED/grid behavior.

## Hardware Target

- Compute: Raspberry Pi Zero 2 W
- Grid: four NeoTrellis 4x4 boards chained as an 8x8 matrix over I2C bus 1
- Buttons: NeoKey 1x4 over I2C bus 1
- Display: SSD1351 128x128 RGB OLED over SPI
- Audio: PCM5102-class DAC through the ADA6250 connector over I2S
- Controls: four clickable direct-GPIO rotary encoders: one main and three aux controls
- Power input: USB-C breakout to the shared `+5V` rail
- Bulk power smoothing: `C1` `470uf` polarized capacitor across `+5V` and `GND`
- I2C addresses: NeoTrellis `0x2E`, `0x2F`, `0x30`, `0x31`; NeoKey `0x3F`

## Power Rule

Power the device through the enclosure USB-C power input only.

Do not power the Raspberry Pi through its own micro-USB power port. The enclosure intentionally covers that connector.

## Software Path

- App: `apps/pi-zero`
- HAL crate: `crates/hal`
- Runtime: `crates/playback-runtime::NativeRunner`
- Core behavior logic: `crates/platform-core`
- Audio engine: `crates/realtime-engine` through `crates/rodio-engine-source`

The Pi app is native Rust. It does not start Node or TypeScript.

The device is intended to run as an appliance. The Pi app should launch on boot through `cellsymphony.service`. SSH is for setup, diagnostics, logs, and updates.

## Pi OS Boot Configuration

Use Raspberry Pi OS Lite. Configure `/boot/firmware/config.txt` so GPIO ownership matches the hardware:

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

After reboot, verify the I2S/card-detect split:

```bash
pinctrl get 18 19 20 21
```

Expected ownership:

- `GPIO18`, `GPIO19`, and `GPIO21`: PCM/I2S alternate function
- `GPIO20`: GPIO input for OLED microSD card detect

## Preflight Before Full Assembly

On the development PC:

```bash
cargo build -p cellsymphony-pi
cargo test -p platform-core -p playback-runtime
cargo test -p cellsymphony-pi render
```

On the Pi, before all hardware is attached:

```bash
ls /dev/i2c-1 /dev/spidev0.0
pinctrl get 2 3 7 8 9 10 11 14 15 18 19 20 21 23 16
aplay -l
```

From Windows, run the SSH preflight helper:

```powershell
./tools/pi/pi-preflight.ps1 -Target pi@192.168.0.211
```

Running the normal app before the PCB/components are attached is only a negative smoke test. Missing OLED or I2C devices should fail clearly rather than silently falling back.

## Build And Deploy

Host-stub build:

```bash
cargo build -p cellsymphony-pi
```

Preferred hardware iteration from Windows:

```powershell
./tools/pi/build-pi-cross.ps1
./tools/pi/deploy-pi-fast.ps1 -Target pi@192.168.0.211 -LocalBinary target/pi-cross/cellsymphony-pi -NoTail
```

Fallback on-Pi build:

```powershell
./tools/pi/deploy-pi-fast.ps1 -Target pi@192.168.0.211 -BuildOnPi -NoTail -AllowServiceFailure
```

`tools/pi/deploy-pi-fast.ps1` preserves the Pi `target/` cache by default. Use `-CleanRemote` only when intentionally discarding that cache.

## Release Image

Explicit GitHub releases include a ready-to-flash Pi Zero 2 W image named `Octessera-<version>-pi-zero-2w.img.zip`.

The image is derived from standard Raspberry Pi OS Bookworm arm64 through pi-gen and includes:

- the release `cellsymphony-pi` binary built with `--release --features hardware-pi`;
- `cellsymphony.service` and the performance governor service;
- runtime audio/I2C/SPI dependencies;
- Octessera boot config and the `i2s-dac-no20` overlay;
- empty `/home/pi/samples` and `/home/pi/presets` directories.

The release image must not include WiFi credentials, SSH keys, GitHub tokens, host logs, or local user secrets. SSH is disabled by default. Configure network access after first boot if you need SSH for setup.

## Verified Development Pi State

The current development Pi at `pi@192.168.0.211` has been verified with `tools/pi/pi-preflight.ps1`:

- Raspberry Pi OS Lite aarch64, kernel `6.18.34+rpt-rpi-v8`
- Persistent journald enabled through `/var/log/journal`
- `dtparam=i2c_arm=on`, `dtparam=spi=on`, `dtparam=audio=off`, `enable_uart=0`, and `dtoverlay=i2s-dac-no20` active
- `/dev/i2c-1` and `/dev/spidev0.0` exist and are accessible to user `pi`
- GPIO14 and GPIO15 are GPIO inputs, not UART
- GPIO18/19/21 are PCM/I2S and GPIO20 remains input for card detect
- ALSA exposes the PCM5102/HifiBerry-style DAC
- `rppal` `0.22.1` is required for this OS/kernel; older `0.14.1` failed model detection
- On-Pi fallback builds with `CARGO_BUILD_JOBS=1 cargo build --profile pi-dev -p cellsymphony-pi --features hardware-pi` succeed but take about 24 minutes on first build
- The app enters persistent hardware fault mode instead of restart-looping when critical hardware initialization fails
- PCM5102/HifiBerry-style DAC output has produced a 440 Hz ALSA test tone on the physical hardware
- NeoKey has been detected at `0x3F` on the PCB I2C path
- NeoTrellis has been detected at `0x2E`, `0x2F`, `0x30`, and `0x31` through a corrected connector path
- The tested SSD1351 OLED module stayed blank with both the Pi and an Arduino Uno running Adafruit SSD1351 test code; replace or independently verify the module before treating PCB or runtime display output as failed
- During active bring-up, `cellsymphony.service` may be disabled to prevent automatic I2C access while checking wiring; re-enable it only after the clean bus scan matches the expected devices

## Bring-Up Checklist

1. Verify `+5V` and `+3.3V` rails under load.
2. Detect NeoTrellis at `0x2E`, `0x2F`, `0x30`, and `0x31`, and NeoKey at `0x3F`.
3. Initialize OLED and render a runtime snapshot frame.
4. Initialize DAC and produce audio output through the realtime engine path.
5. Verify each encoder direction and push switch.
6. Verify all grid coordinates and LED colors.
7. Verify transport timing and MIDI clock behavior.
8. Verify preset/default storage and sample browser paths.

Current open validation work is tracked in [`../../docs/open-work.md`](../../docs/open-work.md).
The no-OLED manual walkthrough and CLI diagnostics are defined in [`manual-hardware-test-suite.md`](manual-hardware-test-suite.md).

Quick diagnostics:

```bash
sudo systemctl disable --now cellsymphony.service
/usr/local/bin/cellsymphony-pi --hardware-test
/usr/local/bin/cellsymphony-pi --hardware-noise-test --skip-trellis --skip-encoders
```

The diagnostics print a final warning/failure summary. Raw NeoKey one-sample glitches with clean immediate rereads are warnings; confirmed idle input remains a failure.

## Planned Hardware Diagnostics

A future `System > Diagnostics > Hardware Test` action should guide the user through:

1. OLED color bars, border, text, and brightness steps.
2. NeoKey press and LED checks.
3. Encoder left/right/push checks.
4. NeoTrellis corner press and LED sweep checks.
5. DAC test tone through the realtime audio path.
6. OLED microSD card-detect GPIO state.
7. Summary and diagnostic log.

Implementation boundary: `playback-runtime` owns the menu action and platform effect; `apps/pi-zero` and `crates/hal` own hardware probes.

## Update/Rollback Plan

Use a simple A/B binary swap first:

1. Upload a release bundle containing `cellsymphony-pi`, version metadata, and optional resources.
2. Write it to `/opt/cellsymphony/releases/<version>/`.
3. Verify checksum and executable bit.
4. Update `/opt/cellsymphony/current` atomically.
5. Restart `cellsymphony.service`.
6. Keep the previous symlink target for rollback.
