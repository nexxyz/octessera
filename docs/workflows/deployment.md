# Pi and board deployment

Deployment changes a real board, so each step here needs a fresh go-ahead
from whoever owns the board. SSH reachability, an earlier approval, or a
read-only probe does not cover the next state-changing action. Raspberry
helpers never deploy to Orange, and the Orange wrappers never touch a
Raspberry.

## Raspberry development deployment

Provision a development Raspberry Pi, or refresh its tracked OS and boot
configuration, with `provision-pi.ps1`. It is safe to repeat. See
[Pi provisioning](../../tools/pi/provision/README.md) for `-UpdateInitramfs`.

```powershell
$PiTarget = "pi@<PI_HOST>"
./tools/pi/provision-pi.ps1 -Target $PiTarget -BoardProfile raspberry-pi-zero-2w
```

Then the fast path, cross-building on the PC:

```powershell
$PiTarget = "pi@<PI_HOST>"
./tools/pi/build-rpi-opi-cross.ps1
./tools/pi/deploy-rpi-fast.ps1 -Target $PiTarget -LocalBinary target/pi-cross/octessera-pi -NoTail
```

The deploy checks the adjacent `target/pi-cross/octessera-pi.metadata.json`,
uploads the binary, restarts the service, and tails logs unless you pass
`-NoTail`. It never changes OS or boot configuration and never rebuilds the
initramfs, so provision first when the OS, boot configuration, splash binary,
or splash assets changed.

The cross-builder accepts exactly `raspberry-pi-zero-2w` (default) and
`orange-pi-zero-2w`, selecting the matching Cargo feature and metadata sidecar.
On Windows it uses WSL2 Docker automatically when available. Native
cross-builds work with an ARM Linux sysroot and cross `pkg-config` for ALSA.

Building on the Pi itself also works, slowly:

```bash
cargo build -p octessera-pi --features hardware-raspberry-pi-zero-2w
# Low-memory variant:
CARGO_BUILD_JOBS=1 cargo build --profile pi-dev -p octessera-pi --features hardware-raspberry-pi-zero-2w
```

If you need a source sync instead of a binary, `deploy-rpi-fast.ps1 -SyncOnly`
keeps the remote Cargo cache. `-CleanRemote` throws that cache away, which you
will regret on a Pi Zero.

## Orange development deployment (not a release update)

This keeps a development build running on an Orange board after the terminal
closes. Build with the Orange builder, release profile, from a clean tree:

```powershell
$OrangeTarget = "octessera@<ORANGE_HOST>"
./tools/orange-pi/build-opi-cross.ps1 -Binary octessera-pi -Profile release
# If a development build is already active, return to the installed release first:
./tools/orange-pi/deploy-opi-fast.ps1 -Target $OrangeTarget -Restore
./tools/orange-pi/deploy-opi-fast.ps1 -Target $OrangeTarget -LocalBinary target/orange-pi-cross/octessera-pi
# When finished:
./tools/orange-pi/deploy-opi-fast.ps1 -Target $OrangeTarget -Restore
```

The deploy only accepts the unmodified `build-opi-cross.ps1` release-profile
sidecar built from the current `HEAD`. Output from the shared
`build-rpi-opi-cross.ps1 -BoardProfile orange-pi-zero-2w` is fine for the
timing smoke but is rejected here. The deploy also refuses to run while any
service drop-in exists, which is why an active development build needs
`-Restore` first. It needs passwordless sudo for the `octessera` account.

What it changes, what it leaves alone, and how rollback works are described in
[Orange Pi tools](../../tools/orange-pi/README.md#persistent-orange-development-runtime-explicit-opt-in).
A running process and a quiet journal do not prove glitch-free audio; listen
and exercise the controls before calling a build good.

## Orange input-routing deployment

For an existing Orange board, use the checked no-reboot wrapper. It records
exact DTB/overlay hashes, prior boot files, and serial-getty state under
`/var/lib/octessera/input-routing-backups/<id>/`.

```powershell
$OrangeTarget = "octessera@<ORANGE_HOST>"
.\tools\orange-pi\provision-input-routing.ps1 -Target $OrangeTarget -Preflight
.\tools\orange-pi\provision-input-routing.ps1 -Target $OrangeTarget -Apply
# after an operator-approved reboot, rerun -Preflight
.\tools\orange-pi\provision-input-routing.ps1 -Target $OrangeTarget -RollbackId <backup-id>
```

Apply never reboots, and neither does rollback; the reboot is yours. The
separate contract is
[`orange-pi-input-routing.md`](../../hardware/docs/orange-pi-input-routing.md).

## Stateful board actions

Connected-hardware loops need explicit operator authorization immediately
before every package or boot change, service change, reboot, deployment,
GPIO/I2C/SPI activity, audio playback, or USB gadget bind/unbind. Image builds
and their gates are in
[`image-construction-and-proof.md`](image-construction-and-proof.md).

## Hardware runtime debug loop

Use the real hardware loop for Pi-only behavior, input latency, OLED rendering,
LEDs, encoders, menu timing, sample playback, and audio stutter. Automated
checks cannot tell you whether an encoder feels laggy or the OLED is readable
from arm's length.

1. Provision first for a new Pi, an OS/configuration change, or a boot-splash
   change, then cross-build and deploy as above.
2. Pull service logs and profile summaries when the behavior is unclear;
   switch `OCTESSERA_PI_UI_PROFILE=1` off again after profiling.
3. Fix the source path. Do not add fallbacks for broken Octessera wiring.
4. Prefer keyed fast paths over broad `apply_menu_state()` and keep autosave
   serialization off rapid input paths.
