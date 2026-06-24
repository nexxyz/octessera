# Development Workflows

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

## Documentation Checks

After changing docs or menu/help resources:

```bash
cargo test -p playback-runtime
git diff --check
```

Search for obsolete completed-work history before committing documentation updates.
