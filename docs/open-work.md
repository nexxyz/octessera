# Open Work

This file tracks current actionable work only. Completed-work history does not belong here.

## Prioritized Technical Debt

This is a ranked backlog, not approval to begin the work. Reassess scope before
starting an item and keep each change independently shippable. Estimates are
rough engineer-days; the preferred evidence paths are hardware-free unless the
item explicitly calls for a final device smoke test.

1. **Add a fakeable Seesaw/I2C transport (4–5 days).** Inject one narrow
   address/write/write-read/timing transport into NeoTrellis and NeoKey instead
   of opening device files inside each driver. Verify exact initialization and
   FIFO transactions, retries, short reads/writes, and ioctl failures on the
   host, followed by one hardware smoke pass when hardware is available.
2. **Consolidate platform-core tick and input post-processing (2–3 days).**
   Extract their shared interpretation, filtering, mapping, global-sound, and
   note-processing finalization while preserving their distinct marker and
   interpretation policies. Verify paired tick/input metadata, held notes, and
   existing engine parity tests.
3. **Bound and overload-harden desktop worker queues (3–4 days).** Replace the
   unbounded producer boundaries in `runtime_worker.rs`,
   `desktop_platform_service.rs`, and `audio_prep_service.rs` with nonblocking,
   typed admission policies. Preserve panic/stop operations and explicitly
   coalesce only safe realtime traffic. Verify saturation, recovery, ordering,
   and desktop build behavior.
4. **Introduce typed persisted NativeRunner configuration DTOs (5–6 days).**
   Type the stable outer payload, runtime, layer, instrument, mixer, and device
   structures while retaining validated extension JSON for behavior-specific
   and FX parameters. Preserve strict schema validation and migrations before
   decoding. Verify defaults, factory payloads, malformed fields, migrations,
   and round trips. Land before the transaction item so transactions build on
   typed aggregates.
5. **Make NativeRunner configuration transactions explicit (5–6 days).**
   Extract the transaction-owned configuration aggregate and have config/menu
   changes produce one audio update plan: none, dynamic commands, or a full
   revisioned configuration. Avoid a broad runner rewrite. Verify live-state
   preservation and exactly one audio revision per committed transaction.

## Legal and attribution follow-up

- Before any future public board-image release, review the applicable source
  duties for its pinned upstream inputs and the Octessera source, patches,
  configuration, and build scripts; see [`release-licensing.md`](release-licensing.md).

## Phase 5 Boot OLED Qualification

- Run the full Raspberry and Orange constructors from the source-bound boot-layer contracts: `resources/image-construction/boot-layers/raspberry-pi-zero-2w.json` and `resources/image-construction/boot-layers/orange-pi-zero-2w.json`. Regenerate the selected initramfs outputs and Orange Python closure; record source hashes and mounted-image proof. A trusted `v0.7.5` runtime/setup parent respin is not proof of this layer.
- On each new image, prove continuous initramfs-to-userspace cycles: one bounded initramfs cycle is fully reaped, the early userspace loop continues, native startup releases and adopts without resetting the OLED, and animation stops before the acknowledged first normal menu frame. Prove there is no blank, static, flickering, or dual-writer interval.
- Exercise restart and failure paths on both boards: animator restart, native startup failure, OLED write failure, stale/mismatched status, lock contention/timeout, and recovery without orphaned processes, writers, lock state, or temporary handoff files. Orange also proves DAC-health gating before readiness.
- Physically qualify the mounted constructor images on both assembled boards: first-menu handoff and OLED readability, then separate sleep, resume, and shutdown/reboot behavior. Confirm those lifecycle paths do not race the boot writer and do not borrow the boot animation contract.

## Hardware Validation

- Setup portal qualification remains open on both boards: verify AP creation and joining, the captive page, successful Wi-Fi credential/hostname/SSH/login application, reconnect on the new network, attachment to an already-running setup service, the 30-minute timeout, failure and partial-state messaging, and absence of secrets in the AP, HTTP responses, status/receipt files, logs, and artifacts.
- Replace or independently verify the SSD1351 OLED module; the tested module stayed blank with Pi and Arduino Adafruit test code despite valid power and command wiring.
- OLED checklist after replacement: validate orientation, clipping, brightness, text layout, startup/help toast wording, help dialogs, confirm dialogs, and long sample-browser rows on the physical display.
- NeoTrellis checklist: validate coordinate orientation, lower-left grid semantics, Play Fn columns, overlay priority, XY marker position, sample/probability assignment colors, and full-frame stability on hardware after the corrected connector path is installed.
- NeoKey checklist: validate Back, Space, Shift, Fn, combined Shift+Fn, modifier-held hints, button LED colors, and help chord entry on the PCB.
- Encoders checklist: validate main encoder turn/press, all aux encoder directions, aux push switches, Fn+Aux binding, turn/press overlay indicators, and no-binding/not-active toasts.
- Audio-adjacent UX checklist: validate audio-device startup status, sample preview feedback, sampler assignment feedback, Play FX assignment feedback, MIDI panic/status, and user-visible errors without requiring full audio quality sign-off.
- Validate runtime audio through the target DAC beyond the successful ALSA 440 Hz test tone.
- Validate sample preview, loaded sample banks, and runtime audio config sync through the Pi host adapter.

## Hardware Follow-Ups

- Hardware test harness: planned after the first successful Pi run. It should verify I2C devices, OLED, NeoKey, NeoTrellis, encoders, DAC output, and basic runtime input/output routing.
