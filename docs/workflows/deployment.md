# Pi and board deployment

Deployment is a state-changing hardware path. Raspberry deployment helpers do
not deploy to Orange, and Orange input-routing changes use their separate
board-specific wrapper. SSH reachability, an earlier approval, and a read-only
probe do not authorize the next state-changing action.

## Raspberry development deployment

Provision a development Raspberry Pi, or refresh its tracked OS and boot
configuration, with `./tools/pi/provision-pi.ps1`. It is safe to repeat.
Initramfs refresh is opt-in via `-UpdateInitramfs`; the default removes retired
Raspberry animation inputs without rebuilding the selected image. `-WakeTrace`
enables the development wake trace in the service configuration.

```powershell
./tools/pi/provision-pi.ps1 -Target pi@192.168.0.218 -BoardProfile raspberry-pi-zero-2w
```

Preferred fast path:

```powershell
./tools/pi/build-pi-cross.ps1
./tools/pi/deploy-pi-fast.ps1 -Target pi@192.168.0.218 -LocalBinary target/pi-cross/octessera-pi -NoTail
# The adjacent target/pi-cross/octessera-pi.metadata.json is checked during deployment.
# If boot-splash assets changed, provision first; this path never rebuilds initramfs.
```

The cross-builder accepts exactly `raspberry-pi-zero-2w` (default) and
`orange-pi-zero-2w`, selecting the matching Cargo feature and metadata sidecar.
On Windows it uses WSL2 Docker automatically when available. Native cross-builds
remain supported with an ARM Linux sysroot and cross `pkg-config` for ALSA.

On a Pi or configured cross environment:

```bash
cargo build -p octessera-pi --features hardware-raspberry-pi-zero-2w
```

Low-resource on-Pi path:

```bash
CARGO_BUILD_JOBS=1 cargo build --profile pi-dev -p octessera-pi --features hardware-raspberry-pi-zero-2w
```

The fast deployment helper transfers Raspberry binary/source content, restarts
the configured service, and optionally tails logs. Provision separately when
the OS, boot configuration, splash binary, or splash assets changed.

## Orange input-routing deployment

For an existing Orange board, use the checked no-reboot wrapper. It records
exact DTB/overlay hashes, prior boot files, and serial-getty state under
`/var/lib/octessera/input-routing-backups/<id>/`.

```powershell
.\tools\orange-pi\provision-input-routing.ps1 -Preflight
.\tools\orange-pi\provision-input-routing.ps1 -Apply
# after an operator-approved reboot, rerun -Preflight
.\tools\orange-pi\provision-input-routing.ps1 -RollbackId <backup-id>
```

Apply never reboots; rollback also leaves reboot to the operator. The separate
contract is [`orange-pi-input-routing.md`](../../hardware/docs/orange-pi-input-routing.md).

## Stateful board actions

Connected-hardware loops require fresh, explicit operator authorization
immediately before every package or boot change, service change, reboot,
deployment, GPIO/I2C/SPI activity, audio playback, or USB gadget bind/unbind.

For an Armbian image validation-only run:

```bash
gh workflow run armbian-image.yml \
  -f board=orangepizero2w \
  -f release=trixie \
  -f kernel_branch=current \
  -f ui=minimal \
  -f compression=sha,img,xz \
  -f 'extensions=preset-firstrun octessera_midi octessera_image_sanitize' \
  -f run_build=false \
  -f artifact_mode=public-generic \
  -f armbian_build_ref=main
```

Change `run_build=true` only with a reviewed full 40-character Armbian commit
SHA. Public builds stay secret-free; first-boot setup handles Wi-Fi and SSH on
the device. See [`image-construction-and-proof.md`](image-construction-and-proof.md)
for the full image gates.

## Hardware runtime debug loop

Use the real hardware loop for Pi-only behavior, input latency, OLED rendering,
LEDs, encoders, menu timing, sample playback, and audio stutter. Automated
checks cannot prove tactile timing or display readability.

1. For a new Pi, OS/configuration change, or boot-splash change, provision first,
   then cross-build and deploy from the PC:

   ```powershell
   ./tools/pi/provision-pi.ps1 -Target pi@192.168.0.218
   ./tools/pi/build-pi-cross.ps1
   ./tools/pi/deploy-pi-fast.ps1 -Target pi@192.168.0.218 -LocalBinary target/pi-cross/octessera-pi -NoTail
   ```
2. Request a focused hardware observation with the control path, expected
   result, and failure signature.
3. Pull service logs and profile summaries when the observation is unclear;
   disable `OCTESSERA_PI_UI_PROFILE=1` after profiling.
4. Inspect `pi-ui-profile` and `menu-key-profile` output before broad refactors.
5. Fix the source path. Do not add fallbacks for broken Octessera wiring.
6. Prefer keyed fast paths over broad `apply_menu_state()` and keep autosave
   serialization off rapid input paths.
7. Run targeted Rust checks before redeploying when possible, then repeat the
   observation.
8. Before a stable hardware milestone, use QA/oracle review for risky changes
   and run the pre-push hook.
