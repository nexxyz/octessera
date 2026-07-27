# Experimental H618 AHUB/PCM5102 path

This is a deliberately fenced empirical experiment for the Orange Pi Zero 2W
on Armbian/build commit `fa7a7b2294d9e760a77630950afd460b7a0b2a26` and peeled
Linux source commit `e46dc0adfe39724bcf52cea47b8f9c9aed86a394` (v6.18.38).
Its patch directory is `archive/sunxi-6.18`, package revision is
`26.8.0-trunk.413`, and kernel ABI is `6.18.38-current-sunxi64`. It is not
image customization, runtime support, or a claim that the wiring is proven.
The launcher passes `KERNELBRANCH=commit:e46dc0adfe39724bcf52cea47b8f9c9aed86a394`
to Armbian rather than selecting a rolling kernel branch.
The experiment adds a separate `octessera_plat` AHUB0 binding for the PI1/PI2/PI3
lane and leaves the existing HDMI `ahub1` path intact.

The launcher copies the complete source kernel config, then applies only
`CONFIG_SND_SOC_PCM5102A=y`, `CONFIG_NVMEM_SUNXI_SID=y`, and
`# CONFIG_SUNXI_SYS_INFO is not set`. No other source-config settings are
replaced.

The overlay configures AHUB0 on APB0/DMA3/TDM0 with TX pin 0 and adds exactly
one playback-only `octessera-dac` machine card. Its empty codec child selects
the vendor dummy-codec fallback; it has no PCM5102A node or codec `sound-dai`
link. PI1 and PI2 use `i2s0`, while PI3 uses `i2s0_dout0`. It makes no MCLK
claim and does not target PI0, SPI, I2C, UART, HDMI, or the existing codec. The
fixture keeps the PI0 main encoder and HDMI/AHUB1 nodes visible.

The merged topology keeps the existing HDMI nodes at `/soc/ahub1_plat` and
`/soc/ahub1_mach` enabled, on TDM1, named `HDMI`, and linked through the HDMI
CPU relation. The overlay adds root `/octessera_plat` on APB0/TDM0 with DMA
request 3 and split PI1/PI2 `i2s0` plus PI3 `i2s0_dout0` pinctrl. Its
`octessera-dac` uses playback-only dummy-codec fallback with CPU and master
phandle links and no MCLK property. These Octessera nodes must be absent from
the base DTB before merge, and no root `/ahub1_*` nodes are accepted.

The kernel artifact does not embed or activate the Octessera overlay. Overlay
deployment is a separate, backed-up, preflight-gated step after kernel
validation.

## Local checks

On a Linux host with `python3`, `dtc`, `fdtoverlay`, `fdtget`, and ShellCheck:

```sh
cd tools/orange-pi/ahub-experiment
export ARMBIAN_SOURCE_DIR=/absolute/path/to/armbian-build
./validate-fixture.sh
./test-validate.sh
./build-ahub-experiment.sh --test
./build-ahub-experiment.sh --dry-run
./check-patch-stack.sh --kernel-source /absolute/path/to/linux-v6.18.38
```

`preflight.py` verifies the immutable Armbian commit, the complete 6.18
`series.conf` order and patch manifest, source hashes, bindings, built-in
AHUB/DAM/machine/PCM5102A configuration, ASoC registration facts, and zero
deferred probes before the fixture is compiled. `check-patch-stack.sh`
dry-runs and then applies the complete pinned archive series in a temporary
clone of Linux v6.18.38 without modifying the supplied source.
`kernel-build-plan.json` pins the build inputs; the running-kernel and log files
are deliberately small execution fixtures, not claims about an unobserved
board.

## Isolated Armbian build

The launcher clones the pinned local Armbian/build checkout into a temporary
detached worktree and uses only its pinned `archive/sunxi-6.18` core patch
series. It copies the complete source config into that worktree's temporary
`userpatches/` directory and applies only the three settings above; the
repository's own `userpatches/` directory is never used. `--test` validates the
full source series, overlay merge configuration, and fake kernel packages;
`--dry-run` prints the exact build plan. The kernel-only package build is
explicit and requires an empty output directory outside this repo.

No package-normalization or header-disable hook is used; the removed hooks do
not exist. Native Armbian output may contain linux-headers and linux-modules
packages, but only one linux-image package, one linux-dtb package, the required
kernel config, and `SHA256SUMS` are staged or uploaded. Headers and modules
must not appear in the staged artifact.

```sh
./build-ahub-experiment.sh --run-kernel --output /absolute/path/ahub-artifacts
```

## CI runbook

The workflow is `.github/workflows/orange-ahub-kernel.yml`. It runs for pushes to
`orange-ahub-kernel-experiment`; trigger it manually when needed:

```sh
gh workflow run orange-ahub-kernel.yml --ref orange-ahub-kernel-experiment
gh run list --workflow orange-ahub-kernel.yml --branch orange-ahub-kernel-experiment
gh run watch RUN_ID --exit-status
```

For a successful run, download `octessera-ahub-kernel`:

```sh
gh run download RUN_ID -n octessera-ahub-kernel -D /tmp/octessera-ahub-kernel
```

1. **Stage and verify the artifact.** A successful run contains exactly one
   matching `linux-image-*.deb` and `linux-dtb-*.deb`, the built kernel config,
   and `SHA256SUMS`. Native headers and modules may be built, but must not be
   staged or uploaded. Stage the image, DTB, config, and checksums, then verify
   the hashes before continuing.

For a failed run, download `orange-ahub-kernel-failure-diagnostics` instead:

```sh
gh run download RUN_ID -n orange-ahub-kernel-failure-diagnostics -D /tmp/octessera-ahub-failure
less /tmp/octessera-ahub-failure/logs/octessera-ahub-kernel-build.log
grep -nEi 'error|failed|packaging|linux-(image|dtb|headers)' \
  /tmp/octessera-ahub-failure/logs/octessera-ahub-kernel-build.log
```

2. **Pass the recovery gate and boot the kernel.** After a separate recovery
   gate, install only the matching image and DTB packages. Reboot cleanly into
   the verified `6.18.38-current-sunxi64` kernel with no Octessera DTBO active.
   Do not use `deploy-rollback.sh` to install kernel packages.

3. **Deploy the overlay only after the verified boot.** Use the running,
   versioned DTB path below. Run remote preflight, then deploy with `--yes` and
   record the printed backup ID:

```sh
tools/orange-pi/ahub-experiment/deploy-rollback.sh preflight \
  --target octessera@192.168.0.217 \
  --dtb /boot/dtb-6.18.38-current-sunxi64/allwinner/sun50i-h618-orangepi-zero2w.dtb

tools/orange-pi/ahub-experiment/deploy-rollback.sh deploy --yes \
  --target octessera@192.168.0.217 \
  --dtb /boot/dtb-6.18.38-current-sunxi64/allwinner/sun50i-h618-orangepi-zero2w.dtb
```

After deploy, use the planned reboot procedure and record the FAT results. If
the test fails, roll back the same DTB path with the recorded backup ID:

```sh
tools/orange-pi/ahub-experiment/deploy-rollback.sh rollback --yes \
  --target octessera@192.168.0.217 \
  --dtb /boot/dtb-6.18.38-current-sunxi64/allwinner/sun50i-h618-orangepi-zero2w.dtb \
  --backup-id ID
```

4. **Keep helper scope narrow.** `deploy-rollback.sh` manages the overlay and
   boot token only. It never installs kernel packages or restarts the device.

CI and package validation do not prove audio. Make no audio-success claim until
the deployed kernel and overlay pass the planned FAT on the board.

## Opt-in deploy and rollback

The helper requires an explicit target, exact DTB path, passwordless `sudo -n`,
and `--yes` for any boot-file change. It validates the overlay against the
remote DTB before copying it, records hashes for `armbianEnv.txt`, the DTB, and
the overlay in a root-owned backup, then adds the overlay name to the existing
Armbian overlay list. Rollback restores those exact backups only after checking
the manifest and hashes. It never restarts the device.

```sh
tools/orange-pi/ahub-experiment/deploy-rollback.sh preflight \
  --target octessera@192.168.0.217 \
  --dtb /boot/dtb-6.18.38-current-sunxi64/allwinner/sun50i-h618-orangepi-zero2w.dtb

tools/orange-pi/ahub-experiment/deploy-rollback.sh deploy --yes \
  --target octessera@192.168.0.217 \
  --dtb /boot/dtb-6.18.38-current-sunxi64/allwinner/sun50i-h618-orangepi-zero2w.dtb

tools/orange-pi/ahub-experiment/deploy-rollback.sh rollback --yes \
  --target octessera@192.168.0.217 \
  --dtb /boot/dtb-6.18.38-current-sunxi64/allwinner/sun50i-h618-orangepi-zero2w.dtb \
  --backup-id ID
```

The overlay directory is derived from the supplied DTB's sibling `overlay`
directory. Set `SSH_BIN` or `SCP_BIN` to an executable wrapper when the
dedicated Orange Pi SSH key needs custom options.

No deploy or board command is run by the fixture test suite.
