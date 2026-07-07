# Engineering Quality Requirements

This contributor document describes the current quality baseline. It should match the checks that are actually wired in this repository.

Primary docs are still the user and hardware docs: `hardware/docs/pinout-and-connections.md`, `hardware/enclosure/README.md`, `hardware/docs/pi-bring-up.md`, and `docs/menu-and-controls-spec.md`.

## Goals

- Deterministic native behavior, interpretation, mapping, and runtime state transitions.
- Stable audio routing through the realtime-engine mixer for all internal synth/sample paths.
- Host adapters that expose platform errors instead of hiding source bugs behind fallbacks.
- Reproducible desktop and Pi builds.
- Documentation that describes current behavior, not completed-work history.

## Required Checks

For command details, see `docs/development-workflows.md`. The pre-push hook in `.githooks/pre-push` is the broad local CI gate.

TypeScript and generated contract checks:

```bash
corepack pnpm run typecheck
corepack pnpm -r test
corepack pnpm -r lint
corepack pnpm -r format:check
```

Rust checks:

```bash
cargo fmt --all --check
cargo test -p platform-core -p playback-runtime -p realtime-engine -p cellsymphony-desktop
cargo clippy -p platform-core -p playback-runtime -p realtime-engine -p cellsymphony-desktop --all-targets -- -D warnings
```

Build checks:

```bash
corepack pnpm --filter @cellsymphony/desktop tauri:build:ci
cargo build -p cellsymphony-pi
cargo check --target aarch64-unknown-linux-gnu -p cellsymphony-hal --features pi-zero
```

Release builds use:

```bash
corepack pnpm --filter @cellsymphony/desktop tauri:build
```

Quality audit:

```bash
corepack pnpm run quality:audit
```

The audit reports file length, function length, simple complexity, wide signatures, and behavior/behaviour naming drift. It is informational, but newly touched files should not make the report worse.

## TypeScript Baseline

- TypeScript is limited to desktop UI and shared bridge/runtime contracts.
- `strict`, `noUnusedLocals`, and `noUnusedParameters` are enabled through `tsconfig.base.json`.
- Tests use Node `node:test` through `tsx --test`; do not add Jest or Vitest.
- Package `lint` and `format:check` scripts are currently placeholders; do not claim ESLint or Prettier coverage until those tools are wired.

## Rust Baseline

- `platform-core` owns behavior/grid/interpretation/mapping logic and generated platform capability constants.
- `playback-runtime` owns native runtime protocol, runner, menu, snapshots, platform effects, audio commands, and runtime status.
- `realtime-engine` owns synth/sample audio rendering, route/pan, FX buses, global FX, and final stereo mix.
- `apps/desktop/src-tauri` and `apps/pi-zero` are host adapters.
- `cargo clippy` warnings are errors for checked crates.

## Capability And Help Resources

- `resources/platform-capabilities.json` is the source of truth for grid size, part count, instrument count, sample slots, bus count, global FX slots, touch-FX concurrency, scan sections, OLED size, and pan positions.
- Run `corepack pnpm run capabilities:generate` after editing platform capabilities.
- Run `corepack pnpm run capabilities:check` to verify generated TypeScript exports are current.
- Rust capability constants are generated at build time for `platform-core` and `realtime-engine`.
- `resources/menu-help-texts.tsv` must cover every native menu/help target with specific rows; generic fallback help is not allowed.

## File Size And Refactoring

- The hard source-file limit is 500 lines.
- There are currently no active file-length exceptions checked into this repository.
- Prefer focused extraction when working near large functions or oversized files.

## Fallback Policy

- Do not add fallbacks for bugs in native runtime wiring, menu layout, platform capabilities, desktop bridge mapping, or core behavior.
- Acceptable fallbacks are limited to external/compatibility conditions such as older saved configs, disconnected MIDI devices, unavailable files, missing saved resources, and host-device availability.
- External fallbacks should surface a toast/status/result where practical.

## Definition Of Done

A code or behavior change is done when:

- Current docs and resource files are updated in the same change.
- Generated capability outputs are current.
- Relevant TypeScript and Rust tests pass.
- Runtime/core boundary rules remain intact.
- Internal audio paths route through `realtime-engine`.
- Hardware/software input semantics remain aligned.
- Any unverified hardware behavior is recorded in `docs/open-work.md`.
