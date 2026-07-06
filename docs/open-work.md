# Open Work

This file tracks current actionable work only. Completed-work history does not belong here.

## Hardware Validation

- Replace or independently verify the SSD1351 OLED module; the tested module stayed blank with Pi and Arduino Adafruit test code despite valid power and command wiring.
- OLED checklist after replacement: validate orientation, clipping, brightness, text layout, startup/help toast wording, help dialogs, confirm dialogs, and long sample-browser rows on the physical display.
- NeoTrellis checklist: validate coordinate orientation, lower-left grid semantics, Dance Fn columns, overlay priority, XY marker position, sample/probability assignment colors, and full-frame stability on hardware after the corrected connector path is installed.
- NeoKey checklist: validate Back, Space, Shift, Fn, combined Shift+Fn, modifier-held hints, button LED colors, and help chord entry on the PCB.
- Encoders checklist: validate main encoder turn/press, all aux encoder directions, aux push switches, Fn+Aux binding, turn/press overlay indicators, and no-binding/not-active toasts.
- Audio-adjacent UX checklist: validate audio-device startup status, sample preview feedback, sampler assignment feedback, Dance FX assignment feedback, MIDI panic/status, and user-visible errors without requiring full audio quality sign-off.
- Validate runtime audio through the target DAC beyond the successful ALSA 440 Hz test tone.
- Validate sample preview, loaded sample banks, and runtime audio config sync through the Pi host adapter.

## Pi Build And Deployment

- Bake the current post-flash fixes into a new Pi image/release artifact; the live Pi currently has fixes deployed over `v0.5.1`.
- Re-enable `cellsymphony.service` after I2C hardware is stable; it was intentionally disabled during bring-up to prevent automatic bus access.
- Keep deployment tooling aligned with the verified cross-build path and current systemd service layout.
- Audit deploy-script quoting scope and narrow it where needed without changing the verified fast deploy path.

## Hardware Follow-Ups

- Hardware test harness: planned after the first successful Pi run. It should verify I2C devices, OLED, NeoKey, NeoTrellis, encoders, DAC output, and basic runtime input/output routing.
- OTA/update flow: planned after the first successful Pi run. It should support safe update and rollback for deployed hardware.

## Product Follow-Ups

- Signal path visualization is on hold due to other priorities.
- Potential product rename to Octessera is captured in `docs/octessera-rename-plan.md`; do not execute until explicitly approved.
- Continue splitting oversized native runtime files when working in those areas instead of expanding them further.

## Quality Targets

- Keep touched source files comfortably below the 500-line limit; avoid hovering at 499-500 LOC.
- Split high-complexity native runtime functions when making adjacent changes, especially config load/apply and input dispatch paths.
- Keep `resources/platform-capabilities.json`, generated TypeScript exports, and generated Rust constants in sync through `pnpm run capabilities:check`.
- Keep `resources/menu-help-texts.tsv` aligned with native menu coverage tests.
