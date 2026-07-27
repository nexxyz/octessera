# Experimental H618 AHUB/PCM5102 path

This is a deliberately fenced empirical experiment for the Orange Pi Zero 2W
on the pinned Armbian/build commit `166b786fc978d88f4ff9ee3e33c353afb39763e8`
and Linux v6.12.30 source. It is not image customization, runtime support, or
a claim that the wiring is proven. The experiment adds a separate `octessera_plat`
AHUB0 binding for the PI1/PI2/PI3 lane and leaves the existing HDMI `ahub1`
path intact.

This experiment explicitly leaves `CONFIG_SUNXI_SYS_INFO` disabled (`n`) and does
not carry the unrelated SID/helper configuration. The dump-reg/sysinfo driver
is not needed for AHUB, PCM5102A, or board boot, so the pinned archive is kept
intact while this experiment does not enable that driver.

The overlay configures AHUB0 on APB0/DMA3/TDM0 with TX pin 0 and adds exactly
one `octessera-dac` machine card linked to built-in `ti,pcm5102a`. It only
selects PI1, PI2, and PI3 for `i2s0`, makes no MCLK claim, and does not target
PI0, SPI, I2C, UART, HDMI, or the existing codec. The fixture keeps the PI0
main encoder and HDMI/AHUB1 nodes visible.

## Local checks

On a Linux host with `python3`, `dtc`, `fdtoverlay`, `fdtget`, and ShellCheck:

```sh
cd tools/orange-pi/ahub-experiment
export ARMBIAN_SOURCE_DIR=/absolute/path/to/armbian-build-166b786
./validate-fixture.sh
./test-validate.sh
./build-ahub-experiment.sh --test
./build-ahub-experiment.sh --dry-run
./check-patch-stack.sh --kernel-source /absolute/path/to/linux-v6.12.30
```

`preflight.py` verifies the immutable Armbian commit, the complete 6.12
`series.conf` order and patch manifest, source hashes, bindings, built-in
AHUB/DAM/machine/PCM5102A configuration, ASoC registration facts, and zero
deferred probes before the fixture is compiled. `check-patch-stack.sh`
dry-runs and then applies the complete pinned archive series in a temporary
clone of Linux v6.12.30 without modifying the supplied source.
`kernel-build-plan.json` pins the build inputs; the running-kernel and log files
are deliberately small execution fixtures, not claims about an unobserved
board.

## Isolated Armbian build

The launcher clones the pinned local Armbian/build checkout into a temporary
detached worktree and uses only its pinned `archive/sunxi-6.12` core patch
series. The built-in kernel config and corrected overlay are supplied through
that worktree's temporary `userpatches/` directory, so the repository's own
`userpatches/` directory is never used. `--test` validates the full source
series, overlay merge configuration, and fake kernel packages; `--dry-run`
prints the exact build plan. The kernel-only package build is explicit and
requires an empty output directory outside this repo:

The launcher also stages one temporary build hook under
`userpatches/build-hooks/`. It renames only `vmlinuz-*`, `config-*`, and
`System.map-*` inputs carrying the `-dirty` suffix before
`kernel_package_callback_linux_image`; it does not change the kernel version,
metadata, or kernel contents. The output validator requires exactly one
nonempty `linux-image-*.deb` package.

```sh
./build-ahub-experiment.sh --run-kernel --output /absolute/path/ahub-artifacts
```

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
  --dtb /boot/dtb-<kernel>/allwinner/sun50i-h618-orangepi-zero2w.dtb

tools/orange-pi/ahub-experiment/deploy-rollback.sh deploy --yes \
  --target octessera@192.168.0.217 \
  --dtb /boot/dtb-<kernel>/allwinner/sun50i-h618-orangepi-zero2w.dtb

tools/orange-pi/ahub-experiment/deploy-rollback.sh rollback --yes \
  --target octessera@192.168.0.217 \
  --dtb /boot/dtb-<kernel>/allwinner/sun50i-h618-orangepi-zero2w.dtb \
  --backup-id ID
```

The overlay directory is derived from the supplied DTB's sibling `overlay`
directory. Set `SSH_BIN` or `SCP_BIN` to an executable wrapper when the
dedicated Orange Pi SSH key needs custom options.

No deploy or board command is run by the fixture test suite.
