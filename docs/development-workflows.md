# Contributor Development Workflows

This is a contributor reference. Start user-facing work at `userdocs/README.md`;
the user-facing hardware build, assembly, and bring-up docs have priority:

- `userdocs/hardware/assembly-manual.md`
- `userdocs/hardware/pinout-and-connections.md`
- `userdocs/hardware/enclosure.md`
- `docs/menu-and-controls-spec.md`

Release owners should start with the [user-facing release support matrix](../userdocs/release-support.md).
It is the single owner checklist for support status, manual FAT evidence, USB
policy, and the final human publish decision.

Contributor-only branding guidance lives in `hardware/docs/branding-assets.md`.

## Install

```bash
corepack pnpm install
```

Use pnpm workspaces. Do not use npm or yarn for this repository.

## Documentation Checks

General documentation checks:

```bash
python tools/docs/check_links.py
python3 tools/docs/test_release_documentation.py
git diff --check
```

Markdown-only edits do not require a Rust test run. Edits to
`resources/menu-help-texts.tsv` or native menu/help targets also require the
focused parity check:

```bash
cargo test -p playback-runtime
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
| Orange-feature host tests | `cargo test -p octessera-pi --no-default-features --features hardware-orange-pi-zero-2w orange_` | Orange-specific host tests, including setup-portal lifecycle and selected-route audio loss/recovery handling, pass without opening board hardware | Orange boot, GPIO, OLED, audio-device, or physical qualification behavior |
| Pi host-stub build        | `cargo build -p octessera-pi`                                                                    | The Pi application builds without hardware                                                                                    | Boot images, peripheral wiring, or physical qualification                      |

Keep the limits visible in reports: a clean hardware-free matrix is evidence
for software and documentation paths, not evidence that a board is ready.
The desktop lint and format rows run real ESLint and Prettier checks. The root
recursive lint and format commands also visit packages whose scripts are no-ops,
so those aggregate commands do not prove uniform coverage across every package.

## Orange live audio benchmark tooling

Phase 2 host tooling extends the fixed-target Orange capability runner; it does
not add another SSH transport. The single-cell mode requires a reviewed
artifact and metadata sidecar, explicit interruption consent, and one approved
scenario/configuration. It validates the production readiness identity, exact
DAC ALSA `buffer_size`/`period_size`, release identity, schema-2 callback
geometry, thermal/memory safety, and restoration before classifying the result.

Preview the deterministic 29-cell order without transport or board access:

```powershell
./tools/orange-pi/run-orange-live-audio-matrix.ps1 -PrintOnly
```

The order is A (11 cells), the selected A 120-second repeat, B (11 cells), then
C0, C2, and C3 (two cells each). CPAL callback batches are variable positive
counts no larger than the requested ALSA buffer; render ratios use each actual
callback size, while spacing lateness uses the fixed ALSA period. Active
execution requires the separate `-AllowMatrixServiceInterruption` switch and
the per-cell consent supplied by the matrix runner. Phase 2 validation is
host-only; do not cross-build,
deploy, or run this matrix as part of contributor checks.

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
- `octessera-<version>-ubuntu-amd64.deb`: Ubuntu/Debian package.
- `octessera-<version>-ubuntu-x86_64.AppImage`: portable Linux AppImage.
- `octessera-<version>-raspberry-pi-zero-2w.img.zip`: ready-to-flash Raspberry Pi Zero 2 W image, including `os_list.rpi-imager-manifest` for Raspberry Pi Imager.
- `octessera-<version>-raspberry-pi-zero-2w.rpi-imager-manifest`: operational standalone Raspberry Pi Imager manifest copy.
- `octessera-<version>-raspberry-pi-zero-2w-device-aarch64.zip`: Raspberry Pi profile-qualified updater payload containing exactly `octessera-pi`, `octessera-device-release.json`, `LICENSE`, and `NOTICE`; the binary entry is executable.
- `SHA256SUMS-raspberry-pi-zero-2w-device.txt`: legacy one-line checksum retained only for existing installed Raspberry updater clients.
- `octessera-<version>-orange-pi-zero-2w.img.xz`: Orange Pi production Armbian image.
- `octessera-<version>-orange-pi-zero-2w-standalone-manual-aarch64.zip`: Orange Pi production runtime bundle for manual installation containing `octessera-pi`, `octessera-runtime.json`, `SHA256SUMS`, `octessera-device-release.json`, `LICENSE`, and `NOTICE`. It remains a manual bundle and is not an OTA asset.
- `octessera-<version>-orange-pi-zero-2w-runtime-updater-aarch64.zip`: Orange Pi profile-qualified runtime-only updater payload containing exactly `octessera-pi`, `octessera-device-release.json`, `LICENSE`, and `NOTICE`.
- `SHA256SUMS-orange-pi-zero-2w-runtime-updater.txt`: checksum for the exact Orange runtime-updater ZIP.
- `octessera-<version>-release-evidence.zip`: supporting build material, including the checksums, kernel packages, image evidence, operational manifest copy, and legal bundle that are not root assets.
- `SHA256SUMS.txt`: lowercase, sorted checksums for the other 13 custom root assets.

macOS distribution is paused until it can be properly signed and notarized, so
it is not currently a GitHub release asset. The final populated-draft gate expects
exactly 14 custom release files. It checks the
portable notice proof, exact four-entry Raspberry updater ZIP, exact six-entry
Orange manual ZIP, exact four-entry Orange runtime-updater ZIP, image and kernel
evidence, runtime identity, and root asset names/checksums. GitHub's automatic source ZIP and tar archives remain visible
source archives but are not custom assets or entries in `SHA256SUMS.txt`.

Release process and owner handoff:

1. Bump versions in Rust manifests, `package.json` files, and `apps/desktop/src-tauri/tauri.conf.json`.
2. Run `corepack pnpm install` after package version edits.
3. Run local validation and rebuild the portable desktop exe if desktop-visible behavior changed.
4. Commit and push the release-prep changes.
5. Create a unique empty draft GitHub release such as `v0.5.0` as the workflow
   input. The release workflow must end with the exact assets attached to that
   release while it remains a populated draft.
6. Run `Release Artifacts` manually with that existing tag. The workflow derives
   the semver from the tag and confirms it against the package metadata; it does
   not replace the release-owner review in [release support](../userdocs/release-support.md).
7. Stop at the populated draft. Use the release-owner checklist to verify exact
   asset names/count, checksums, image manifests, ZIP contents, sample/default
   coverage, desktop launch, per-board FAT, source duties, and limitations. Do
   not announce or publish until a human explicitly makes that decision.

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
`/opt/octessera/releases/<version>`. Orange Check/Apply/Rollback goes through the
root-owned update broker and guarded updater, which accepts only the Orange
profile's `octessera-<version>-orange-pi-zero-2w-runtime-updater-aarch64.zip`
and `SHA256SUMS-orange-pi-zero-2w-runtime-updater.txt` pair. It updates only the
managed runtime release and binary link. Full Armbian, kernel, device-tree, and
image replacement remains manual; the standalone manual ZIP is not an OTA asset.
Missing or mismatched profile, asset, manifest, checksum, or health precondition
fails closed. Orange never consumes Raspberry assets or falls back to the manual
ZIP or a full image path.

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

Generated outputs are checked in for TypeScript, CSS, and Rust consumers. The `platform-core` build script copies the tracked Rust output into `OUT_DIR` for the existing `palette.rs` include.

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
- `desktop.json`: desktop device-local brightness overrides.
- `pi.json`: Pi-family device-local brightness overrides.

The generator allowlist is deliberately narrow: platform files may override only
`runtimeConfig.buttonBrightness`, `runtimeConfig.displayBrightness`, and
`runtimeConfig.gridBrightness`, each as an integer from 0 through 100. Musical
runtime, mapping, sample, instrument, layer, FX, and aux data belongs in the
shared base or a portable patch, not in a platform override.

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

Preset saves use schema-versioned `octessera.patch` envelopes in
`presets/patches/<name>.json`. The envelope carries musical patch state,
including sampler paths, while device-local settings and device/system aux
bindings stay with the host; musical aux bindings travel with the patch. Hosts
still load legacy `presets/<name>.json`; when both files exist for one logical
preset name, the patch-directory file wins, and delete removes both. The
portable evidence contract is patch-projection equality plus verified
sample-path loadability; it does not claim full device-config equality or
physical-board qualification.
Default, recovery, backup, and confirmed device-apply payloads remain full
local configs.

## Focused Verification

Use package-scoped and crate-scoped checks while iterating:

```bash
corepack pnpm --filter @octessera/desktop typecheck
corepack pnpm --filter @octessera/desktop lint
corepack pnpm --filter @octessera/desktop format:check
corepack pnpm --filter @octessera/desktop test
cargo fmt --all --check
cargo test -p platform-core -p playback-runtime -p realtime-engine -p octessera-desktop
cargo clippy -p platform-core -p playback-runtime -p realtime-engine -p octessera-desktop --all-targets -- -D warnings
```

These are fast or focused confidence checks, not the full workspace gate. The
desktop lint and format commands run real ESLint and Prettier; the recursive
workspace commands also visit packages with no-op lint/format scripts.

## Full Local and CI Verification

`.githooks/pre-push` delegates to `tools/quality/pre-push.sh`. The fast profile
is explicit and skips Cargo tests/builds:

```bash
./tools/quality/pre-push.sh --fast
```

The default profile is the full local gate and expects a clean worktree:

```bash
./tools/quality/pre-push.sh
```

It runs the root `lint`, `typecheck`, `format:check`, `test`, and
`test:coverage` scripts, Cargo formatting, file-length checks, workspace Rust
tests and coverage, the ignored factory-patch scenario, desktop/Pi checks, the
Tauri build smoke test, and clippy for the checked workspace crates. Its
workspace Cargo test and clippy selections exclude `rodio-engine-source`.
GitHub CI separately runs that crate's tests and clippy; the current Rust
coverage script covers `platform-core`, `playback-runtime`, and `realtime-engine`
only. CI otherwise splits the same broad coverage across TypeScript
lint/format/typecheck/tests, Rust format/lint/tests, coverage, and the
conditional factory-patch scenario.

The quality audit is an additional local source-structure check:

```bash
corepack pnpm run quality:audit
```

The root `typecheck` runs `config:check`, `capabilities:check`, and `palette:check` before package typechecks.

For menu/runtime-visible Rust changes on Windows, use the focused wrapper while iterating:

```powershell
./tools/quality/validate-menu-runtime.ps1 -IncludePi -BuildDesktopExe
```

Add `-IncludePlatformCore` when platform behavior changes and `-Typecheck` when shared contracts or TypeScript-visible payloads change.

The pre-push hook runs CI-like checks against the committed tree, including lint, typecheck, format checks, tests, coverage, file-length checks, desktop Rust adapter tests after the desktop check, Tauri build smoke, and clippy. It also runs the ignored factory patch UI scenario. Use a long timeout when pushing from automation. Do not skip the hook; fix failures and push again.

When committing and immediately pushing, run targeted confidence checks and required artifact builds before committing, then rely on the pre-push hook for the full CI-like suite. Avoid manually running a hook-equivalent full validation immediately before `git push` unless the change is high-risk, explicitly requested, or the hook cannot run.

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

See [`board-profiles.md`](board-profiles.md) for the canonical IDs, canonical
Cargo feature owners, deprecated compatibility aliases, and Raspberry/Orange
image boundary.

Host-stub Pi app build:

```bash
cargo build -p octessera-pi
```

Hardware HAL target check when the Rust target is installed:

```bash
cargo check --target aarch64-unknown-linux-gnu -p octessera-hal --features raspberry-pi-zero-2w

# Deprecated compatibility alias; accepted for existing Cargo commands.
cargo check --target aarch64-unknown-linux-gnu -p octessera-hal --features pi-zero
```

## Pi Hardware Build

Provision a development Pi, or refresh its tracked OS and boot configuration, with `./tools/pi/provision-pi.ps1`. This is separate from fast deployment and is safe to repeat. Initramfs refresh is opt-in via `-UpdateInitramfs`; the default path removes retired Raspberry animation inputs without rebuilding the selected image, while the explicit path installs the current static hook/script before rebuilding it. Pass `-WakeTrace` only when enabling the development wake trace in the service configuration.

```powershell
./tools/pi/provision-pi.ps1 -Target pi@192.168.0.211 -BoardProfile raspberry-pi-zero-2w
```

Preferred fast path: run `./tools/pi/build-pi-cross.ps1` to produce a Linux ARM binary, then upload a Raspberry build with `./tools/pi/deploy-pi-fast.ps1 -LocalBinary target/pi-cross/octessera-pi -NoTail`. The cross-builder accepts exactly `raspberry-pi-zero-2w` (the default) and `orange-pi-zero-2w`, selecting the matching Cargo feature and adjacent metadata sidecar. The deployment helper only transfers Raspberry binary/source content, restarts the configured service, and optionally tails its logs. On Windows, the build helper uses WSL2 Docker automatically when available. Native cross-builds are still supported with an ARM Linux sysroot and cross `pkg-config` setup for ALSA.

```powershell
./tools/pi/build-pi-cross.ps1
./tools/pi/deploy-pi-fast.ps1 -Target pi@192.168.0.211 -LocalBinary target/pi-cross/octessera-pi -NoTail
# The adjacent target/pi-cross/octessera-pi.metadata.json is checked during deployment.
# If the boot-splash binary or assets changed, run provision-pi.ps1 first; this fast path never rebuilds initramfs.
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
bash tools/pi-image/test-rpi-boot-services.sh
python3 tools/armbian-image/test_orange_oled_logo.py
python3 tools/armbian-image/test_orange_oled_handoff.py
python3 tools/armbian-image/test-orange-construction.py
```

The handoff tests exercise the exclusive `/run/octessera-boot` lock, strict
status/stop files, release/adoption sequence, failure recovery, and no-clobber
behavior. Raspberry's selected initramfs is checked for one static frame path
and its command closure; Orange's selected initramfs is checked for one static
RGB565 frame and its Python closure. Run the Unix-only lock
coverage in Linux or
WSL when the host is Windows. The visual contract is
`resources/oled/boot-sweep-v1.json`; source tests must continue to prove its
30-frame, 1,200,000,000 ns absolute-deadline sweep at 25 fps, decreasing
mounted-controller X motion that appears left-to-right on the physical panel,
and a panel-facing right slash. Canonical bottom-to-top coordinates use
`slanted_origin = bottom_origin - row_y`, with four 8 px color bands and 4 px
white-separator behavior.
The clean logo+wordmark frame follows the sweep; continuous loops hold it for a
responsive 2,000,000,000 ns rest. The initramfs writers leave the final systemd
sweep and handoff unchanged. These are source-contract checks, not live visual
qualification. The contract also checks the conservative 16 MHz wire budget:
30 frames use 491.625 ms (40.96875%), leaving 39.03125 percentage points to the
80% limit; 58 frames pass the limit and 59 frames fail it.

The native instrument-menu lifecycle is a separate bounded path: PlaybackRuntime
emits the exact `Going to sleep`, `Shutting down`, and `Rebooting` toasts, while
the board runtime force-acknowledges the final snapshot before preserving OLED
pixels/on state, zeroing LEDs, detaching, and submitting ordinary power. Do not
use arbitrary administrative `systemctl` commands as evidence for this path.

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
   state before boot finalization. Install Raspberry's root systemd animator and
   runtime inputs; install Orange's root service, lifecycle helpers, and RGB565
   assets, and its static initramfs closure. Regenerate both selected initramfs
   images only as part of the constructor. Do not use the runtime-only or
   setup-only `v0.7.5` parent respin as a boot-layer build.
3. Run mounted-image proof before any board deployment. Raspberry must show one
   exact selected initramfs, one enabled early writer, the canonical welcome,
   exact pi hushlogin, inactive Raspberry UART configuration, serial-console
   absence, and the expected serial-getty/Bluetooth service masks. Orange must show the
   canonical welcome, root-installed renderer/lifecycle/assets, fixed SPI/GPIO
   dependencies, static Orange initramfs frame, and Python closure. Verify the installed
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

That output declares `hardware-orange-pi-zero-2w` in its adjacent metadata sidecar. Raspberry deploy, provision, preflight, and pi-gen image tooling accepts only the Raspberry profile. The Orange production image uses the Armbian path and ships the native runtime as `octessera.service` under the locked `octessera-runtime` account. Every non-empty Jack/USB/HDMI output set is valid; Jack is fatal/required only when selected, recognized disconnected USB or HDMI may wait, selected route faults block readiness, and no route is a fallback. Simultaneous physical outputs use independent unsynchronized clocks and can drift or echo; this phase does not provide sample alignment. Readiness follows selected-route status, initialized control-surface devices, and the first rendered snapshot. FIFO priority 70 is granted through `LimitRTPRIO=70`; no `CAP_SYS_NICE` or ambient capability is added. The observed Orange HDMI connector path is `/sys/class/drm/card0-HDMI-A-1`. On the live Raspberry Pi Zero 2 W, kernel `6.12.93+rpt-rpi-v8` exposes the exact `/sys/class/drm/card0-HDMI-A-1/{status,edid}` paths; Raspberry code pins card0 and does not fall back to card1. This is connector identity evidence only, not connected HDMI audio or audible qualification. Orange runtime-only Check/Apply/Rollback uses the guarded, profile-qualified updater and explicit runtime-updater ZIP; full Armbian, kernel, device-tree, and image replacement remains manual, and the standalone manual ZIP is not an OTA asset. Profile or asset mismatches fail closed without Raspberry or manual/image fallback.

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
produce a production image or its `production-runtime` bundle; that
bundle is built and hash-checked by the release workflow. No artifact is run
against the board by this helper.

The shared `build-armbian-image` action has an explicit `image_kind` input:
`diagnostic` builds the separate bring-up image, while `production` requires the
hash-bound Orange runtime bundle. The generic `Armbian Image` workflow uses
diagnostic mode; the release workflow invokes the action in production mode.
Its immutable v0.7.5 output was `octessera-0.7.5-orange-pi-zero-2w.img.xz`;
future releases use the same version-qualified name. Diagnostic mode does
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
the exact board parent assets, and records setup-layer proof. Its live release
preflight allows only the two withdrawn macOS assets to be absent; the historical
27-asset trust manifest remains unchanged. No production parent image has been
exercised here.

Pi UI/render responsiveness profiling is quiet by default. Enable periodic summaries with either control:

```bash
OCTESSERA_PI_UI_PROFILE=1 octessera-pi
octessera-pi --profile-ui
```

Summaries include loop cadence, runtime tick lateness/advance, render overruns, snapshot/config sync, hardware polling, and LED/NeoKey/OLED phase timings.

### Pi Timing And Audio Stability Probes

Use Pi-side probes for rhythmic timing, trigger latency, audio-drain latency, and DSP budget questions. PC/runtime-only probes are useful for quick plausibility checks, but they cannot prove hardware audio timing.

`tools/pi/run-pi-timing-probes.ps1` is Raspberry-only. Never point it at the
Orange board: Orange uses the fixed-target capability runner below, and its
offline DSP entry point is explicit rather than environment-triggered.

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

Orange command generation and offline comparisons use:

```powershell
./tools/orange-pi/run-orange-capability-study.ps1 -Mode PassiveBaseline -PrintOnly
./tools/orange-pi/run-orange-capability-study.ps1 -Mode Dsp64 -PrintOnly
./tools/orange-pi/run-orange-capability-study.ps1 -Mode Dsp256 -PrintOnly
```

Non-print DSP runs stop the production service and therefore require an
explicit acknowledgement:

```powershell
./tools/orange-pi/run-orange-capability-study.ps1 -Mode Dsp64 -AllowServiceInterruption
./tools/orange-pi/run-orange-capability-study.ps1 -Mode Dsp256 -AllowServiceInterruption
```

The Orange offline DSP profile locates computational knees but is not live-xrun
proof. The current CPAL/ALSA path cannot count internally recovered `EPIPE`
events, so a clean offline report must not be described as zero xruns or used
to change capabilities. The bounded live-candidate mode is Phase 2 only and
requires `-AllowServiceInterruption`.

For musical timing issues, inspect p99/p99.9/p99.99 and outlier counts, not only p95. Check recent logs after live probes:

```powershell
ssh -i "$env:USERPROFILE\.ssh\octessera_pi_dev" -o IdentitiesOnly=yes pi@192.168.0.211 "journalctl -u octessera.service --since '10 minutes ago' --no-pager | grep -E 'audio callback RT promotion not qualified|audio stream error|underrun|POLLERR' || true"
```

Prefer offering a live probe when the report is subjective or audio-path-specific. Do not run long live probes by default for unrelated code changes.

## Pi Hardware Runtime Debug Loop

Use the real hardware loop for Pi-only behavior, input latency, OLED rendering, LEDs, encoders, menu timing, sample playback, and audio stutter. Automated checks cannot prove tactile timing or display readability.

1. For a new Pi, OS/configuration change, or boot-splash binary/asset change, provision first; then cross-build and deploy from the PC. Fast deployment alone is insufficient for boot-splash changes:

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

## Menu/Control Playback-Priority Changes

Menu and control changes can affect playback timing. Use this checklist when changing `crates/playback-runtime` menu apply paths, desktop/Pi runtime loops, or audio config/control routing:

1. Prefer key-specific fast paths over broad `apply_menu_state()` for high-frequency edit paths.
2. Keep dynamic parameters immediate and bounded.
3. For structural selectors, avoid full config/audio rebuilds unless the selected structure actually changed.
4. Delay autosave payload generation for rapid edits; explicit Save Default remains immediate.
5. Preserve hardware parity by implementing behavior in `playback-runtime` or `platform-core`, not desktop TypeScript.
6. Update `docs/menu-and-controls-spec.md` and `resources/menu-help-texts.tsv` for parity-affecting control/menu behavior; native help coverage must remain specific.
7. Run targeted playback-runtime tests first, then full `cargo test -p playback-runtime` before commit.
8. Rebuild `apps/desktop/dist-desktop/octessera.exe` when the change is desktop-visible.
