# Open Work

This file tracks current actionable work only. Completed-work history does not belong here.

## Hardware Validation

- Run the first physical Pi smoke test with the native Rust Pi app.
- OLED checklist: validate SSD1351 orientation, clipping, brightness, text layout, startup/help toast wording, help dialogs, confirm dialogs, and long sample-browser rows on the physical display.
- NeoTrellis checklist: validate coordinate orientation, lower-left grid semantics, Dance Fn columns, overlay priority, XY marker position, sample/probability assignment colors, and full-frame stability on hardware.
- NeoKey checklist: validate Back, Space, Shift, Fn, combined Shift+Fn, modifier-held hints, button LED colors, and help chord entry.
- Encoders checklist: validate main encoder turn/press, all aux encoder directions, aux push switches, Fn+Aux binding, turn/press overlay indicators, and no-binding/not-active toasts.
- Audio-adjacent UX checklist: validate audio-device startup status, sample preview feedback, sampler assignment feedback, Dance FX assignment feedback, MIDI panic/status, and user-visible errors without requiring full audio quality sign-off.
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
- Continue splitting oversized native runtime files when working in those areas instead of expanding them further.

## Quality Targets

- Keep touched source files comfortably below the 500-line limit; avoid hovering at 499-500 LOC.
- Split high-complexity native runtime functions when making adjacent changes, especially config load/apply and input dispatch paths.
- Keep `resources/platform-capabilities.json`, generated TypeScript exports, and generated Rust constants in sync through `pnpm run capabilities:check`.
- Keep `resources/menu-help-texts.tsv` aligned with native menu coverage tests.
