# Raspberry Pi first boot and OLED handoff

This page describes the current source contract and constructor image behavior.
Read the [board qualification and status page](board-qualification.md) for the
boundary between source/build proof and physical-board qualification. The boot
contract is not a claim that every existing flashed card has been qualified.

## Console welcome and inactive UART safety

The constructor installs one canonical `/etc/profile.d/octessera-welcome.sh`.
It prints the Octessera welcome only for an interactive terminal, only once per
shell environment, and stays quiet for noninteractive commands or redirected
output. An empty `/home/pi/.hushlogin` keeps the distro login text from crowding
the little greeting; it does not remove Octessera's own profile script.

The Raspberry image declares the UART inactive before boot finalization: it
removes `console=serial0`, `console=ttyAMA0`, and `console=ttyS0` tokens, sets
`enable_uart=0`, disables Bluetooth, and masks the serial-getty units with
`/dev/null`. This is a declarative image safety state. There is no post-boot
Raspberry UART release utility or ownership operation. It does not edit PAM,
update-motd, or the runtime/setup mutation entries.

This matters because encoder switch `SW3` is wired to GPIO14 (physical pin 8),
the Raspberry UART TX pin. With the UART inactive, GPIO14 can remain an input
for reliable SW3 operation; do not re-enable the serial console on a built
instrument.

## Expected boot animation

On a constructor-qualified Raspberry Pi Zero 2 W image, the OLED boot sweep is
the same as Orange:

- magenta, green, yellow, and cyan bands, 8 px each;
- a rigid 45° panel-facing lean toward the top-right while the mounted SSD1351 controller origin decreases and the train travels left-to-right; canonical bottom-to-top coordinates use `bottom_origin - row_y`;
- only white source pixels are recolored;
- 30 frames across 1.2 seconds (25 fps), followed by a responsive 2-second rest in the continuous loop.

Raspberry's initramfs writes one static logo+wordmark frame and stops. The
root-installed `octessera-boot-splash.service` is the sole animator and starts
concurrently during systemd boot. The runtime requests release, waits for the exclusive
`/run/octessera-boot` lock, adopts the already initialized OLED without
resetting it, and stops the animation immediately before an acknowledged first
normal menu frame. Raspberry first-menu readiness requires that real OLED write
acknowledgement. For the shared audio contract,
every non-empty set is valid; Jack is required only when selected, recognized
disconnected USB or HDMI routes may wait, a selected route fault blocks
readiness, and no route is a fallback. On the live Raspberry Pi Zero 2 W board,
kernel `6.12.93+rpt-rpi-v8` exposes the exact connector paths
`/sys/class/drm/card0-HDMI-A-1/{status,edid}`. The runtime pins that card0
identity and does not scan or fall back to card1. This is connector identity
evidence only; it does not claim connected HDMI audio or audible qualification.

Sleep, resume, and shutdown/reboot are separate display paths. They should not
restart the boot sweep or share its writer. If the OLED is blank, static,
flickering, or shows two writers during a future hardware check, stop and
record it for qualification rather than treating it as normal boot behavior.

For the shared setup flow, see [Open or reopen the full setup portal](setup-portal.md).
The Raspberry gadget applies the saved default from `/home/pi/presets/default.json`;
save device settings and use the confirmed apply/reboot action before changing
the host-visible USB composition.
For wiring and assembly, see the [pinout and connections](pinout-and-connections.md)
and [build and assembly manual](assembly-manual.md).
