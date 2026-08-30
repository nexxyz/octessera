# Open Work

This file tracks current physical FAT gaps only. Start with the [two-board FAT
quick run](../userdocs/hardware/fat-quick-run.md).

## Boot, display, and lifecycle

- Verify the exact release image, board identity, assembled hardware, power path,
  and evidence record for each board.
- Qualify boot, OLED animation, native handoff, first normal menu rendering, and
  recovery without blanking, flicker, dual writers, orphaned processes, or stale
  handoff state.
- Exercise animator restart, native startup failure, OLED write failure, stale
  status, lock contention, suspend/resume, shutdown, and cold recovery on both
  boards. Include Orange route-readiness and selected USB/HDMI wait and recovery
  behavior.
- Qualify physical HDMI and OLED behavior, including terminal ownership, native
  grid output, framebuffer retry, fatal OLED reclaim, brightness, sleep/resume,
  and long-running lifecycle behavior.

## Setup and data continuity

- Qualify `System > Configure WiFi > Open Portal` on both boards, including AP
  join, captive-page submission, credential and hostname application, reconnect,
  timeout/failure reporting, status hygiene, and the user-window behavior.
- Qualify standalone `System > Backup / Restore` on both boards, including URL
  and code lifetime, reopen, Back/Stop, expiry, authentication revocation,
  restore confirmation, and input blocking.

## Physical controls and displays

- Validate NeoTrellis orientation, lower-left grid semantics, Play Fn columns,
  overlay priority, XY markers, sample/probability colors, and frame stability.
- Validate NeoKey Back, Space, Shift, Fn, combined modifiers, hints, LED colors,
  and help-chord entry.
- Validate main and auxiliary encoder turn/press behavior, Fn bindings,
  overlays, and no-binding or inactive toasts.

## Audio and USB

- Evaluate PC-over-USB keyboard control for Orange Pi through the existing native
  device-input path. Preserve the simulator mappings for the main encoder and
  four NeoKeys. Use `E`/`R`/`T`, `F`/`G`/`H`, and `V`/`B`/`N` as candidate
  left/right/click mappings for auxiliary encoders 1, 2, and 3. Do not add a
  separate runtime behavior path.
- Validate audio startup status, sample preview and assignment feedback, Play FX
  assignment, MIDI panic/status, and user-visible audio errors.
- Qualify each selected Jack, USB, and HDMI route beyond source and bench checks,
  including independent-clock drift or echo and endpoint-loss recovery. Do not
  use one route as a fallback for another.
- Qualify sample preview, loaded sample banks, and runtime audio-configuration
  synchronization through the Pi host adapter.
- Qualify USB Audio/MIDI identity, port role, VBUS/CC and no-backfeed safety,
  enumeration, intended UAC2 audio, intended MIDI, reconnect, and absence of
  mass storage. Ordinary DAC or Jack audio is not USB evidence.

## Post-FAT action

- Keep the Orange current-parent respin lane nonpublishing and boot-neutral until
  exact constructor-image qualification and FAT close. Raspberry remains
  constructor-only for image replacement.
