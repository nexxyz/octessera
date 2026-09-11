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

- Qualify `System > Setup > Configure WiFi > Open Portal` on both boards, including AP
  join, captive-page submission, credential and hostname application, reconnect,
  timeout/failure reporting, status hygiene, and the user-window behavior.
- Qualify standalone `System > Setup > Backup / Restore` on both boards, including URL
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

- Validate audio startup status, sample preview and assignment feedback, Play FX
  assignment, MIDI panic/status, and user-visible audio errors.
- Qualify each selected Jack and HDMI route beyond source and bench checks,
  including independent-clock drift or echo and endpoint-loss recovery. Do not
  use one route as a fallback for another. Ordinary DAC or Jack audio is not USB
  evidence.
- Qualify sample preview, loaded sample banks, and runtime audio-configuration
  synchronization through the Pi host adapter.
- Repeat the named USB identity and traffic checks on the exact release
  constructor image, including USB0/UDC, ConfigFS `interface_string`, the
  actual MIDI interface descriptor and exact Windows
  `DEVPKEY_Device_BusReportedDeviceDesc` value `Octessera MIDI`, the
  `Octessera Audio` UAC2 endpoint, the `Octessera Audio + MIDI` composite
  product, 44.1 kHz stereo tone capture, and bidirectional MIDI.
- Complete physical connector mapping (bench clue: the user called it `port 2`),
  VBUS/CC/no-backfeed electrical qualification, physical replug and host
  suspend/resume, SD2 mass-storage start/eject/stop recovery, and authorized
  public VID/PID qualification before claiming public USB support.

## Recording

- On Raspberry and Orange, qualify WAV and Audio+OLED AVI recording in both
  `System > Audio > Perf. Mode` choices, `Latency` and `Capacity`, under
  representative synth, sampler, and FX load. Treat Raspberry Capacity as
  production requalification evidence, not an already-qualified claim.
- Confirm the exact writable roots: Raspberry `/home/pi/recordings` and
  `/home/pi/screen-recordings`; Orange `/var/lib/octessera/recordings` and
  `/var/lib/octessera/screen-recordings`. Record the mounted storage and free
  space used for each run.
- Exercise explicit `Stop`, a short `Max Time` auto-stop, and the file-size
  ceiling where the Audio+OLED take reaches it. Confirm the final WAV/AVI is
  readable and the active `.partial.*` is gone after successful finalization.
- While a take is active, exercise the native Reboot and Shutdown actions on
  separate runs. Confirm the recording finalizes before the board leaves the
  menu, then confirm the next boot does not leave an orphaned recorder or
  partial file.
- Where a controlled stress fixture can safely drop audio or accepted OLED
  timeline data, verify `.incomplete.wav`/`.incomplete.avi` handling and keep
  the evidence. Do not pull power or storage merely to manufacture a failure;
  mark deliberate incomplete handling `NOT RUN` when it cannot be induced
  safely.
- Run `ffprobe` on every retained WAV and AVI, then open representative files
  in VLC, mpv, `ffplay`, or Audacity/a DAW as appropriate. Confirm WAV audio and
  AVI MJPEG plus PCM streams, 44.1 kHz stereo audio, 128x128 video, and 10 fps.
- Listen through the full representative takes for discontinuities, missing
  audio, audible glitches, or A/V drift. Save the recording result, player or
  `ffprobe` output, and relevant `journalctl -u octessera.service` or desktop
  runtime logs under the board's recording evidence directory.

## Post-FAT action

- Keep the Orange current-parent respin lane nonpublishing and boot-neutral until
  exact constructor-image qualification and FAT close. Raspberry remains
  constructor-only for image replacement.
