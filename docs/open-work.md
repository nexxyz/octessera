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

Desktop queue admission policy is not product work. The desktop is a
non-authoritative simulator and harness; native runtime queues and their policy
remain the authoritative implementation boundary.

## Manual FAT — current open checks

Use the [two-board FAT orchestrator](../userdocs/hardware/fat-quick-run.md) first.
It intentionally covers boot, native handoff, OLED startup, service readiness,
setup, basic controls, selected ordinary audio, and a known runtime sound once
per board. Do not repeat those checks while closing the gaps below.

### Boot, OLED, and lifecycle

- The v0.8.1 Raspberry and Orange constructors/source-bound evidence now exists. Retain the exact draft's image, source hashes, selected boot outputs, and Raspberry mounted-image/kernel proof; physical exact-artifact FAT remains open. A trusted-parent respin is not proof of this layer.
- Keep `tools/image-respin`, its trusted proof scripts, workflows, and recovery machinery as frozen legacy recovery until FAT closes. Do not use that lane as v0.8.1 qualification or delete it in this pre-FAT step. Raspberry full-image update remains manual.
- Normal first boot does not wait six minutes: upstream filesystem resize is ordered before runtime, and live Orange evidence saw resize complete from about 19.4 s to 22.7 s with runtime starting at 24.3 s. Six minutes is only the upstream maximum timeout, not a planned user-visible duration. On each new image, prove the root systemd animator starts concurrently, native startup releases and adopts without resetting the OLED, and animation or the handoff frame stops before the acknowledged first normal menu frame. If the handoff window expires first, prove that both board source paths show one static polished `STARTUP DELAYED` / `PLEASE WAIT` frame and continue single-writer handoff polling. This delayed-start mitigation does not claim resize state. Also prove the permitted initial blank interval, lack of static/flickering/dual-writer behavior after service start, and clean handoff.
- Exercise restart and failure paths on both boards: animator restart, native startup failure, OLED write failure, stale/mismatched status, lock contention/timeout, and recovery without orphaned processes, writers, lock state, or temporary handoff files. Orange also proves Jack-route readiness gating and selected optional USB/HDMI wait/recovery behavior.
- Orange runtime recovery is fail-closed: systemd permits the initial start plus two retries in 30 seconds, then requires `sudo systemctl reset-failed octessera.service` followed by `sudo systemctl start octessera.service`. The OLED animator handoff window is 30 seconds monotonic; its timeout writes the static `STARTUP DELAYED` / `PLEASE WAIT` frame once and continues polling rather than failing or blanking the display. Only genuine animator errors or termination signals attempt black then display-off independently and leave a native-recoverable failed handoff. Exact Trellis wiring and addresses remain required; no alternate fallback is supported.
- Both selected initramfs images are source-tested for their static frame and closure; both boards' systemd boot animations are source-tested for readiness and handoff. Constructor-image qualification remains open; the completed bounded live cold-boot result is recorded in the [shared board-profile qualification result](board-profiles.md#bounded-boot-result-historical). Linux system suspend/resume and shutdown remain separate physical qualification gates; confirm they do not race the boot writer or borrow the boot animation contract.
- Physical Orange/Raspberry HDMI and OLED qualification remains deferred. Qualify `/dev/tty1` Terminal ownership, the native grid VT lease, no connector force or display server, missing-`/dev/fb0` retry/nonfatal behavior, splash observation through `first_menu_rendered`, and fatal OLED reclaim. The source and contract checks do not claim hardware proof.

### Setup and data continuity

- Setup portal qualification remains open on both boards: from the instrument choose `System > Configure WiFi > Open Portal`, verify AP creation and joining, the captive page, successful Wi-Fi credential/hostname/SSH/login application, reconnect on the new network, the 10-minute user window after readiness, terminal failure/timeout messaging, and absence of secrets in AP traffic, HTTP responses, the single current status file, logs, and artifacts. Browser submission is provisional; the OLED terminal result is authoritative. Success requires only a usable global `wlan0` IPv4 address, not Internet access, a default route, DNS, or ICMP; then direct the user to `System > Info` without a reboot. A new menu action is the only retry. Do not expect an automatic hotspot on a fresh production image. Retained legacy first-boot behavior remains scoped by the Orange first-boot page.
- Qualify standalone `System > Backup & Restore` on both Pi boards: usable regular
  `wlan0` address selection, dynamic URL/code, reopen without lifetime extension,
  Back/Stop behavior, 15-minute expiry, authentication revocation, and physical
  restore confirmation/input blocking. No regular IP must return unavailable
  without binding or retry. Desktop is unsupported. This is separate from
  `Configure WiFi` and rolling `System > Saves > Default > Backups`.

### Physical controls and displays

- The current Raspberry and Orange SSD1351 modules are operational for the bounded cold-boot static-logo, animation, and menu-handoff qualification above. Broader OLED interaction, brightness, sleep/resume, and lifecycle checks remain open.
- Orange OLED orientation, edge alignment, boot handoff, default idle sleep, and first-input-consumed wake are qualified. Raspberry still needs the full checklist. Both boards still need physical brightness, startup/help toast wording, help dialogs, confirm dialogs, and long sample-browser row checks.
- NeoTrellis checklist: validate coordinate orientation, lower-left grid semantics, Play Fn columns, overlay priority, XY marker position, sample/probability assignment colors, and full-frame stability on hardware after the corrected connector path is installed. Standard NeoTrellis/NeoKey operation and fail-hard absence behavior are accepted engineering contracts; physical input, coordinate, and LED checks belong here.
- NeoKey checklist: validate Back, Space, Shift, Fn, combined Shift+Fn, modifier-held hints, button LED colors, and help chord entry on the PCB.
- Encoders checklist: validate main encoder turn/press, all aux encoder directions, aux push switches, Fn+Aux binding, turn/press overlay indicators, and no-binding/not-active toasts.

### Audio and USB gaps

- Audio-adjacent UX checklist: validate audio-device startup status, sample preview feedback, sampler assignment feedback, Play FX assignment feedback, MIDI panic/status, and user-visible errors without requiring full audio quality sign-off.
- Validate runtime audio through the selected exact routes beyond the successful ALSA 440 Hz test tone, including independent unsynchronized USB/HDMI clocks, possible drift/echo, and recovery after endpoint loss; this phase does not provide sample alignment. Every non-empty output set is valid. Jack is fatal/required only when selected, recognized disconnected USB/HDMI may wait, selected faults block readiness, and no route is a fallback.
- Validate sample preview, loaded sample banks, and runtime audio config sync through the Pi host adapter.
- USB Audio/MIDI remains a dedicated gap test: authorized identity, port role, VBUS/CC and no-backfeed safety, host enumeration, intended UAC2 audio, intended MIDI, reconnect, and absence of mass storage. Do not count ordinary DAC/Jack audio from the quick run as USB evidence.
