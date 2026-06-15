# Open Work

This file tracks current actionable work only. Completed-work history does not belong here.

## Hardware Validation

- Run the first physical Pi smoke test with the native Rust Pi app.
- Validate NeoTrellis coordinate orientation, LED color priority, and full-frame stability on hardware.
- Validate NeoKey button mapping and LED colors for Back, Space, Shift, Fn, and combined modifiers.
- Validate all five encoder directions and push switches with direct GPIO quadrature decoding.
- Validate SSD1351 OLED orientation, clipping, brightness, and text layout on the physical display.
- Validate PCM5102-class I2S output through the target DAC and ALSA device configuration.
- Validate sample preview, loaded sample banks, and runtime audio config sync through the Pi host adapter.

## Pi Build And Deployment

- Configure a real Pi or cross sysroot that can build `cellsymphony-pi --features hardware-pi` with ALSA dependencies.
- Document the working Pi image, required apt packages, ALSA device name, and service startup command after the first successful run.
- Keep `tools/deploy-pi.sh` aligned with the verified hardware build path.

## Hardware Follow-Ups

- Hardware test harness: planned after the first successful Pi run. It should verify I2C devices, OLED, NeoKey, NeoTrellis, encoders, DAC output, and basic runtime input/output routing.
- OTA/update flow: planned after the first successful Pi run. It should support safe update and rollback for deployed hardware.

## Product Follow-Ups

- Signal path visualization is on hold due to other priorities.
- Replace the Pi OLED placeholder renderer with the production snapshot renderer once hardware display orientation and spacing are verified.
- Continue splitting oversized native runtime files when working in those areas instead of expanding them further.

## Quality Targets

- Reduce `apps/desktop/src/ui/App.tsx` below the 500-line limit.
- Split high-complexity native runtime functions when making adjacent changes, especially config load/apply and input dispatch paths.
- Keep `resources/platform-capabilities.json`, generated TypeScript exports, and generated Rust constants in sync through `pnpm run capabilities:check`.
- Keep `resources/menu-help-texts.tsv` aligned with native menu coverage tests.
