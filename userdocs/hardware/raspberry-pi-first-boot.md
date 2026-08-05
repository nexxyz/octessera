# Raspberry Pi first boot and OLED handoff

This page describes the current source contract and constructor image behavior.
The boot-layer source is bound and tested, but a newly respun image has not yet
been physically qualified, so treat the flashed-card notes below as a promise
of the constructor inputs rather than a claim that every existing card has
them.

## Console welcome and serial ownership

The constructor installs one canonical `/etc/profile.d/octessera-welcome.sh`.
It prints the Octessera welcome only for an interactive terminal, only once per
shell environment, and stays quiet for noninteractive commands or redirected
output. An empty `/home/pi/.hushlogin` keeps the distro login text from crowding
the little greeting; it does not remove Octessera's own profile script.

The Raspberry constructor also releases the serial console after the selected
kernel is finalized. It removes `console=serial0`, `console=ttyAMA0`, and
`console=ttyS0` tokens, sets `enable_uart=0`, disables Bluetooth, and masks the
serial-getty units with `/dev/null`. The same checked operation is available to
live provisioning, and supports either `/boot/firmware` or `/boot` without
guessing when both layouts are present. It does not edit PAM, update-motd, or
the runtime/setup mutation entries.

## Expected boot animation

On a constructor-qualified Raspberry Pi Zero 2 W image, the OLED boot sweep is
the same as Orange:

- cyan, yellow, green, and magenta bands, 8 px each;
- a +8 px lean toward the top-right;
- only white source pixels are recolored;
- 24 frames across one second, cycling without an added pause.

Initramfs runs one bounded foreground cycle and fully reaps it. Early userspace
then loops the animation while the native runtime starts. The runtime requests
release, waits for the exclusive `/run/octessera-boot` lock, adopts the already
initialized OLED without resetting it, and stops the animation immediately
before an acknowledged first normal menu frame. Raspberry first-menu
readiness requires that real OLED write acknowledgement; Orange has the extra
DAC-health gate.

Sleep, resume, and shutdown/reboot are separate display paths. They should not
restart the boot sweep or share its writer. If the OLED is blank, static,
flickering, or shows two writers during a future hardware check, stop and
record it for qualification rather than treating it as normal boot behavior.

For the shared setup flow, see [Open or reopen the full setup portal](setup-portal.md).
For wiring and assembly, see the [pinout and connections](pinout-and-connections.md)
and [build and assembly manual](assembly-manual.md).
