# Image construction and checks

The two fixed board image paths are separate: Raspberry uses pi-gen and Orange
uses Armbian. Their source-bound constructor contracts are separate from
runtime-only respins and hardware testing.

## Armbian workflow

`.github/workflows/armbian-image.yml` builds through `armbian/build`.
Validation-only runs may inspect the default ref; every image intended for
hardware validation requires a pinned full 40-character Armbian commit SHA.
Local checks are:

```bash
bash -n userpatches/customize-image.sh tools/armbian-image/validate.sh
tools/armbian-image/validate.sh
shellcheck userpatches/customize-image.sh tools/armbian-image/validate.sh
actionlint .github/workflows/armbian-image.yml
```

The validation path requires `dtc`, `fdtoverlay`, and `fdtget` from
`device-tree-compiler`. GitHub validation-only smoke test:

```bash
gh workflow run armbian-image.yml -f run_build=false -f artifact_mode=public-generic
# Optional public values are one compact JSON object:
gh workflow run armbian-image.yml -f 'public_inputs={"public_preset_configuration_url":"https://example.invalid/preset.conf"}'
```

The Orange validation shape is:

```bash
gh workflow run armbian-image.yml \
  -f board=orangepizero2w \
  -f release=trixie \
  -f kernel_branch=current \
  -f ui=minimal \
  -f compression=xz \
  -f 'extensions=preset-firstrun octessera_midi octessera_image_sanitize' \
  -f run_build=false \
  -f artifact_mode=public-generic \
  -f 'public_inputs={"public_preset_configuration_url":"https://example.invalid/preset.conf"}'
```

The no-secret full-build shape is:

```bash
gh workflow run armbian-image.yml \
  -f board=orangepizero2w \
  -f release=trixie \
  -f kernel_branch=current \
  -f ui=minimal \
  -f compression=xz \
  -f 'extensions=preset-firstrun octessera_midi octessera_image_sanitize' \
  -f run_build=true \
  -f artifact_mode=public-generic \
  -f armbian_build_ref=<40-character-armbian-commit>
```

Do not pass secrets as workflow inputs. Private first-run payloads belong to
the protected environment and its repository/environment secrets.

Public generic builds may use board/release/kernel/UI/compression/extensions and
one bounded public JSON input. Do not pass raw Wi-Fi, user, SSH, or private
first-run values. Personalized builds use `artifact_mode=private-personalized`
and repository/environment secrets. Personalized artifacts are not release
assets.

For workflow or `userpatches/` changes, run:

```bash
bash tools/armbian-image/validate.sh
python3 tools/pi-image/test-board-profile.py
node --check userpatches/overlay/usr/local/share/octessera-setup-ui/js/app.js
git diff --check
```

For an extracted root filesystem or ext4 root partition image, inspect the
artifact directly:

```bash
tools/armbian-image/inspect-built-image.sh --verification-profile full-constructor <rootfs-dir-or-ext4-image>
```

Use `full-constructor` for source-built images. Orange runtime-only respins use
the `validated-parent` mode and the exact current parent record; their boot
integrity comes from the separate boot-neutral check.

## Source-bound constructor procedure

Construct each board from its source-bound boot-layer contract, not from the
Orange current-parent respin lane:

1. Freeze current source inputs and hashes in
   `resources/image-construction/boot-layers/raspberry-pi-zero-2w.json` and
   `orange-pi-zero-2w.json`; cross-build the matching native binary first.
2. Run the Raspberry pi-gen and Orange Armbian constructors. Stage the
   canonical welcome, preserve declared hushlogin behavior, encode Raspberry's
   inactive-UART state, and install each board's declared runtime/initramfs
   inputs. Regenerate selected initramfs images only in the constructor.
3. Run mounted-image checks before board deployment. Raspberry must show the
   selected initramfs, enabled early writer, welcome, exact hushlogin,
   inactive-UART and serial-console state, and expected service masks. Orange
   must show the welcome, root-installed renderer/lifecycle/assets, fixed
   SPI/GPIO dependencies, static initramfs frame/Python closure, and one
   `/run/octessera-boot` owner with no second writer.
4. Preserve the image and its source/build metadata. Then use the
   [deployment workflow](deployment.md) and the board-specific bring-up
   procedure for hardware testing.

## Runtime-only respin boundary

Dispatch the Orange runtime-only workflow for a boot-neutral refresh:

```bash
gh workflow run respin-board-image.yml --ref main
```

This path does not change the kernel, device tree, initramfs, or base OS. Those
changes require the full constructor workflow.

## Board image dispatch

`release-board-artifacts.yml` accepts the required `tag`, `version`, and exact
`source_sha` inputs and builds both complete production images. Use the
constructor workflow for Raspberry image changes; the runtime-only path is
Orange-only.

## OLED boot layer checks

Run these source and contract checks before an image build:

```bash
cargo test -p octessera-pi sweep_
cargo test -p octessera-pi handoff
python3 tools/pi-image/test-boot-layer-contract.py
bash tools/pi-image/test-rpi-boot-services.sh
python3 tools/armbian-image/test_orange_oled_logo.py
python3 tools/armbian-image/test_orange_oled_handoff.py
python3 tools/armbian-image/test-orange-construction.py
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
./tools/pi/build-pi-cross.ps1 -BoardProfile raspberry-pi-zero-2w -OutDir target/pi-cross-phase5
./tools/pi/build-pi-cross.ps1 -BoardProfile orange-pi-zero-2w -Backend wsl-docker -OutDir target/orange-pi-cross-phase5
```

Each output needs its adjacent metadata sidecar with the matching profile.
Inspect initramfs contents, services, mounted-image layout, OLED handoff, DAC
health, and physical display behavior separately.

## Orange production and diagnostic image modes

Only `raspberry-pi-zero-2w` and `orange-pi-zero-2w` are supported cross-build
IDs. Raspberry selects `hardware-raspberry-pi-zero-2w`; Orange selects
`hardware-orange-pi-zero-2w`. The Orange image uses the locked
`octessera-runtime` account and `octessera.service`; every valid Pi audio set
includes always-on Jack. USB and HDMI are independently selectable optional
mirrors of the Jack mix; their absent, waiting, or faulted status is route-local
and never blocks or replaces Jack. HDMI audio remains separate from video. Full Armbian, kernel, device-tree, and image replacement
remains manual; the standalone manual ZIP is not an OTA asset.

The WSL Docker-only local Orange cross-build is documented in
[`pi-development-and-profiling.md`](pi-development-and-profiling.md). It does
not produce a production image or runtime bundle. The shared action's explicit
`image_kind=diagnostic` builds a separate bring-up image; `production` requires
the hash-bound Orange runtime bundle. The generic workflow uses diagnostic mode;
the release workflow invokes production mode. Diagnostic mode does not contain
or enable `octessera.service`.

The image path compiles and merges the separate
`octessera-h618-input-routing` overlay against the exact boot-selected H618 DTB.
It clears `console=ttyS0`, masks `serial-getty@ttyS0.service`, clears UART0
stdout, and releases PH0/PH1 without changing SSH. It refuses any board other
than `orangepizero2w`, resolves the boot-selected DTB, and records non-secret
SPI/input overlay hashes in `/etc/octessera/build-metadata.env`. The parser
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
python3 tools/image-respin/test_setup_contract.py
python3 tools/armbian-image/test_orange_image_proof_validated.py
python3 tools/image-respin/test_runtime_contract.py
python3 tools/image-respin/test_workflow_records.py
python3 tools/image-respin/test_workflow_static.py
node --check userpatches/overlay/usr/local/share/octessera-setup-ui/js/app.js
```

Root-required mutation and disk fixtures run in CI as
`sudo python3 tools/image-respin/test_setup_mutation.py` and
`sudo python3 -m unittest discover -s tools/image-respin -p 'test_disk_*.py'`.
The current-parent exercise is the Orange runtime-only path for boot-neutral
updates. It does not replace a constructor image or a hardware test.

For full production image, kernel, setup-portal, sample, sanitization, and
runtime-bundle contracts, see the [Orange production reference](../../hardware/docs/orange-pi-production-reference.md).
