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
# When finished, return to the installed release:
./tools/pi/deploy-rpi-fast.ps1 -Target $PiTarget -Restore
```

The deploy uploads the binary, runs it as a development build (see
[below](#how-a-development-deploy-works-both-boards)), and tails the logs
unless you pass `-NoTail`. Redeploying replaces the running development build.
It never changes OS or boot configuration and never rebuilds the initramfs, so
provision first when the OS, boot configuration, splash binary, or splash
assets changed.

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
copies the tree to `/home/pi/octessera-dev` and keeps the remote Cargo cache.
Adding `-CleanRemote` throws that cache away, which you will regret on a Pi
Zero. Binary deploys and `-Restore` need passwordless sudo for `pi`, the same
as provisioning.

## Orange development deployment (not a release update)

This keeps a development build running on an Orange board after the terminal
closes:

```powershell
$OrangeTarget = "octessera@<ORANGE_HOST>"
./tools/orange-pi/build-opi-cross.ps1 -Binary octessera-pi -Profile release
./tools/orange-pi/deploy-opi-fast.ps1 -Target $OrangeTarget -LocalBinary target/orange-pi-cross/octessera-pi
# When finished, return to the installed release:
./tools/orange-pi/deploy-opi-fast.ps1 -Target $OrangeTarget -Restore
```

Redeploying simply replaces the running development build. The deploy needs
passwordless sudo for the `octessera` account.

## How a development deploy works (both boards)

Both deploy tools share `tools/board-dev/dev-runtime.sh`. It checks the board
model and that `octessera.service` expects this board's profile, installs the
uploaded binary as `/opt/octessera/dev/octessera-pi` after comparing its
SHA-256 with the local file, and points the service at it with one drop-in,
`/etc/systemd/system/octessera.service.d/90-octessera-dev.conf`, which only
replaces `ExecStart`. It then restarts the service and waits for the new
process to stay up.

`-Restore` removes the drop-in, restarts the installed release, and deletes
`/opt/octessera/dev`. Neither action touches `current`, releases, updater
state, boot files, or the image. If the development build does not come up,
read the journal and run `-Restore`. A running process and a quiet journal do
not prove glitch-free audio; listen and exercise the controls before calling a
build good.

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
