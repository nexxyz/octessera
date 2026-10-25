# Image construction and checks

The two fixed board image paths are separate: Raspberry uses pi-gen and Orange
uses Armbian. Building an image is separate from hardware testing.

Which workflow builds what:

| Workflow | Builds | Use it for |
| --- | --- | --- |
| `release-board-artifacts.yml` | Both production images, runtimes, kernel package, and device ZIPs | Release images and full constructor runs (see [Board image dispatch](#board-image-dispatch)) |
| `armbian-image.yml` | Orange **diagnostic** images only | Validating `userpatches/` and Armbian inputs, bring-up images |

## Armbian workflow

`.github/workflows/armbian-image.yml` builds through `armbian/build`. Both of
its build jobs pass `image_kind: diagnostic`, so its images never contain the
production runtime bundle or `octessera.service`.

Local checks:

```bash
bash -n userpatches/customize-image.sh tools/armbian-image/validate.sh
tools/armbian-image/validate.sh
shellcheck userpatches/customize-image.sh tools/armbian-image/validate.sh
actionlint .github/workflows/armbian-image.yml
```

The validation path needs `dtc`, `fdtoverlay`, and `fdtget` from
`device-tree-compiler`.

The workflow defaults already describe the Orange board (`orangepizero2w`,
`trixie`, `current`, `minimal`, `sha,img,xz`). The four Octessera extensions
(`octessera_midi`, `octessera_audio`, `octessera_sd2`,
`octessera_image_sanitize`) are mandatory, and
`tools/armbian-image/resolve-armbian-extensions.sh` appends any you leave out
of `extensions`.

Validation only:

```bash
gh workflow run armbian-image.yml -f run_build=false
# Optional public values are one compact JSON object:
gh workflow run armbian-image.yml -f run_build=false \
  -f 'public_inputs={"public_preset_configuration_url":"https://example.invalid/preset.conf"}'
```

A full build needs a pinned 40-character Armbian commit; `validate.sh` refuses
`run_build=true` with anything else:

```bash
gh workflow run armbian-image.yml \
  -f run_build=true \
  -f artifact_mode=public-generic \
  -f armbian_build_ref=<40-character-armbian-commit>
```

Never pass secrets as workflow inputs. Public generic builds take the
board/release/kernel/UI/compression/extensions inputs and one bounded public
JSON input, and nothing like raw Wi-Fi, user, or SSH values.
`artifact_mode=private-personalized` reads its first-run payload from the
protected `armbian-image-personalized` environment secrets, only runs from
`main` or a tag, and its artifacts are never release assets.

For workflow or `userpatches/` changes, run:

```bash
bash tools/armbian-image/validate.sh
python3 tools/pi-image/test-board-profile.py
node --check userpatches/overlay/usr/local/share/octessera-setup-ui/js/app.js
git diff --check
```

## Constructor procedure

1. Cross-build the matching native binary first.
2. Run the Raspberry pi-gen and Orange Armbian constructors (see
   [Board image dispatch](#board-image-dispatch)).
3. Smoke-check the result with `tools/armbian-image/smoke-built-image.sh`
   (Orange) or `tools/pi-image/verify-sanitized-image.sh` (Raspberry), then use
   the [deployment workflow](deployment.md) and the board bring-up procedure
   for hardware testing.

The Orange check runs inside the shared Armbian build action on every image. It
reads the root partition with `debugfs` and checks that the three Octessera
`.dtbo` overlays are installed and listed in `user_overlays`, that a production
image has `octessera.service` enabled and the release binary matches the runtime
bundle (a diagnostic image must have neither), that no SSH keys or
NetworkManager profiles are baked in, and that the legal notice tree is
canonical.

The Raspberry check is a small smoke test that `release-board-artifacts.yml`
runs on the finished ZIP. It mounts the image and checks that
`octessera.service` is enabled, the release binary matches the runtime bundle,
the DAC/OLED/Wi-Fi overlays and the `config.txt` marker are present, the
initramfs carries `octessera-pi` and the boot splash script, no SSH keys or
NetworkManager profiles are baked in, and the legal notice tree is canonical.

## Board image dispatch

`release-board-artifacts.yml` is the full constructor for both boards. The
release workflow calls it, and you can dispatch it on its own with the three
required inputs:

```bash
gh workflow run release-board-artifacts.yml \
  -f tag=v<x.y.z> -f version=<x.y.z> -f source_sha=<40-character-commit>
```

It checks out exactly `source_sha`, builds each board runtime once, builds the
Raspberry kernel package, and then builds the Raspberry pi-gen image, the
Orange production Armbian image, and both device ZIPs from those runtimes. Every
image change goes through this workflow; runtime-only changes reach the boards
through the device updater.

## OLED boot layer checks

Run these source and contract checks before an image build:

```bash
cargo test -p octessera-pi sweep_
cargo test -p octessera-pi handoff
bash tools/pi-image/test-rpi-boot-services.sh
python3 tools/armbian-image/test_orange_oled_logo.py
python3 tools/armbian-image/test_orange_oled_handoff.py
bash tools/armbian-image/test-octessera-sd-card.sh
bash tools/armbian-image/test-orange-storage-lifecycle.sh
python3 tools/armbian-image/test-orange-storage-control.py
```

The handoff checks enforce the exclusive `/run/octessera-boot` lock, strict
status/stop files, failure recovery, and no-clobber behavior. Raspberry checks
one static selected-initramfs frame; Orange checks
one static RGB565 frame and its Python closure. Unix-only lock tests need Linux
or WSL on Windows. These are source-contract checks; run hardware tests
separately. The contract is
`resources/oled/boot-sweep-v1.json`; retain its 30-frame, 1,200,000,000 ns,
25-fps and panel-orientation requirements.

The clean logo+wordmark frame follows the sweep; continuous loops hold it for a
responsive 2,000,000,000 ns rest. The selected initramfs writers leave the
final systemd sweep and handoff unchanged. The conservative 16 MHz wire budget
is 491.625 ms for 30 frames (40.96875%), below the 80% limit.

HDMI source checks cover Linux `/dev/tty1` Terminal ownership, the native grid VT
lease, no connector force or display server, and nonfatal retry when `/dev/fb0` is
missing. The splash observes the OLED handoff until `first_menu_rendered` and
reclaims a fatal startup frame when native startup fails. Orange/Raspberry HDMI and
OLED connector, framebuffer, VT, and ownership behavior still require hardware
testing; these checks do not replace it.

The native instrument-menu lifecycle is separate: PlaybackRuntime emits the
exact sleep/shutdown/reboot toasts, and the board runtime force-acknowledges the
final snapshot before preserving OLED state, zeroing LEDs, detaching, and
submitting ordinary power. Do not substitute arbitrary `systemctl` commands for
these checks.

Build both native binaries without deploying them as a constructor substitute:

```powershell
./tools/pi/build-rpi-opi-cross.ps1 -BoardProfile raspberry-pi-zero-2w -OutDir target/pi-cross-image
./tools/pi/build-rpi-opi-cross.ps1 -BoardProfile orange-pi-zero-2w -Backend wsl-docker -OutDir target/orange-pi-cross-image
```

Each output needs its adjacent metadata sidecar with the matching profile.
Inspect initramfs contents, services, mounted-image layout, OLED handoff, DAC
health, and physical display behavior separately.

## Orange production and diagnostic image modes

The shared `.github/actions/build-armbian-image` action takes an explicit
`image_kind`. `diagnostic` builds a separate bring-up image without
`octessera.service`; `production` requires the hash-bound Orange runtime
bundle and runs it as the locked `octessera-runtime` account. `armbian-image.yml`
always uses diagnostic mode, and `release-board-artifacts.yml` uses production
mode. The local WSL Docker Orange cross-build in
[`pi-development-and-profiling.md`](pi-development-and-profiling.md) produces
neither. Audio routes, accounts, and the updater boundary are described in
[`board-profiles.md`](../board-profiles.md) and
[`runtime-boundaries.md`](../runtime-boundaries.md).

The image path compiles and merges the separate
`octessera-h618-input-routing` overlay against the exact boot-selected H618 DTB.
It clears `console=ttyS0`, masks `serial-getty@ttyS0.service`, clears UART0
stdout, and releases PH0/PH1 without changing SSH. It refuses any board other
than `orangepizero2w` and resolves the boot-selected DTB. The parser
rejects duplicate assignments/tokens, commented assignments, and malformed
lists. Do not enable an overlay on another board or kernel without a matching
contract update.

## Setup portal and mutation boundaries

The Raspberry setup source is
`tools/pi-image/stage4-octessera/files/root`; the Orange source is
`userpatches/overlay`. The deletion-first layer installs one coordinator, one
config module, one request-path unit, one root setup service, and one static UI.
The fixed marker is `/run/octessera-setup-request/inbox/start`; runtime status is
the one current file `/run/octessera-setup-status/current.json`. It also installs
pinned patched wifi-connect 4.11.84 and its metadata/notice material. Exact source
paths, digests, modes, preimages, stale markers, and enabled-unit differences
remain recorded in the two image-mutation contracts. The layer removes the old setup
plumbing, leaves only the request path enabled, and does not mutate packages,
accounts, network, boot, or firmware. Missing parent packages, accounts,
executables, services, ownership, modes, xattrs, or preimages fail closed.

Run the targeted setup and security checks:

```bash
bash tools/armbian-image/test-setup-layer.sh
PYTHONDONTWRITEBYTECODE=1 python3 tools/armbian-image/test-setup-readiness.py
PYTHONDONTWRITEBYTECODE=1 python3 tools/armbian-image/test-setup-ui.py
PYTHONDONTWRITEBYTECODE=1 python3 tools/armbian-image/test-setup-request.py
PYTHONDONTWRITEBYTECODE=1 python3 tools/armbian-image/test-setup-http.py
PYTHONDONTWRITEBYTECODE=1 python3 tools/armbian-image/test-setup-flow.py
PYTHONDONTWRITEBYTECODE=1 python3 tools/armbian-image/test-setup-state.py
bash tools/armbian-image/validate.sh
node --check userpatches/overlay/usr/local/share/octessera-setup-ui/js/app.js
```

For full production image, kernel, setup-portal, sample, sanitization, and
runtime-bundle contracts, see the [Orange production reference](../../hardware/docs/orange-pi-production-reference.md).
