# Contributor Development Workflows

This is a contributor reference. End-user hardware build, assembly, and bring-up docs have priority:

- `hardware/pinout-and-connections.md`
- `hardware/enclosure/README.md`
- `hardware/pi-bring-up.md`
- `docs/menu-and-controls-spec.md`

## Install

```bash
corepack pnpm install
```

Use pnpm workspaces. Do not use npm or yarn for this repository.

## Desktop Development

```bash
corepack pnpm --filter @cellsymphony/desktop tauri:dev
```

Windows convenience launcher:

```bash
cellSymphony.bat
```

## Desktop Builds

CI smoke build without bundling:

```bash
corepack pnpm --filter @cellsymphony/desktop tauri:build:ci
```

Portable desktop executable:

```bash
corepack pnpm --filter @cellsymphony/desktop tauri:build:exe
```

The portable executable is copied to `apps/desktop/dist-desktop/CellSymphony.exe`.

On Windows, use the cached wrapper for the same portable build when iterating:

```powershell
./tools/desktop-exe-fast.ps1
```

Rebuild it after significant changes that affect desktop-visible behavior, native runtime behavior, audio behavior, config/default payloads, Tauri host integration, or runtime contracts. Do not rebuild it for Rust-only changes that are clearly internal and not desktop/runtime/audio observable, such as isolated tests, docs, formatting, refactors with no behavior change, or Pi/HAL-only work. When unsure whether a change is observable through the desktop app, rebuild the portable exe.

Release executable and NSIS installer:

```bash
corepack pnpm --filter @cellsymphony/desktop tauri:build
```

Release outputs are written under `target/release/`.

## Explicit GitHub Releases

GitHub release assets are built only by `.github/workflows/release-artifacts.yml` for explicit releases. Tag pushes and intermediate CI builds must not publish release assets.

Release assets:

- `CellSymphony-<version>-windows-installer.exe`: primary Windows installer.
- `CellSymphony-<version>-windows-portable.exe`: portable Windows alternative.
- `CellSymphony-<version>-pi-zero-2w.img.zip`: ready-to-flash Raspberry Pi Zero 2 W image.
- `SHA256SUMS-*.txt`: checksums for release assets.

Release process:

1. Bump versions in Rust manifests, `package.json` files, and `apps/desktop/src-tauri/tauri.conf.json`.
2. Run `corepack pnpm install` after package version edits.
3. Run local validation and rebuild the portable desktop exe if desktop-visible behavior changed.
4. Commit and push the release-prep changes.
5. Create a draft GitHub release such as `v0.5.0`.
6. Run `Release Artifacts` manually with that existing tag, or publish the release to trigger it.
7. Confirm the installer, portable EXE, Pi image zip, and checksum files are attached before announcing the release.

The Pi image build is a necessary slow path because it generates a full Raspberry Pi OS image through pi-gen. Keep it release-only.

The release Pi image must be sanitized: no WiFi credentials, SSH keys, GitHub tokens, host logs, or local user secrets. SSH is disabled by default.

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

## Standard Verification

```bash
corepack pnpm run typecheck
corepack pnpm -r test
corepack pnpm -r lint
corepack pnpm -r format:check
corepack pnpm run quality:audit
cargo fmt --all --check
cargo test -p platform-core -p playback-runtime -p realtime-engine -p cellsymphony-desktop
cargo clippy -p platform-core -p playback-runtime -p realtime-engine -p cellsymphony-desktop --all-targets -- -D warnings
```

The root `typecheck` runs `capabilities:check` before package typechecks.

For menu/runtime-visible Rust changes on Windows, use the focused wrapper while iterating:

```powershell
./tools/validate-menu-runtime.ps1 -IncludePi -BuildDesktopExe
```

Add `-IncludePlatformCore` when platform behavior changes and `-Typecheck` when shared contracts or TypeScript-visible payloads change.

The pre-push hook runs CI-like checks against the committed tree, including lint, typecheck, format checks, tests, coverage, file-length checks, Tauri build smoke, and clippy. Use a long timeout when pushing from automation. Do not skip the hook; fix failures and push again.

On Windows, use the cached Cargo wrapper while iterating. It enables `sccache` when installed, uses a local rustc-wrapper shim to strip Cargo's incremental env var before invoking `sccache`, and passes temporary profile overrides that disable incremental for that command so more crates can be cached. Without `sccache`, Cargo uses its normal `target/` cache:

```powershell
./tools/cargo-fast.ps1 check -p cellsymphony-pi
./tools/cargo-fast.ps1 test -p playback-runtime
```

Install `sccache` once with:

```powershell
cargo install sccache --locked
```

## Pi Builds Without Hardware

Host-stub Pi app build:

```bash
cargo build -p cellsymphony-pi
```

Hardware HAL target check when the Rust target is installed:

```bash
cargo check --target aarch64-unknown-linux-gnu -p cellsymphony-hal --features pi-zero
```

## Pi Hardware Build

Preferred fast path: run `./tools/build-pi-cross.ps1` to produce a Linux ARM binary, then upload it with `./tools/deploy-pi-fast.ps1 -LocalBinary target/pi-cross/cellsymphony-pi -NoTail`. On Windows, the helper uses WSL2 Docker automatically when available. Native cross-builds are still supported with an ARM Linux sysroot and cross `pkg-config` setup for ALSA.

```powershell
./tools/build-pi-cross.ps1
./tools/deploy-pi-fast.ps1 -Target pi@192.168.0.211 -LocalBinary target/pi-cross/cellsymphony-pi -NoTail
```

On a Pi or properly configured cross environment:

```bash
cargo build -p cellsymphony-pi --features hardware-pi
```

Low-resource on-Pi fallback:

```bash
CARGO_BUILD_JOBS=1 cargo build --profile pi-dev -p cellsymphony-pi --features hardware-pi
```

Pi UI/render responsiveness profiling is quiet by default. Enable periodic summaries with either control:

```bash
CELLSYMPHONY_PI_UI_PROFILE=1 cellsymphony-pi
cellsymphony-pi --profile-ui
```

Summaries include loop cadence, runtime tick lateness/advance, render overruns, snapshot/config sync, hardware polling, and LED/NeoKey/OLED phase timings.

### Pi Timing And Audio Stability Probes

Use Pi-side probes for rhythmic timing, trigger latency, audio-drain latency, and DSP budget questions. PC/runtime-only probes are useful for quick plausibility checks, but they cannot prove hardware audio timing.

Wrapper examples:

```powershell
# Safe default: runtime-only, does not stop the service or open live audio.
./tools/run-pi-timing-probes.ps1 -Mode RuntimeOnly -Durations 15s -Scenarios idle,sense

# Optional live-audio probe. Use when subjective timing/audio behavior is unclear.
./tools/run-pi-timing-probes.ps1 -Mode Live -Durations 10m -Scenarios idle

# Optional audio-source drain latency probe.
./tools/run-pi-timing-probes.ps1 -Mode AudioDrain -Durations 10m

# Focused FX budget profile.
./tools/run-pi-timing-probes.ps1 -Mode DspFxLimits
```

The wrapper stops `cellsymphony.service` for live/audio/DSP modes and restarts it after the probe. Runtime-only mode leaves the service running. Use `-PrintOnly` to inspect the remote command first.

For musical timing issues, inspect p99/p99.9/p99.99 and outlier counts, not only p95. Check recent logs after live probes:

```powershell
ssh -i "$env:USERPROFILE\.ssh\cellsymphony_pi_dev" -o IdentitiesOnly=yes pi@192.168.0.211 "journalctl -u cellsymphony.service --since '10 minutes ago' --no-pager | grep -E 'realtime priority unavailable|audio stream error|underrun|POLLERR' || true"
```

Prefer offering a live probe when the report is subjective or audio-path-specific. Do not run long live probes by default for unrelated code changes.

## Pi Hardware Runtime Debug Loop

Use the real hardware loop for Pi-only behavior, input latency, OLED rendering, LEDs, encoders, menu timing, sample playback, and audio stutter. Automated checks cannot prove tactile timing or display readability.

1. Cross-build and deploy from the PC first:

   ```powershell
   ./tools/build-pi-cross.ps1
   ./tools/deploy-pi-fast.ps1 -Target pi@192.168.0.211 -LocalBinary target/pi-cross/cellsymphony-pi -NoTail
   ```

2. Ask for a focused hardware observation before assuming the fix worked. Specify the control path, expected behavior, and what failure would look or sound like.
3. Pull service logs and profile summaries when the observation is unclear. Enable `CELLSYMPHONY_PI_UI_PROFILE=1` only while profiling, then disable it again.
4. For menu, control, or runtime stutter, inspect `pi-ui-profile` and `menu-key-profile` output before broad refactors.
5. Fix the source path. Do not add fallbacks for broken Cell Symphony wiring; fail visibly so missing handlers, stale config paths, or bridge mismatches get fixed at the source.
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
8. Rebuild `apps/desktop/dist-desktop/CellSymphony.exe` when the change is desktop-visible.
