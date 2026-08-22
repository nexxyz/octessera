# Image construction and proof

The two fixed board image paths are separate: Raspberry uses pi-gen and Orange
uses Armbian. A source check, cross-build, or parent respin is not an image or
physical qualification result. Full image construction is a necessary slow path
and should remain release-only except for an explicitly authorized proof run.

## Armbian workflow

`.github/workflows/armbian-image.yml` builds through `armbian/build`.
Validation-only runs may inspect the default ref; every qualification image
requires a reviewed full 40-character Armbian commit SHA. Local checks are:

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

The reviewed Orange validation shape is:

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
  -f armbian_build_ref=<reviewed-40-character-armbian-commit>
```

Do not pass secrets as workflow inputs. Private first-run payloads belong to
the protected environment and its repository/environment secrets.

Public generic builds may use board/release/kernel/UI/compression/extensions and
one bounded public JSON input. Do not pass raw Wi-Fi, user, SSH, or private
first-run values. Personalized builds use
`artifact_mode=private-personalized` from trusted `main` or tags, protected
approval for `armbian-image-personalized`, and repository/environment secrets.
Private artifacts are short-lived and never release assets.

Before pushing workflow or `userpatches/` changes, also run:

```bash
bash tools/armbian-image/validate.sh
python3 tools/pi-image/test-board-profile.py
node --check userpatches/overlay/usr/local/share/octessera-setup-ui/app.js
git diff --check
```

For an extracted root filesystem or ext4 root partition image, inspect the
artifact directly:

```bash
tools/armbian-image/inspect-built-image.sh --verification-profile full-constructor <rootfs-dir-or-ext4-image>
```

Use `full-constructor` for source-built images. Trusted v0.7.5 runtime-only
respins use `legacy-runtime-only`; setup-layer respins use
`legacy-setup-layer`. Their boot integrity comes from the separate trusted
v0.7.5 boot-neutral proof.

## Source-bound constructor procedure

For a constructor refresh, construct each board from its source-bound boot-layer
contract, not from a trusted-parent respin:

1. Freeze current source inputs and hashes in
   `resources/image-construction/boot-layers/raspberry-pi-zero-2w.json` and
   `orange-pi-zero-2w.json`; cross-build the matching native binary first.
2. Run the reviewed Raspberry pi-gen and Orange Armbian constructors. Stage the
   canonical welcome, preserve declared hushlogin behavior, encode Raspberry's
   inactive-UART state, and install each board's declared runtime/initramfs
   inputs. Regenerate selected initramfs images only in the constructor.
3. Run mounted-image proof before board deployment. Raspberry must show the
   selected initramfs, enabled early writer, welcome, exact hushlogin,
   inactive-UART and serial-console state, and expected service masks. Orange
   must show the welcome, root-installed renderer/lifecycle/assets, fixed
   SPI/GPIO dependencies, static initramfs frame/Python closure, and one
   `/run/octessera-boot` owner with no second writer.
4. Preserve image, source hashes, selected boot outputs, and proof logs. Only
   then perform the physical loop in [`../open-work.md`](../open-work.md).

The v0.8.1 Raspberry and Orange constructor/source-bound evidence exists, but
physical FAT remains the gate for exact release artifacts.

## Phase 5 OLED boot layer

Run these source and contract checks before an image build:

```bash
cargo test -p octessera-pi sweep_
cargo test -p octessera-pi handoff
python3 tools/pi-image/test-boot-layer-contract.py
bash tools/pi-image/test-rpi-boot-services.sh
python3 tools/armbian-image/test_orange_oled_logo.py
python3 tools/armbian-image/test_orange_oled_handoff.py
python3 tools/armbian-image/test-orange-construction.py
```

The handoff checks prove the exclusive `/run/octessera-boot` lock, strict
status/stop files, release/adoption sequence, failure recovery, and no-clobber
behavior. Raspberry checks one static selected-initramfs frame; Orange checks
one static RGB565 frame and its Python closure. Unix-only lock coverage needs
Linux or WSL on Windows. These are source-contract checks, not live visual
qualification. The contract is
`resources/oled/boot-sweep-v1.json`; retain its 30-frame, 1,200,000,000 ns,
25-fps and panel-orientation requirements.

The clean logo+wordmark frame follows the sweep; continuous loops hold it for a
responsive 2,000,000,000 ns rest. The selected initramfs writers leave the
final systemd sweep and handoff unchanged. The conservative 16 MHz wire budget
is 491.625 ms for 30 frames (40.96875%), below the 80% limit; 58 frames pass
that limit and 59 fail it.

The native instrument-menu lifecycle is separate: PlaybackRuntime emits the
exact sleep/shutdown/reboot toasts, and the board runtime force-acknowledges the
final snapshot before preserving OLED state, zeroing LEDs, detaching, and
submitting ordinary power. Do not use arbitrary `systemctl` commands as proof.

Build both native binaries without deploying them as a constructor substitute:

```powershell
./tools/pi/build-pi-cross.ps1 -BoardProfile raspberry-pi-zero-2w -OutDir target/pi-cross-phase5
./tools/pi/build-pi-cross.ps1 -BoardProfile orange-pi-zero-2w -Backend wsl-docker -OutDir target/orange-pi-cross-phase5
```

Each output needs its adjacent metadata sidecar with the matching profile. This
does not prove initramfs contents, services, mounted-image layout, OLED handoff,
DAC health, or physical display behavior.

## Orange production and diagnostic image modes

Only `raspberry-pi-zero-2w` and `orange-pi-zero-2w` are supported cross-build
IDs. Raspberry selects `hardware-raspberry-pi-zero-2w`; Orange selects
`hardware-orange-pi-zero-2w`. The Orange image uses the locked
`octessera-runtime` account and `octessera.service`; every non-empty Jack/USB/
HDMI output set is valid, Jack is required only when selected, recognized
disconnected USB or HDMI may wait, selected route faults block readiness, and
no route is a fallback. Full Armbian, kernel, device-tree, and image replacement
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
lists. Do not enable an overlay on another board or kernel without a new review.

## Setup portal and mutation proof

The Raspberry setup source is
`tools/pi-image/stage4-octessera/files/root`; the Orange source is
`userpatches/overlay`. Exact source paths, digests, modes, preimages, stale
markers, and enabled-unit differences are recorded in
`resources/image-mutations/raspberry-pi-zero-2w-setup.json` and
`orange-pi-zero-2w-setup.json`. The setup layer is opt-in and permits only its
declared files, symlinks, and stale-marker removal; it does not mutate packages,
accounts, network, boot, or firmware. Missing parent packages, accounts,
executables, services, ownership, modes, xattrs, or preimages fail closed.

Run the targeted setup and security checks:

```bash
bash tools/armbian-image/test-setup-layer.sh
PYTHONDONTWRITEBYTECODE=1 python3 tools/armbian-image/test_setup_sidecar.py
PYTHONDONTWRITEBYTECODE=1 python3 tools/armbian-image/test-setup-request.py
PYTHONDONTWRITEBYTECODE=1 python3 tools/armbian-image/test-setup-http.py
PYTHONDONTWRITEBYTECODE=1 python3 tools/armbian-image/test-setup-flow.py
PYTHONDONTWRITEBYTECODE=1 python3 tools/armbian-image/test-setup-state.py
bash tools/armbian-image/validate.sh
python3 tools/image-respin/test_setup_contract.py
python3 tools/image-respin/test_trust_manifest.py
python3 tools/image-respin/verify-parent-release.py --manifest resources/image-parents/v0.7.5-trust-manifest.json --validate-manifest
python3 tools/image-respin/test_runtime_contract.py
python3 tools/image-respin/test_workflow_records.py
python3 tools/image-respin/test_workflow_static.py
node --check userpatches/overlay/usr/local/share/octessera-setup-ui/app.js
```

Root-required mutation and disk fixtures run in CI as
`sudo python3 tools/image-respin/test_setup_mutation.py` and
`sudo python3 -m unittest discover -s tools/image-respin -p 'test_disk_*.py'`.
The trusted v0.7.5 parent exercise is frozen legacy recovery, not v0.8.1
qualification. If explicitly required, run both board lanes:

```bash
gh workflow run respin-board-image.yml -f board=raspberry-pi-zero-2w -f setup_layer=setup-portal
gh workflow run respin-board-image.yml -f board=orange-pi-zero-2w -f setup_layer=setup-portal
```

For full production image, kernel, setup-portal, sample, sanitization, and
runtime-bundle contracts, see the [Orange production reference](../../hardware/docs/orange-pi-production-reference.md).
