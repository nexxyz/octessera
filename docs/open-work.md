# Open Work

This file tracks current actionable work only. Completed-work history does not belong here.

See [`internal/quality-improvement-plan.md`](internal/quality-improvement-plan.md) for the ranked quality backlog and its current implementation status.

## Prioritized Technical Debt

This is a ranked backlog, not approval to begin the work. Reassess scope before
starting an item and keep each change independently shippable. Estimates are
rough engineer-days. Orange automated on-device validation and attended physical
observation are currently available. Raspberry on-device evidence and the
remaining Orange qualification gates are deferred. Prefer PC evidence plus safe
Orange diagnostics where they establish a real additional fact.

1. **T1 remaining: qualify Seesaw in available evidence layers.** Now, use a
   hash-bound Orange diagnostic and fail-closed service restoration to verify
   `/dev/i2c-2`, fixed addresses, hardware IDs, bounded reads/writes, timeout
   behavior, shutdown, and sustained retained-descriptor health. Later, verify
   Orange inputs, coordinates, and LED appearance physically. When Raspberry is
   available, verify interrupt initialization/clear, all inputs and coordinates,
   GRB output, and sustained writes on `/dev/i2c-1`. Do not treat the Orange
   automated transaction result as visual/input qualification.
2. **Finish F2 desktop queue admission policy.** Platform requests are already
   bounded. Define explicit loss, retry, coalescing, emergency, and shutdown
   semantics before bounding runtime-worker commands, audio-prep control
   requests, audio trigger events, native MIDI events, audio failures, or result
   channels. Do not infer those policies from the downstream rodio queues.
3. **Complete nested persisted NativeRunner configuration DTOs (remaining
   scope).** Type the remaining runtime, layer, instrument, mixer, and device
   structures while retaining validated extension JSON for behavior-specific
   and FX parameters. The outer/application DTO slice is complete. Preserve
   strict schema validation and migrations before decoding; verify defaults,
   factory payloads, malformed fields, migrations, and round trips before the
   transaction item builds on typed aggregates.
4. **Make NativeRunner configuration transactions explicit (5–6 days).**
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
- Exercise restart and failure paths on both boards: animator restart, native startup failure, OLED write failure, stale/mismatched status, lock contention/timeout, and recovery without orphaned processes, writers, lock state, or temporary handoff files. Orange also proves Jack-route readiness gating and selected optional USB/HDMI wait/recovery behavior.
- Orange runtime recovery is fail-closed: systemd permits the initial start plus two retries in 30 seconds, then requires `sudo systemctl reset-failed octessera.service` followed by `sudo systemctl start octessera.service`. The OLED animator deadline is 30 seconds monotonic; timeout and cleanup failures attempt black then display-off independently and leave a native-recoverable failed handoff. Exact Trellis wiring and addresses remain required; no alternate fallback is supported.
- A regenerated selected initramfs is source-tested for an acknowledged, readable first menu; constructor-image and physical qualification remain open on both boards. Linux system suspend/resume and shutdown remain separate physical qualification gates; confirm they do not race the boot writer or borrow the boot animation contract.

## Hardware Validation

- Setup portal qualification remains open on both boards: verify AP creation and joining, the captive page, successful Wi-Fi credential/hostname/SSH/login application, reconnect on the new network, attachment to an already-running setup service, the 30-minute timeout, failure and partial-state messaging, and absence of secrets in the AP, HTTP responses, status/receipt files, logs, and artifacts.
- Replace or independently verify the Raspberry SSD1351 OLED module; the tested Raspberry module stayed blank with Pi and Arduino Adafruit test code despite valid power and command wiring. The Orange OLED is operational.
- Orange OLED orientation, edge alignment, boot handoff, default idle sleep, and first-input-consumed wake are qualified. Raspberry still needs the full checklist. Both boards still need physical brightness, startup/help toast wording, help dialogs, confirm dialogs, and long sample-browser row checks.
- NeoTrellis checklist: validate coordinate orientation, lower-left grid semantics, Play Fn columns, overlay priority, XY marker position, sample/probability assignment colors, and full-frame stability on hardware after the corrected connector path is installed.
- NeoKey checklist: validate Back, Space, Shift, Fn, combined Shift+Fn, modifier-held hints, button LED colors, and help chord entry on the PCB.
- Encoders checklist: validate main encoder turn/press, all aux encoder directions, aux push switches, Fn+Aux binding, turn/press overlay indicators, and no-binding/not-active toasts.
- Audio-adjacent UX checklist: validate audio-device startup status, sample preview feedback, sampler assignment feedback, Play FX assignment feedback, MIDI panic/status, and user-visible errors without requiring full audio quality sign-off.
- Validate runtime audio through the selected exact routes beyond the successful ALSA 440 Hz test tone, including independent unsynchronized USB/HDMI clocks, possible drift/echo, and recovery after endpoint loss; this phase does not provide sample alignment. Every non-empty output set is valid. Jack is fatal/required only when selected, recognized disconnected USB/HDMI may wait, selected faults block readiness, and no route is a fallback.
- Validate sample preview, loaded sample banks, and runtime audio config sync through the Pi host adapter.

## Hardware Follow-Ups

- Hardware test harness: planned after the first successful Pi run. It should verify I2C devices, OLED, NeoKey, NeoTrellis, encoders, DAC output, and basic runtime input/output routing.
