# Image construction

The two fixed boards have separate image paths: Raspberry uses pi-gen and
Orange uses Armbian. Building an image is separate from hardware testing.

## What gets built

`release-board-artifacts.yml` builds everything an image needs, for both
boards, from one commit:

- each board runtime, built once;
- the Raspberry kernel package;
- the Raspberry pi-gen image and the Orange Armbian production image;
- both device update ZIPs.

The release workflow calls it, and you can dispatch it on its own with the
three required inputs:

```bash
gh workflow run release-board-artifacts.yml \
  -f tag=v<x.y.z> -f version=<x.y.z> -f source_sha=<40-character-commit>
```

It creates no tag or release. A run of this workflow is the proof for any
change to pi-gen stages, Armbian userpatches, the image build action, or the
kernel tooling; local checks only catch mistakes early. Runtime-only changes
reach the boards through the device updater and need no image run.

The Orange image goes through `armbian/build` via the shared
`.github/actions/build-armbian-image` action, which accepts only the Orange
board (`orangepizero2w`, `current` kernel) and the reviewed Armbian commit. It
needs the hash-bound Orange runtime bundle (`octessera-pi`,
`octessera-runtime.json`, `SHA256SUMS`) and runs it as the locked
`octessera-runtime` account. The four Octessera extensions (`octessera_midi`,
`octessera_audio`, `octessera_sd2`, `octessera_image_sanitize`) are mandatory;
`tools/armbian-image/resolve-armbian-extensions.sh` appends any you leave out
of `extensions`.

The Orange image also merges the `octessera-h618-input-routing` overlay into
the boot-selected H618 DTB, frees the UART0 pins for input (no serial console
on `ttyS0`), and leaves SSH alone. Do not enable that overlay on another board
or kernel.

## Smoke checks on the built image

Each finished image gets one small smoke check inside the workflow:

- Orange: `tools/armbian-image/smoke-built-image.sh` reads the root partition
  with `debugfs` and checks that the three Octessera `.dtbo` overlays are
  installed and listed in `user_overlays`, `octessera.service` is enabled, the
  release binary matches the runtime bundle, no SSH keys or NetworkManager
  profiles are baked in, and the legal notice tree is canonical.
- Raspberry: `tools/pi-image/verify-sanitized-image.sh` mounts the image and
  checks that `octessera.service` is enabled, the release binary matches the
  runtime bundle, the DAC/OLED/Wi-Fi overlays and the `config.txt` marker are
  present, the initramfs carries `octessera-pi` and the boot splash script, no
  SSH keys or NetworkManager profiles are baked in, and the legal notice tree is
  canonical.

After that, use the [deployment workflow](deployment.md) and the board
bring-up procedure for hardware testing. Inspect the OLED handoff, DAC health,
HDMI, and the physical display on the boards; the checks above do not replace
that.

## Local checks before dispatching

For workflow or `userpatches/` changes:

```bash
bash -n userpatches/customize-image.sh tools/armbian-image/validate.sh
bash tools/armbian-image/validate.sh
shellcheck userpatches/customize-image.sh tools/armbian-image/validate.sh
actionlint .github/workflows/release-board-artifacts.yml
python3 tools/pi-image/test-board-profile.py
node --check userpatches/overlay/usr/local/share/octessera-setup-ui/js/app.js
git diff --check
```

`validate.sh` needs `dtc`, `fdtoverlay`, and `fdtget` from
`device-tree-compiler`, and it also runs the image source tests listed in
`tools/armbian-image/validate-image-tests.sh`.

For the OLED boot layer:

```bash
cargo test -p octessera-pi sweep_
cargo test -p octessera-pi handoff
bash tools/pi-image/test-rpi-boot-services.sh
python3 tools/armbian-image/test_orange_oled_logo.py
python3 tools/armbian-image/test_orange_oled_handoff.py
```

The Unix-only lock tests need Linux or WSL on Windows. The boot sweep contract
is `resources/oled/boot-sweep-v1.json`, and the `/run/octessera-boot` handoff
is described in [`runtime-boundaries.md`](../runtime-boundaries.md).

For the setup portal (Raspberry source in
`tools/pi-image/stage4-octessera/files/root`, Orange source in
`userpatches/overlay`):

```bash
bash tools/armbian-image/test-setup-layer.sh
PYTHONDONTWRITEBYTECODE=1 python3 tools/armbian-image/test-setup-readiness.py
PYTHONDONTWRITEBYTECODE=1 python3 tools/armbian-image/test-setup-ui.py
PYTHONDONTWRITEBYTECODE=1 python3 tools/armbian-image/test-setup-request.py
PYTHONDONTWRITEBYTECODE=1 python3 tools/armbian-image/test-setup-http.py
PYTHONDONTWRITEBYTECODE=1 python3 tools/armbian-image/test-setup-flow.py
PYTHONDONTWRITEBYTECODE=1 python3 tools/armbian-image/test-setup-state.py
```

The setup layer installs one coordinator, one config module, one request-path
unit, one root setup service, one static UI, and the pinned patched
wifi-connect 4.11.84. The request marker is
`/run/octessera-setup-request/inbox/start`; status is
`/run/octessera-setup-status/current.json`.

For the full Orange production image, kernel, setup-portal, sample, and
runtime-bundle contracts, see the
[Orange production reference](../../hardware/docs/orange-pi-production-reference.md).
Audio routes, accounts, and the updater boundary are in
[`board-profiles.md`](../board-profiles.md) and
[`runtime-boundaries.md`](../runtime-boundaries.md).
