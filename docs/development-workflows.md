# Contributor Development Workflows

This page is the contributor index. Start user-facing work at
[`userdocs/README.md`](../userdocs/README.md); start release-owner work at the
[release support matrix](../userdocs/release-support.md). The v0.8.0 Raspberry
and Orange constructor/source-bound evidence exists, but it does not close
physical FAT for exact release artifacts.

## Workflow map

Use the focused page for the responsibility you are changing:

- [Desktop development](workflows/desktop-development.md) — simulator,
  hardware-free matrix, Tauri builds, and the heavy UI scenario.
- [Pi development and profiling](workflows/pi-development-and-profiling.md) —
  host builds, Orange cross-build tools, Pi profiling, and audio studies.
- [Image construction and proof](workflows/image-construction-and-proof.md) —
  pi-gen/Armbian construction, boot layers, image modes, and source proofs.
- [Release assembly](workflows/release-assembly.md) — exact assets, staging,
  update boundaries, and the populated-draft handoff.
- [Pi and board deployment](workflows/deployment.md) — state-changing board
  actions, Raspberry deployment, Orange input routing, and runtime debug loops.

The [Orange production reference](../hardware/docs/orange-pi-production-reference.md)
owns detailed production image, service, storage, audio, USB, and updater
contracts. The ordered user procedure remains in
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

Markdown-only edits do not require Rust tests. Edits to
`resources/menu-help-texts.tsv` or native menu/help targets also require:

```bash
cargo test -p playback-runtime
```

For the slower HTTP/BOM pass:

```bash
python tools/docs/check_links.py --http
```

After editing `userdocs/print/*.html`, `userdocs/print/*.svg`, or
`userdocs/print/print.css`, render the user PDF:

```powershell
./tools/docs/render_userdocs_pdf.ps1
```

## Shared source of truth

- [`board-profiles.md`](board-profiles.md) owns the canonical Raspberry and
  Orange IDs, feature owners, aliases, and image boundary.
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

## Focused verification

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

These are focused confidence checks, not the full workspace gate. Keep
desktop-visible native behavior in `platform-core`/`playback-runtime`, not
desktop TypeScript. Internal synth/sample paths use `realtime-engine`; MIDI
instruments remain external MIDI paths.

## Full local and CI verification

The pre-push profiles are:

```bash
./tools/quality/pre-push.sh --fast
./tools/quality/pre-push.sh
corepack pnpm run quality:audit
```

The fast profile skips Cargo tests/builds. The default profile expects a clean
worktree and runs root checks, Cargo formatting/tests/coverage, file-length
checks, the ignored factory-patch scenario, desktop/Pi checks, Tauri smoke, and
clippy. CI separately covers `rodio-engine-source`; the current Rust coverage
script covers `platform-core`, `playback-runtime`, and `realtime-engine`.

The audit warns above 300 lines and enforces the 500-line source limit. Treat
around 300 lines as a design review threshold; split only along real ownership
boundaries.

## Menu and control playback-priority changes

For `playback-runtime` menu apply paths, desktop/Pi loops, or audio
configuration/control routing:

1. Prefer key-specific fast paths over broad `apply_menu_state()` on
   high-frequency edits.
2. Keep dynamic parameters immediate and bounded; avoid full rebuilds unless a
   selected structure changed.
3. Delay autosave serialization for rapid edits; explicit Save Default remains
   immediate.
4. Preserve hardware parity in `playback-runtime` or `platform-core`, not
   desktop TypeScript.
5. Update `docs/menu-and-controls-spec.md` and `resources/menu-help-texts.tsv`
   for parity-affecting behavior.
6. Run targeted tests, then full `cargo test -p playback-runtime`; rebuild the
   portable desktop executable when the change is desktop-visible.

## Release handoff

The [release assembly page](workflows/release-assembly.md) ends with a populated
draft. Keep it unpublished while the owner verifies exact assets, checksums,
manifests, ZIPs, samples, desktop launch, per-board FAT, source duties, and
limitations. A human explicitly makes that decision; a populated draft is not
a public release.
