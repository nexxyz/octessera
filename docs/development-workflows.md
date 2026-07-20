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
- `octessera-<version>-windows-portable.exe`: portable Windows alternative.
- `octessera-<version>-macos-unsigned.dmg`: unsigned macOS DMG.
- `octessera-<version>-ubuntu-amd64.deb`: Ubuntu/Debian package.
- `octessera-<version>-ubuntu-x86_64.AppImage`: portable Linux AppImage.
- `octessera-<version>-pi-zero-2w.img.zip`: ready-to-flash Raspberry Pi Zero 2 W image, including `os_list.rpi-imager-manifest` for Raspberry Pi Imager.
- `octessera-<version>-pi-zero-2w.rpi-imager-manifest`: standalone Raspberry Pi Imager manifest copy.
- `octessera-<version>-device-aarch64.zip`: Linux aarch64 device update payload for the on-device updater.
- `SHA256SUMS-*.txt`: checksums for release assets.

Release process:

1. Bump versions in Rust manifests, `package.json` files, and `apps/desktop/src-tauri/tauri.conf.json`.
2. Run `corepack pnpm install` after package version edits.
3. Run local validation and rebuild the portable desktop exe if desktop-visible behavior changed.
4. Commit and push the release-prep changes.
5. Create a draft GitHub release such as `v0.5.0`.
6. Run `Release Artifacts` manually with that existing tag, or publish the release to trigger it.
7. Confirm the installer, portable EXE, macOS DMG, Ubuntu DEB/AppImage, Pi image zip, Pi Imager manifest, device update ZIP, and checksum files are attached before announcing the release.

The Pi image build is a necessary slow path because it generates a full Raspberry Pi OS image through pi-gen. Keep it release-only.

The release Pi image must be sanitized: no WiFi credentials, SSH keys, GitHub tokens, host logs, or local user secrets. SSH is disabled by default.

Device updates use `/usr/local/sbin/octessera-update` on Pi-family images. It downloads `octessera-<version>-device-aarch64.zip` and `SHA256SUMS-device.txt` from GitHub releases, verifies the checksum and manifest, installs into `/opt/octessera/releases/<version>`, then atomically switches `/opt/octessera/current` and `/usr/local/bin/octessera-pi`. `octessera-update rollback` switches back to the previous recorded release.

## Armbian Image Workflow

`.github/workflows/armbian-image.yml` builds Armbian images through `armbian/build`. The action currently tracks upstream `armbian/build@main`; this is accepted risk for this experimental pipeline, with minimal repository permissions and secrets kept out of validation and public generic builds.

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

GitHub validation-only smoke test:

```bash
gh workflow run armbian-image.yml -f run_build=false -f artifact_mode=public-generic
```

Public generic builds may use board/release/kernel/UI/compression/extensions inputs, a public non-secret `PRESET_CONFIGURATION` URL, and an optional HTTPS payload URL plus SHA256. Do not add raw Wi-Fi, user, SSH, or private first-run values as workflow inputs.

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

The pre-push hook runs this scenario. GitHub CI can also run it from `workflow_dispatch` with `run-heavy-ui-scenario=true`.

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

The pre-push hook runs CI-like checks against the committed tree, including lint, typecheck, format checks, tests, coverage, file-length checks, Tauri build smoke, and clippy. Use a long timeout when pushing from automation. Do not skip the hook; fix failures and push again.

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

Host-stub Pi app build:

```bash
cargo build -p octessera-pi
```

Hardware HAL target check when the Rust target is installed:

```bash
cargo check --target aarch64-unknown-linux-gnu -p octessera-hal --features pi-zero
```

## Pi Hardware Build

Preferred fast path: run `./tools/pi/build-pi-cross.ps1` to produce a Linux ARM binary, then upload it with `./tools/pi/deploy-pi-fast.ps1 -LocalBinary target/pi-cross/octessera-pi -NoTail`. On Windows, the helper uses WSL2 Docker automatically when available. Native cross-builds are still supported with an ARM Linux sysroot and cross `pkg-config` setup for ALSA.

```powershell
./tools/pi/build-pi-cross.ps1
./tools/pi/deploy-pi-fast.ps1 -Target pi@192.168.0.211 -LocalBinary target/pi-cross/octessera-pi -NoTail
```

On a Pi or properly configured cross environment:

```bash
cargo build -p octessera-pi --features hardware-pi
```

Low-resource on-Pi fallback:

```bash
CARGO_BUILD_JOBS=1 cargo build --profile pi-dev -p octessera-pi --features hardware-pi
```

## Orange Pi Armbian Image

The `Armbian Image` GitHub Actions workflow builds the Orange Pi Armbian image. Run validation first:

```bash
gh workflow run armbian-image.yml \
  -f board=orangepizero2w \
  -f release=trixie \
  -f kernel_branch=current \
  -f ui=minimal \
  -f compression=sha,img,xz \
  -f extensions=preset-firstrun \
  -f run_build=false \
  -f artifact_mode=public-generic \
  -f armbian_build_ref=main
```

Run the full public build by changing `run_build=true`. Public builds must stay secret-free. The first-boot setup portal handles Wi-Fi and SSH on the device.

Local validation before pushing image changes:

```bash
bash tools/armbian-image/validate.sh
node --check userpatches/overlay/usr/local/share/octessera-setup-ui/app.js
git diff --check
```

The workflow inspects the artifact before upload. For manual checks, inspect the artifact for expected setup files and no Octessera-added SSH material: no `octessera` user password, no `/etc/ssh/ssh_host_*`, and `ssh.service` masked until setup finalizes. Do not treat Armbian's own first-run root/bootstrap material as an Octessera secret.

If you have an extracted root filesystem directory or ext4 root partition image, run:

```bash
tools/armbian-image/inspect-built-image.sh <rootfs-dir-or-ext4-image>
```

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
ssh -i "$env:USERPROFILE\.ssh\octessera_pi_dev" -o IdentitiesOnly=yes pi@192.168.0.211 "journalctl -u octessera.service --since '10 minutes ago' --no-pager | grep -E 'realtime priority unavailable|audio stream error|underrun|POLLERR' || true"
```

Prefer offering a live probe when the report is subjective or audio-path-specific. Do not run long live probes by default for unrelated code changes.

## Pi Hardware Runtime Debug Loop

Use the real hardware loop for Pi-only behavior, input latency, OLED rendering, LEDs, encoders, menu timing, sample playback, and audio stutter. Automated checks cannot prove tactile timing or display readability.

1. Cross-build and deploy from the PC first:

   ```powershell
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
