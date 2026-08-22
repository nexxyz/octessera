# Raspberry Pi first boot and OLED handoff

This page describes the current source-defined Raspberry constructor contract.
It does not claim that every flashed card or assembled board has been physically
qualified. Read [board qualification and status](board-qualification.md) for
that boundary.

## Published image and current constructor boundary

The boot, OLED handoff, and lifecycle expectations below apply only to an image
built from these contracts and identified by release and qualification evidence.
The live trusted v0.7.5 images are runtime/setup parents. They remain usable
published images for their documented runtime and setup paths, but they do not
prove the current constructor layer. A new full constructor image still needs
build validation and physical qualification.

## Before boot

- Use the Raspberry Pi Zero 2 W image from the [current release page](https://github.com/nexxyz/octessera/releases).
- Keep the board, OLED, controls, DAC, and power path accessible for the bench
  checks. See [safety and power](safety-and-power.md) before connecting power or
  a host cable.
- Do not re-enable the serial console. Encoder switch `SW3` uses GPIO14
  (physical pin 8), the Raspberry UART TX pin.

## Console welcome and UART state

The constructor installs one canonical `/etc/profile.d/octessera-welcome.sh`.
It prints the Octessera welcome only for an interactive terminal, only once per
shell environment, and stays quiet for noninteractive commands or redirected
output. An empty `/home/pi/.hushlogin` keeps distro login text from crowding the
greeting; it does not remove Octessera's profile script.

The image declares the UART inactive before boot finalization: it removes
`console=serial0`, `console=ttyAMA0`, and `console=ttyS0`, sets `enable_uart=0`,
disables Bluetooth, and masks the serial-getty units with `/dev/null`. There is
no post-boot UART release utility or ownership operation. With the UART
inactive, GPIO14 remains an input for reliable `SW3` operation.

## Expected boot and first menu

The freshly flashed image boots offline without waiting for a network. NetworkManager
remains available, but the standalone DNS and wait-online units stay disabled and no
hotspot or SSH service starts by itself. Networking and SSH are deliberate opt-in
actions from `System > Configure WiFi > Open Portal`.

On a constructor-qualified image, the OLED boot sweep is:

- magenta, green, yellow, and cyan bands, 8 px each;
- a rigid 45° panel-facing lean toward the top-right while the mounted SSD1351
  controller origin decreases and the train travels left-to-right;
- bottom-to-top coordinates using `bottom_origin - row_y`;
- recoloring of white source pixels only; and
- 30 frames over 1.2 seconds (25 fps), followed by a responsive 2-second rest
  in the continuous loop.

The Raspberry initramfs writes one static logo-and-wordmark frame and stops.
The root-installed `octessera-boot-splash.service` is the sole animator. Native
startup waits for the exclusive `/run/octessera-boot` lock, adopts the already
initialized OLED without resetting it, and stops the animation immediately
before an acknowledged first normal menu frame. A queued frame is not enough.

If native ownership has not arrived by the 30-second handoff window, that same
splash owner writes a persistent dimmed `FIRST-RUN` / `HOUSEKEEPING` / `PLEASE
WAIT` status and continues polling the handoff state as the sole OLED writer.
This is a delayed-start legibility/recovery mitigation, not proof that
filesystem expansion is active or complete. A timeout alone does not invoke
black/off failure cleanup; only a genuine writer error or termination signal
does.

For selected audio outputs, every non-empty set is valid. Jack is required only
when selected; recognized disconnected USB or HDMI routes may wait; a selected
route fault blocks readiness; and no route is a fallback. The live Raspberry
connector identity is `/sys/class/drm/card0-HDMI-A-1/{status,edid}` on kernel
`6.12.93+rpt-rpi-v8`. This is connector evidence only, not connected HDMI audio
or audible qualification.

Sleep, resume, and shutdown/reboot use separate display paths. If the OLED is
blank, static, flickering, or shows two writers during a hardware check, stop
and record it for qualification rather than treating it as normal boot.

## Instrument lifecycle messages

The confirmed instrument-menu lifecycle keeps presentation native:

- sleep shows `Going to sleep`;
- reboot shows `Rebooting`; and
- shutdown shows `Shutting down`.

Each message appears over the existing static logo-and-wordmark frame. Native
acknowledges the final snapshot before preserving the OLED while it detaches and
submits the board power request. These strings do not describe arbitrary
administrative power commands.

## Apply saved device settings

For the shared setup flow, see [Open or reopen the full setup portal](setup-portal.md).
The Raspberry gadget reads the saved default from
`/home/pi/presets/default.json`. Save device settings and use the confirmed
apply/reboot action before changing the host-visible USB composition.

Image construction stages the complete 320-file sample artifact under
`/home/pi/samples`: 318 WAV rows are sampler-loadable, while two AIFF rows are
retained as attribution-inventory metadata outside the WAV-only browser/decoder.
User samples remain supported. USB Audio and USB MIDI are experimental/local
bench validation only, with defaults disabled. Do not call the path supported
without an authorized USB identity and an exact-image electrical/manual FAT
record. Keep the board on the enclosure power input and use a data-only or
power-isolating host cable; see [safety and power](safety-and-power.md).

For wiring and the open-assembly checks, use [pinout and connections](pinout-and-connections.md),
[the assembly manual](assembly-manual.md), and [troubleshooting](../troubleshooting.md).
