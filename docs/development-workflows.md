# Contributor Development Workflows

This is a contributor reference. End-user hardware build, assembly, and bring-up docs have priority:

- `userdocs/hardware/assembly-manual.md`
- `userdocs/hardware/pinout-and-connections.md`
- `userdocs/hardware/enclosure.md`
- `hardware/docs/branding-assets.md`
- `docs/menu-and-controls-spec.md`

## Install

```bash
corepack pnpm install
```

Use pnpm workspaces. Do not use npm or yarn for this repository.

## Documentation Checks

Check local Markdown links:

```bash
python tools/docs/check_links.py
```

For a slower pass that also fetches HTTP links and validates known BOM product pages by content, run:

```bash
python tools/docs/check_links.py --http
```

Regenerate the printable user PDF after editing `userdocs/print/*.html`, `userdocs/print/*.svg`, or `userdocs/print/print.css`. The script prints the HTML sheet to PDF:

```powershell
./tools/docs/render_userdocs_pdf.ps1
```

## Desktop Development

```bash
corepack pnpm --filter @octessera/desktop tauri:dev
```

### Hardware-free verification matrix

These checks are useful before a board is available. They validate source,
desktop, and host-build behavior only; none is board qualification.

| Check                     | Command                                                                                          | Confirms                                                                                                                      | Does not confirm                                                               |
| ------------------------- | ------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| Documentation links       | `python tools/docs/check_links.py`                                                               | Local Markdown targets resolve                                                                                                | Release downloads or hardware paths                                            |
| Desktop contract          | `corepack pnpm --filter @octessera/desktop typecheck`                                            | Desktop TypeScript contracts compile                                                                                          | Physical input, display, or audio output                                       |
| Desktop lint              | `corepack pnpm --filter @octessera/desktop lint`                                                 | Desktop ESLint checks pass                                                                                                    | Runtime behavior or hardware integration                                       |
| Desktop format            | `corepack pnpm --filter @octessera/desktop format:check`                                         | Desktop Prettier checks pass                                                                                                  | Runtime behavior or hardware integration                                       |
| Desktop tests             | `corepack pnpm --filter @octessera/desktop test`                                                 | Simulator/runtime-facing test cases pass                                                                                      | Board timing, GPIO, DAC, or USB behavior                                       |
| Native host tests         | `cargo test -p platform-core -p playback-runtime -p realtime-engine`                             | Native behavior and rendering logic pass on the host                                                                          | A particular board, enclosure, power supply, or assembled control surface      |
| Orange-feature host tests | `cargo test -p octessera-pi --no-default-features --features hardware-orange-pi-zero-2w orange_` | Orange-specific host tests, including setup-portal lifecycle and DAC/audio-loss handling, pass without opening board hardware | Orange boot, GPIO, OLED, DAC, audio-device, or physical qualification behavior |
| Pi host-stub build        | `cargo build -p octessera-pi`                                                                    | The Pi application builds without hardware                                                                                    | Boot images, peripheral wiring, or physical qualification                      |

Keep the limits visible in reports: a clean hardware-free matrix is evidence
for software and documentation paths, not evidence that a board is ready.

## Desktop Builds

CI smoke build without bundling:

```bash
corepack pnpm --filter @octessera/desktop tauri:build:ci
```

Portable desktop executable:

```bash
corepack pnpm --filter @octessera/desktop tauri:build:exe
```

The portable executable is copied to `apps/desktop/dist-desktop/octessera.exe`.

The Tauri bundle uses its configured `bundle.resources` entry for the legal
resource tree. The release checks the configured resource contract and the
portable notice ZIP; it does not rely on extracting an installer to prove the
resource configuration.

On Windows, use the cached wrapper for the same portable build when iterating:

```powershell
./tools/desktop/desktop-exe-fast.ps1
```

Rebuild it after significant changes that affect desktop-visible behavior, native runtime behavior, audio behavior, config/default payloads, Tauri host integration, or runtime contracts. Do not rebuild it for Rust-only changes that are clearly internal and not desktop/runtime/audio observable, such as isolated tests, docs, formatting, refactors with no behavior change, or Pi/HAL-only work. When unsure whether a change is observable through the desktop app, rebuild the portable exe.

Release executable and NSIS installer:

```bash
corepack pnpm --filter @octessera/desktop tauri:build
```

Release outputs are written under `target/release/`.

## Explicit GitHub Releases

GitHub release assets are built only by `.github/workflows/release-artifacts.yml` for explicit releases. Tag pushes and intermediate CI builds must not publish release assets.

Release assets:

- `octessera-<version>-windows-installer.exe`: primary Windows installer.
- `octessera-<version>-windows-portable.zip`: portable Windows alternative with the legal notice bundle.
- `octessera-<version>-macos-unsigned.dmg`: unsigned macOS DMG.
- `octessera-<version>-ubuntu-amd64.deb`: Ubuntu/Debian package.
- `octessera-<version>-ubuntu-x86_64.AppImage`: portable Linux AppImage.
- `octessera-<version>-raspberry-pi-zero-2w.img.zip`: ready-to-flash Raspberry Pi Zero 2 W image, including `os_list.rpi-imager-manifest` for Raspberry Pi Imager.
- `octessera-<version>-raspberry-pi-zero-2w.rpi-imager-manifest`: standalone Raspberry Pi Imager manifest copy.
- `octessera-<version>-raspberry-pi-zero-2w-device-aarch64.zip`: Raspberry Pi profile-qualified Linux aarch64 device update payload.
- `SHA256SUMS-raspberry-pi-zero-2w-device.txt`: checksum for the Raspberry Pi device update payload.
- `octessera-<version>-orange-pi-zero-2w.img.xz`: Orange Pi production Armbian image.
- `octessera-<version>-orange-pi-zero-2w-standalone-manual-aarch64.zip`: Orange Pi production runtime bundle for manual installation.
- `SHA256SUMS-orange-pi-zero-2w.txt` and `SHA256SUMS-orange-pi-zero-2w-device.txt`: checksums for the Orange image and manual runtime bundle.
- `octessera-<version>-notices.zip`: release-level legal notice bundle.
- `SHA256SUMS-*.txt`: checksums for the other release assets.

The final publish gate expects exactly 28 release files. It checks the notice
bundle, portable notice proof, device ZIP legal files, image proofs, runtime
identity, and exact asset names/checksums.

Release process:

1. Bump versions in Rust manifests, `package.json` files, and `apps/desktop/src-tauri/tauri.conf.json`.
2. Run `corepack pnpm install` after package version edits.
3. Run local validation and rebuild the portable desktop exe if desktop-visible behavior changed.
4. Commit and push the release-prep changes.
5. Create a unique empty draft GitHub release such as `v0.5.0`.
6. Run `Release Artifacts` manually with that existing tag. The workflow derives
   the future semver from the tag and confirms it against the package metadata.
7. Confirm the installer, portable ZIP, macOS DMG, Ubuntu DEB/AppImage,
   Raspberry image and device assets, Orange production image and standalone
   runtime bundle, notices, kernel evidence, and checksum files are attached
   before announcing the release. Board device ZIPs carry exact root-level
   `LICENSE` and `NOTICE` files; the release-level `notices.zip` remains the
   full bundle.

The Pi and Orange image builds are necessary slow paths because they generate
full OS images through pi-gen and Armbian. Keep them release-only.

Before any future public board-image release, review the applicable source
duties for the pinned upstream inputs and the Octessera source, patches,
configuration, and build scripts; see [`release-licensing.md`](release-licensing.md).

Before a local Raspberry constructor run, use the canonical source checkout and
stage the notices into the disposable stage4 tree:

```bash
export OCTESSERA_REPOSITORY_ROOT="$PWD"
sudo python3 tools/legal/stage_notices.py \
  --repository-root "$OCTESSERA_REPOSITORY_ROOT" \
  --destination-root tools/pi-image/stage4-octessera/files/root
```

The Raspberry stage script requires that environment variable and verifies the
staged tree with `--check`. Remove the generated
`tools/pi-image/stage4-octessera/files/root/usr/share/doc/octessera/` tree after
the local constructor run; release workflows stage it only in a disposable
checkout and clean it before the job ends. Orange image staging follows the
same manifest-driven pattern with `OCTESSERA_REPOSITORY_ROOT`.

Both fixed Pi image paths also install the inactive Wi-Fi foundation:
`octessera-wifi-foundation` and `octessera-wifi-foundation.service`. It is a
root-owned, Wi-Fi-only wrapper around the pinned `wifi-connect` arm64 asset,
fixed to `wlan0` and `192.168.42.1`, with a bounded invocation. The service is
deliberately not enabled and has no systemd wants symlink. It does not add
menu/runtime behavior or serialize credentials.

The release Pi image must be sanitized: no WiFi credentials, SSH keys, GitHub tokens, host logs, or local user secrets. SSH is disabled by default.

Device updates use `/usr/local/sbin/octessera-update` on supported Raspberry Pi
images. Assets are profile-qualified: the Raspberry Pi updater fetches
`octessera-<version>-raspberry-pi-zero-2w-device-aarch64.zip` with
`SHA256SUMS-raspberry-pi-zero-2w-device.txt`, verifies the checksum and embedded
board-profile manifest, and stages an immutable candidate under
`/opt/octessera/releases/<version>`. Orange update check, apply, rollback, and
OTA remain unsupported in 0.7.5; Orange must not consume Raspberry assets or
pretend that a manual image install is an OTA update.

On supported Raspberry installations, Apply and rollback use a guarded
transaction. The guard verifies the candidate service restart, process identity,
and a stability window; downloaded Apply candidates also require a matching
package-version/profile readiness marker. If validation fails or times out, the
updater restores the previous current link and starts a verified fallback
automatically. It commits the candidate only after validation succeeds. `Apply`
may return `Update health validation scheduled.` before that commit; this is a
scheduling acknowledgement, not a claim that the update passed.

Legacy installations without board-profile metadata are not eligible for new online releases. Provision or update the OS bundle to install the current profile metadata and updater, or reflash the device. The legacy updater must not apply new releases.

## Armbian Image Workflow

`.github/workflows/armbian-image.yml` builds Armbian images through `armbian/build`. Validation-only runs may inspect the default ref, but every qualification image build requires a reviewed full 40-character Armbian commit SHA. The workflow keeps minimal repository permissions and secrets out of validation and public generic builds.

Local checks before pushing workflow or `userpatches/` changes:

```bash
bash -n userpatches/customize-image.sh tools/armbian-image/validate.sh
tools/armbian-image/validate.sh
```

Also run these if installed:

```bash
shellcheck userpatches/customize-image.sh tools/armbian-image/validate.sh
actionlint .github/workflows/armbian-image.yml
```

The Armbian image validation also requires `dtc`, `fdtoverlay`, and `fdtget` (all packaged as `device-tree-compiler`).

GitHub validation-only smoke test:

```bash
gh workflow run armbian-image.yml -f run_build=false -f artifact_mode=public-generic
# Optional public values are one compact JSON object:
gh workflow run armbian-image.yml -f 'public_inputs={"public_preset_configuration_url":"https://example.invalid/preset.conf"}'
```

Public generic builds may use board/release/kernel/UI/compression/extensions inputs and one compact public JSON input, for example `{"public_preset_configuration_url":"https://example.invalid/preset.conf","payload_url":"https://example.invalid/payload.tar","payload_sha256":"<64-hex-sha256>"}`. Do not add raw Wi-Fi, user, SSH, or private first-run values as workflow inputs.

Personalized builds must use `artifact_mode=private-personalized`, run from trusted `main` or tags, and pass protected environment approval for `armbian-image-personalized`. First-run and private payload URLs come from repository/environment secrets such as `ARMBIAN_PRESET_CONFIGURATION_URL`, `OCTESSERA_PRIVATE_PAYLOAD_URL`, and `OCTESSERA_PRIVATE_PAYLOAD_SHA256`. Private artifacts are retained briefly and must not be uploaded to releases.

## Platform Capabilities

Canonical source:

```text
resources/platform-capabilities.json
```

Regenerate TypeScript exports after editing it:

```bash
corepack pnpm run capabilities:generate
```

Check generated output freshness:

```bash
corepack pnpm run capabilities:check
```

Rust constants for `platform-core` and `realtime-engine` are generated at build time from the same JSON.

## Display Palette

`resources/display-palette.json` is the source of truth for the shared display/UI palette used by runtime LEDs, Pi rendering, desktop simulator rendering, printable docs, and shared TypeScript contracts. Do not add palette colors without deciding the product role first.

Generate palette exports after editing that source:

```bash
corepack pnpm run palette:generate
```

Generated outputs are checked in for TypeScript and CSS consumers; Rust constants are generated at build time through `platform-core`.

Check generated output freshness:

```bash
corepack pnpm run palette:check
```

## Heavy Runtime UI Scenario

The factory patch UI scenario is an ignored Rust test. It drives `NativeRunner` through protocol messages and simulated device input, not through private menu state.

Run it directly when changing menu traversal, runtime modulation, sampler assignment, Play, or factory-patch setup behavior:

```bash
cargo test -p playback-runtime factory_patch_ui_scenario -- --ignored
```

The pre-push hook runs this scenario. GitHub CI runs it on every push to `main` and on pull requests that change parity-sensitive native runtime inputs: `crates/platform-core/`, `crates/playback-runtime/`, shipped config JSON, menu help text, or platform capabilities. Other pull requests, and manual dispatches without `run-heavy-ui-scenario=true`, record an explicit successful skip.

The documented input recipe lives in [`factory-patch-ui-scenario.md`](factory-patch-ui-scenario.md).

## Platform Default Configs

Source defaults live under `config/defaults/`:

- `base.json`: shared default runtime payload.
- `desktop.json`: desktop overrides, including desktop brightness defaults.
- `pi.json`: Pi-family hardware overrides.

Generate platform runtime defaults after editing those sources:

```bash
corepack pnpm run config:generate
```

Generated outputs are checked in:

- `config/generated/desktop/default.json`
- `config/generated/pi/default.json`
- `config/default.json` as the current Pi-family default for existing tooling

Check generated output freshness:

```bash
corepack pnpm run config:check
```

Preset saves use portable patch envelopes in `presets/patches/<name>.json`. Hosts still load legacy `presets/<name>.json`; when both files exist for one logical preset name, the patch-directory file wins, and delete removes both. Default and recovery files remain full local snapshots until the device/default split protocol is introduced.

## Standard Verification

```bash
corepack pnpm run typecheck
corepack pnpm -r test
corepack pnpm -r lint
corepack pnpm -r format:check
corepack pnpm run quality:audit
cargo fmt --all --check
cargo test -p platform-core -p playback-runtime -p realtime-engine -p octessera-desktop
cargo clippy -p platform-core -p playback-runtime -p realtime-engine -p octessera-desktop --all-targets -- -D warnings
```

The root `typecheck` runs `config:check`, `capabilities:check`, and `palette:check` before package typechecks.

For menu/runtime-visible Rust changes on Windows, use the focused wrapper while iterating:

```powershell
./tools/quality/validate-menu-runtime.ps1 -IncludePi -BuildDesktopExe
```

Add `-IncludePlatformCore` when platform behavior changes and `-Typecheck` when shared contracts or TypeScript-visible payloads change.

The pre-push hook runs CI-like checks against the committed tree, including lint, typecheck, format checks, tests, coverage, file-length checks, desktop Rust adapter tests after the desktop check, Tauri build smoke, and clippy. It also runs the ignored factory patch UI scenario. Use a long timeout when pushing from automation. Do not skip the hook; fix failures and push again.

When committing and immediately pushing, run targeted confidence checks and required artifact builds before committing, then rely on the pre-push hook for the exhaustive CI-like suite. Avoid manually running a hook-equivalent full validation immediately before `git push` unless the change is high-risk, explicitly requested, or the hook cannot run.

For hardware branding and enclosure artifact changes, also check `hardware/docs/branding-assets.md` before committing. It documents the SVG source of truth, Pi PNG/initramfs path, PCB/enclosure branding conversions, and the cleanup checklist for stale generated artifacts and local absolute paths.

On Windows, use the cached Cargo wrapper while iterating. It enables `sccache` when installed, uses a local rustc-wrapper shim to strip Cargo's incremental env var before invoking `sccache`, and passes temporary profile overrides that disable incremental for that command so more crates can be cached. Without `sccache`, Cargo uses its normal `target/` cache:

```powershell
./tools/dev/cargo-fast.ps1 check -p octessera-pi
./tools/dev/cargo-fast.ps1 test -p playback-runtime
```

Install `sccache` once with:

```powershell
cargo install sccache --locked
```

## Pi Builds Without Hardware

See [`board-profiles.md`](board-profiles.md) for the canonical IDs, retained
Cargo aliases, and Raspberry/Orange image boundary.

Host-stub Pi app build:

```bash
cargo build -p octessera-pi
```

Hardware HAL target check when the Rust target is installed:

```bash
cargo check --target aarch64-unknown-linux-gnu -p octessera-hal --features raspberry-pi-zero-2w

# Compatibility alias retained during the profile transition.
cargo check --target aarch64-unknown-linux-gnu -p octessera-hal --features pi-zero
```

## Pi Hardware Build

Provision a development Pi, or refresh its tracked OS and boot configuration, with `./tools/pi/provision-pi.ps1`. This is separate from fast deployment and is safe to repeat. Pass `-UpdateInitramfs` when the early boot splash or its boot configuration needs to be refreshed; pass `-WakeTrace` only when enabling the development wake trace in the service configuration.

```powershell
./tools/pi/provision-pi.ps1 -Target pi@192.168.0.211 -BoardProfile raspberry-pi-zero-2w
```

Preferred fast path: run `./tools/pi/build-pi-cross.ps1` to produce a Linux ARM binary, then upload a Raspberry build with `./tools/pi/deploy-pi-fast.ps1 -LocalBinary target/pi-cross/octessera-pi -NoTail`. The cross-builder accepts exactly `raspberry-pi-zero-2w` (the default) and `orange-pi-zero-2w`, selecting the matching Cargo feature and adjacent metadata sidecar. The deployment helper only transfers Raspberry binary/source content, restarts the configured service, and optionally tails its logs. On Windows, the build helper uses WSL2 Docker automatically when available. Native cross-builds are still supported with an ARM Linux sysroot and cross `pkg-config` setup for ALSA.

```powershell
./tools/pi/build-pi-cross.ps1
./tools/pi/deploy-pi-fast.ps1 -Target pi@192.168.0.211 -LocalBinary target/pi-cross/octessera-pi -NoTail
# The adjacent target/pi-cross/octessera-pi.metadata.json is checked during deployment.
```

On a Pi or properly configured cross environment:

```bash
cargo build -p octessera-pi --features hardware-raspberry-pi-zero-2w
```

Low-resource on-Pi fallback:

```bash
CARGO_BUILD_JOBS=1 cargo build --profile pi-dev -p octessera-pi --features hardware-raspberry-pi-zero-2w
```

## Phase 5 OLED Boot Layer

The current source implements one boot-sweep and handoff contract for both
fixed boards. Run the fast source and contract checks before spending time on
an image build:

```bash
cargo test -p octessera-pi sweep_
cargo test -p octessera-pi handoff
python3 tools/pi-image/test-boot-layer-contract.py
bash tools/pi-image/test-rpi-boot-splash.sh
bash tools/pi-image/test-rpi-boot-services.sh
bash tools/armbian-image/test-orange-boot-splash-hook.sh
python3 tools/armbian-image/test_orange_oled_logo.py
python3 tools/armbian-image/test_orange_oled_handoff.py
python3 tools/armbian-image/test-orange-construction.py
```

The handoff tests exercise the exclusive `/run/octessera-boot` lock, strict
status/stop files, initramfs marker, release/adoption sequence, failure
recovery, and no-clobber behavior. Run the Unix-only lock coverage in Linux or
WSL when the host is Windows. The visual contract is
`resources/oled/boot-sweep-v1.json`; source tests must continue to prove its
24-frame, one-second, four-band, white-only, +8 px lean behavior.

Build both native binaries without deploying them as a constructor substitute:

```powershell
./tools/pi/build-pi-cross.ps1 -BoardProfile raspberry-pi-zero-2w -OutDir target/pi-cross-phase5
./tools/pi/build-pi-cross.ps1 -BoardProfile orange-pi-zero-2w -Backend wsl-docker -OutDir target/orange-pi-cross-phase5
```

Each output must retain its adjacent `octessera-pi.metadata.json` sidecar with
the matching board profile. This is a source/build check only; it does not
prove initramfs contents, service ownership, mounted-image layout, OLED
handoff, DAC health, or physical display behavior.

No full constructor or production image has been built for this boot layer yet.

### Later constructor procedure

When the Phase 5 image work is authorized, construct each board from its
source-bound boot-layer contract, not from a trusted parent respin:

1. Freeze the current source inputs and hashes in
   `resources/image-construction/boot-layers/raspberry-pi-zero-2w.json` and
   `orange-pi-zero-2w.json`; cross-build the matching native binary first.
2. Run the reviewed Raspberry pi-gen constructor and the reviewed Orange
   Armbian constructor. Stage the canonical interactive welcome, preserve the
   declared hushlogin behavior, and encode Raspberry's inactive-UART safety
   state before boot finalization. Install the boot service, hook, and runtime
   inputs; regenerate the selected initramfs on Raspberry and both the
   initramfs and Python closure on Orange. Do not use the runtime-only or
   setup-only `v0.7.5` parent respin as a boot-layer build.
3. Run mounted-image proof before any board deployment. Raspberry must show one
   exact selected initramfs, one enabled early writer, the canonical welcome,
   exact pi hushlogin, inactive Raspberry UART configuration, serial-console
   absence, and the expected serial-getty/Bluetooth service masks. Orange must show the
   canonical welcome, exact selected initramfs, fixed SPI/GPIO dependencies,
   the complete Python closure, and no broad GPIO probe. Verify the installed
   `/run/octessera-boot` ownership/status/lock paths and no second writer.
4. Preserve the resulting image, source hashes, selected boot outputs, and
   proof logs as constructor evidence. Only then perform the physical loop in
   `docs/open-work.md`; source tests, a cross-build, or a parent respin do not
   count as an image or hardware qualification.

## Orange Pi Armbian Image

`raspberry-pi-zero-2w` and `orange-pi-zero-2w` are the only supported cross-build board profile IDs. The Raspberry default selects `hardware-raspberry-pi-zero-2w`; an Orange runtime binary can be built without contacting a board:

```powershell
./tools/pi/build-pi-cross.ps1 -BoardProfile orange-pi-zero-2w -Backend wsl-docker
```

That output declares `hardware-orange-pi-zero-2w` in its adjacent metadata sidecar. Raspberry deploy, provision, preflight, and pi-gen image tooling accepts only the Raspberry profile. The Orange production image uses the Armbian path and ships the native runtime as `octessera.service` under the locked `octessera-runtime` account. It supports the OLED, NeoTrellis, NeoKey, four encoders, persistent store, samples, MIDI, and the exact internal DAC at the shared 44.1 kHz rate. USB-only audio is rejected; `audioOut=both` may add UAC2, but `audioOut=usb` is unsupported. Readiness follows healthy audio, initialized control-surface devices, and the first rendered snapshot. FIFO priority 70 is granted through `LimitRTPRIO=70`; no `CAP_SYS_NICE` or ambient capability is added. Orange update check/apply/rollback and OTA remain unsupported.

For a local Orange Pi cross-build, use the WSL Docker-only builder. It never
contacts or deploys to a board, installs the aarch64 GNU linker/sysroot in its
ephemeral tool container, and writes checked artifacts under
`target/orange-pi-cross/`:

```powershell
./tools/orange-pi/build-orange-cross.ps1 -Binary orange-oled-smoke -Profile release
./tools/orange-pi/build-orange-cross.ps1 -Binary octessera-pi -Profile release
./tools/orange-pi/test-build-orange-cross.ps1
```

Cargo and rustup caches use persistent named Docker volumes; `-DryRun` prints
the command without starting Docker. The builder accepts the two diagnostic
smoke binaries and a local Orange `octessera-pi` development binary. It does not
produce the 0.7.5 production image or its `production-runtime` bundle; that
bundle is built and hash-checked by the release workflow. No artifact is run
against the board by this helper.

The shared `build-armbian-image` action has an explicit `image_kind` input:
`diagnostic` builds the separate bring-up image, while `production` requires the
hash-bound Orange runtime bundle. The generic `Armbian Image` workflow uses
diagnostic mode; the 0.7.5 release workflow invokes the action in production
mode and builds `octessera-0.7.5-orange-pi-zero-2w.img.xz`. Diagnostic mode does
not contain or enable `octessera.service`. Run validation first:

The image path compiles and merges the separate
`octessera-h618-input-routing` overlay against the exact boot-selected H618
DTB. It clears `console=ttyS0`, masks `serial-getty@ttyS0.service`, clears the
UART0 stdout path, and releases PH0/PH1 without changing SSH. For an existing
board, the checked no-reboot provision/deploy wrapper is:

```powershell
.\tools\orange-pi\provision-input-routing.ps1 -Preflight
.\tools\orange-pi\provision-input-routing.ps1 -Apply
# after an operator-approved reboot, rerun -Preflight
.\tools\orange-pi\provision-input-routing.ps1 -RollbackId <backup-id>
```

Apply records the exact DTB/overlay hashes, prior boot files, and serial-getty
state under `/var/lib/octessera/input-routing-backups/<id>/`. It never reboots;
rollback is explicit and also leaves reboot to the operator.

Connected-hardware deepwork loops require fresh, explicit operator authorization
immediately before every stateful board action: package or boot changes,
service changes, reboot, deployment, GPIO/I2C/SPI activity, audio playback, and
USB gadget bind/unbind. SSH reachability, an earlier approval, and a successful
read-only probe do not authorize the next state-changing action.

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

Run the full public build by changing `run_build=true` and replacing `armbian_build_ref=main` with the reviewed full 40-character Armbian commit SHA. Public builds must stay secret-free. The first-boot setup portal handles Wi-Fi and SSH on the device.

Local validation before pushing image changes:

```bash
bash tools/armbian-image/validate.sh
python3 tools/pi-image/test-board-profile.py
node --check userpatches/overlay/usr/local/share/octessera-setup-ui/app.js
git diff --check
```

The workflow inspects the artifact before upload. For manual checks, inspect the artifact for expected setup files and no Octessera-added SSH material: no `octessera` user password, no `/etc/ssh/ssh_host_*`, and `ssh.service` masked until setup finalizes. Do not treat Armbian's own first-run root/bootstrap material as an Octessera secret.

If you have an extracted root filesystem directory or ext4 root partition image, run:

```bash
tools/armbian-image/inspect-built-image.sh <rootfs-dir-or-ext4-image>
```

### Full setup portal image layer

The setup portal is a separate, source-bound image layer. The Raspberry Pi source
tree is `tools/pi-image/stage4-octessera/files/root`, with profile
`raspberry-pi-zero-2w`. The Orange Pi source tree is `userpatches/overlay`, with
profile `orange-pi-zero-2w`. Each tree carries the same functional assets:
`etc/octessera/setup-profile`, `octessera-wifi-connect`,
`octessera-setup-sidecar`, the request/start/cleanup helpers, the setup status
helpers, the three setup systemd units, and the static files under
`usr/local/share/octessera-setup-ui/`. Exact source paths, digests, modes,
preimages, stale markers, and enabled-unit differences are recorded in
`resources/image-mutations/raspberry-pi-zero-2w-setup.json` and
`resources/image-mutations/orange-pi-zero-2w-setup.json`.

Setup mutation contracts and runtime-only contracts are separate. The ordinary
`raspberry-pi-zero-2w.json` and `orange-pi-zero-2w.json` contracts describe
runtime respins. `setup-portal` is opt-in and uses `disk_setup_respin.py` with
the matching `*-setup.json` contract; `runtime-only` remains the default and
uses `disk_respin.py`. The setup layer permits only its declared files, symlinks,
and stale-marker removal. It does not mutate packages, accounts, network, boot,
or firmware.

The setup constructor requires the parent to already contain `openssh-server`,
`network-manager`, `dnsmasq`, `python3-minimal`, `/usr/local/bin/wifi-connect`,
`/usr/bin/python3`, and the `ssh.service`, `NetworkManager.service`, and
`dnsmasq.service` units. Raspberry also requires the `pi:pi` account at
`/home/pi` with `/bin/bash`; Orange requires `octessera:octessera` at
`/home/octessera` with `/bin/bash` and `octessera-runtime:octessera-runtime` at
`/nonexistent` with `/usr/sbin/nologin`. Missing or mismatched package,
account, executable, service, preimage, ownership, mode, or xattr data is
constructor-required and fails closed; the setup layer does not create it.

Targeted setup and security/static checks are:

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

The root-required mutation and disk fixtures run in CI as
`sudo python3 tools/image-respin/test_setup_mutation.py` and
`sudo python3 -m unittest discover -s tools/image-respin -p 'test_disk_*.py'`.

The exact v0.7.5 parent exercise is still deferred. When authorized, run the
manual `Respin board image` workflow once for each board:

```bash
gh workflow run respin-board-image.yml -f board=raspberry-pi-zero-2w -f setup_layer=setup-portal
gh workflow run respin-board-image.yml -f board=orange-pi-zero-2w -f setup_layer=setup-portal
```

That lane validates `resources/image-parents/v0.7.5-trust-manifest.json`, fetches
the exact `v0.7.5` parent assets, and records setup-layer proof. No production
parent image has been exercised here.

Pi UI/render responsiveness profiling is quiet by default. Enable periodic summaries with either control:

```bash
OCTESSERA_PI_UI_PROFILE=1 octessera-pi
octessera-pi --profile-ui
```

Summaries include loop cadence, runtime tick lateness/advance, render overruns, snapshot/config sync, hardware polling, and LED/NeoKey/OLED phase timings.

### Pi Timing And Audio Stability Probes

Use Pi-side probes for rhythmic timing, trigger latency, audio-drain latency, and DSP budget questions. PC/runtime-only probes are useful for quick plausibility checks, but they cannot prove hardware audio timing.

Wrapper examples:

```powershell
# Safe default: runtime-only, does not stop the service or open live audio.
./tools/pi/run-pi-timing-probes.ps1 -Mode RuntimeOnly -Durations 15s -Scenarios idle,pulses-stress

# Optional live-audio probe. Use when subjective timing/audio behavior is unclear.
./tools/pi/run-pi-timing-probes.ps1 -Mode Live -Durations 10m -Scenarios idle

# Optional audio-source drain latency probe.
./tools/pi/run-pi-timing-probes.ps1 -Mode AudioDrain -Durations 10m

# Focused FX budget profile.
./tools/pi/run-pi-timing-probes.ps1 -Mode DspFxLimits

# Same profile with explicit current high-headroom Pi settings.
./tools/pi/run-pi-timing-probes.ps1 -Mode DspFxLimits -SynthSlotWorkers 2 -AudioBlockFrames 256
```

The wrapper stops `octessera.service` for live/audio/DSP modes and restarts it after the probe. Runtime-only mode leaves the service running. Use `-PrintOnly` to inspect the remote command first.

For musical timing issues, inspect p99/p99.9/p99.99 and outlier counts, not only p95. Check recent logs after live probes:

```powershell
ssh -i "$env:USERPROFILE\.ssh\octessera_pi_dev" -o IdentitiesOnly=yes pi@192.168.0.211 "journalctl -u octessera.service --since '10 minutes ago' --no-pager | grep -E 'audio callback RT promotion not qualified|audio stream error|underrun|POLLERR' || true"
```

Prefer offering a live probe when the report is subjective or audio-path-specific. Do not run long live probes by default for unrelated code changes.

## Pi Hardware Runtime Debug Loop

Use the real hardware loop for Pi-only behavior, input latency, OLED rendering, LEDs, encoders, menu timing, sample playback, and audio stutter. Automated checks cannot prove tactile timing or display readability.

1. For a new Pi or OS/configuration change, provision first; then cross-build and deploy from the PC:

   ```powershell
   ./tools/pi/provision-pi.ps1 -Target pi@192.168.0.211
   ./tools/pi/build-pi-cross.ps1
   ./tools/pi/deploy-pi-fast.ps1 -Target pi@192.168.0.211 -LocalBinary target/pi-cross/octessera-pi -NoTail
   ```

2. Ask for a focused hardware observation before assuming the fix worked. Specify the control path, expected behavior, and what failure would look or sound like.
3. Pull service logs and profile summaries when the observation is unclear. Enable `OCTESSERA_PI_UI_PROFILE=1` only while profiling, then disable it again.
4. For menu, control, or runtime stutter, inspect `pi-ui-profile` and `menu-key-profile` output before broad refactors.
5. Fix the source path. Do not add fallbacks for broken octessera wiring; fail visibly so missing handlers, stale config paths, or bridge mismatches get fixed at the source.
6. Prefer keyed fast paths over broad `apply_menu_state()` on high-frequency edits. Keep autosave serialization off rapid input paths.
7. Run targeted Rust checks before redeploying when possible, then repeat the hardware observation.
8. Before pushing a stable hardware milestone, use QA or oracle review for risky runtime/menu changes and run the pre-push hook. It validates the committed tree and includes file-length checks, coverage, Tauri smoke build, and clippy.

## Documentation Checks

After changing docs or menu/help resources:

```bash
cargo test -p playback-runtime
git diff --check
```

Search for obsolete completed-work history before committing documentation updates.

## Menu/Control Playback-Priority Changes

Menu and control changes can affect playback timing. Use this checklist when changing `crates/playback-runtime` menu apply paths, desktop/Pi runtime loops, or audio config/control routing:

1. Prefer key-specific fast paths over broad `apply_menu_state()` for high-frequency edit paths.
2. Keep dynamic parameters immediate and bounded.
3. For structural selectors, avoid full config/audio rebuilds unless the selected structure actually changed.
4. Delay autosave payload generation for rapid edits; explicit Save Default remains immediate.
5. Preserve hardware parity by implementing behavior in `playback-runtime` or `platform-core`, not desktop TypeScript.
6. Update `docs/menu-and-controls-spec.md` for parity-affecting control/menu behavior.
7. Run targeted playback-runtime tests first, then full `cargo test -p playback-runtime` before commit.
8. Rebuild `apps/desktop/dist-desktop/octessera.exe` when the change is desktop-visible.
