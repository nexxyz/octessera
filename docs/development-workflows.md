# Development workflows

This page indexes the current contributor commands and workflow references.
Start user-facing work at [`userdocs/README.md`](../userdocs/README.md).

## Workflow map

Use the focused page for the responsibility you are changing:

- [Desktop development](workflows/desktop-development.md) — simulator,
  hardware-free matrix, Tauri builds, and the heavy UI scenario.
- [Pi development and profiling](workflows/pi-development-and-profiling.md) —
  host builds, Orange cross-build tools, Pi profiling, and audio probes.
- [Image construction](workflows/image-construction-and-proof.md) —
  pi-gen/Armbian construction, boot layers, image modes, and validators.
- [Release assembly](workflows/release-assembly.md) — artifact formats, staging,
  update boundaries, and attribution companions.
- [Pi and board deployment](workflows/deployment.md) — state-changing board
  actions, Raspberry deployment, Orange input routing, and runtime debug loops.

The [Orange production reference](../hardware/docs/orange-pi-production-reference.md)
owns detailed production image, service, storage, audio, USB, and updater
contracts. The technical workbench/hardware-gate procedure remains in
[`orange-pi-armbian-bringup.md`](../hardware/docs/orange-pi-armbian-bringup.md).

## Install and documentation checks

```bash
corepack pnpm install
```

Use pnpm workspaces; do not use npm or yarn.

```bash
python tools/docs/check_links.py
python3 tools/docs/test_release_documentation.py
git diff --check
```

Menu/help changes also use the focused runtime test:

```bash
cargo test -p playback-runtime
```

## Shared source of truth

- [`board-profiles.md`](board-profiles.md) owns the canonical Raspberry and
  Orange IDs, feature owners, and image boundary.
- `resources/platform-capabilities.json` owns dimensions and limits.
- `resources/display-palette.json` owns the shared display/UI palette.
- `config/defaults/` owns shipped default configuration sources.
- [`runtime-boundaries.md`](runtime-boundaries.md) owns native/runtime/adapter
  responsibility boundaries.
- [`menu-and-controls-spec.md`](menu-and-controls-spec.md) owns parity-sensitive
  controls and menu behavior.
- Contributor-only branding guidance lives in
  [`hardware/docs/branding-assets.md`](../hardware/docs/branding-assets.md).

After editing capabilities, palette, or default sources, run the matching
generator and freshness check:

```bash
corepack pnpm run capabilities:generate
corepack pnpm run capabilities:check
corepack pnpm run palette:generate
corepack pnpm run palette:check
corepack pnpm run config:generate
corepack pnpm run config:check
```

Rust capability constants are generated at build time. Generated TypeScript,
CSS, Rust palette, and platform default outputs are checked in. Default config
platform overrides remain limited to device-local brightness values.

## Focused package and crate checks

Use package- and crate-scoped checks while iterating:

```bash
corepack pnpm --filter @octessera/desktop typecheck
corepack pnpm --filter @octessera/desktop lint
corepack pnpm --filter @octessera/desktop format:check
corepack pnpm --filter @octessera/desktop test
cargo fmt --all --check
cargo test -p platform-core -p playback-runtime -p realtime-engine -p octessera-desktop
cargo clippy -p platform-core -p playback-runtime -p realtime-engine -p octessera-desktop --all-targets -- -D warnings
```

These checks keep native runtime behavior in `platform-core` and
`playback-runtime`, internal synth/sample paths in `realtime-engine`, and MIDI
instruments on external MIDI paths. Desktop TypeScript remains a UI and bridge
layer.
