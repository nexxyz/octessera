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

On a Pi or properly configured cross environment:

```bash
cargo build -p cellsymphony-pi --features hardware-pi
```

Cross-building the hardware Pi app from Windows requires an ARM Linux sysroot and cross `pkg-config` setup for ALSA. Without that, `alsa-sys` will fail before linking.

## Documentation Checks

After changing docs or menu/help resources:

```bash
cargo test -p playback-runtime
git diff --check
```

Search for obsolete completed-work history before committing documentation updates.
