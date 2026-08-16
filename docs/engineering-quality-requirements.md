# Engineering Quality Requirements

This contributor document describes the current quality baseline. It should match the checks that are actually wired in this repository.

Primary docs are still the user and hardware docs under `userdocs/`, plus `docs/menu-and-controls-spec.md` for the canonical controls contract.

## Project Scale

Octessera is a hobbyist instrument project. Quality work should protect musical behavior, fixed-hardware correctness, user data, and practical build confidence without turning the repository into an enterprise release system. Prefer direct fixes and physical board tests over new provenance frameworks, policy layers, redundant artifact proofs, compliance machinery, or distribution hardening.

## Goals

- Deterministic native behavior, interpretation, mapping, and runtime state transitions.
- Stable audio routing through the realtime-engine mixer for all internal synth/sample paths.
- Host adapters that expose platform errors instead of hiding source bugs behind fallbacks.
- Reproducible desktop and Pi builds.
- Documentation that describes current behavior, not completed-work history.

## Focused Checks

For command details, see `docs/development-workflows.md`. The pre-push hook in
`.githooks/pre-push` is the broad local CI gate described below.

Workspace checks (broad but not exhaustive):

```bash
corepack pnpm run typecheck
corepack pnpm run test
corepack pnpm run lint
corepack pnpm run format:check
```

These root scripts are workspace aggregates, not uniform lint/format coverage:
the desktop package runs real ESLint and Prettier, while the device-contracts
package currently has no-op `lint` and `format:check` scripts.

Focused Rust checks:

```bash
cargo fmt --all --check
cargo test -p platform-core -p playback-runtime -p realtime-engine -p octessera-desktop
cargo clippy -p platform-core -p playback-runtime -p realtime-engine -p octessera-desktop --all-targets -- -D warnings
```

Build checks:

```bash
corepack pnpm --filter @octessera/desktop tauri:build:ci
cargo build -p octessera-pi
cargo check --target aarch64-unknown-linux-gnu -p octessera-hal --features raspberry-pi-zero-2w
```

`raspberry-pi-zero-2w` and `hardware-raspberry-pi-zero-2w` are the canonical
internal feature owners. The deprecated `rpi-zero-2w`, `pi-zero`,
`hardware-rpi-zero-2w`, and `hardware-pi` aliases remain accepted for existing
Cargo commands and CI coverage; use canonical names for new commands. No alias
removal date is promised.

Release builds use:

```bash
corepack pnpm --filter @octessera/desktop tauri:build
```

Quality audit:

```bash
corepack pnpm run quality:test
corepack pnpm run quality:audit
```

## Full Pre-Push And CI Coverage

The fast pre-push profile is `./tools/quality/pre-push.sh --fast`; it runs root
lint, typecheck, format checks, Cargo formatting, and file-length checks without
Cargo tests or builds. The default `./tools/quality/pre-push.sh` profile is the
full/default local gate on a clean worktree. It adds workspace tests and
coverage, the ignored factory-patch scenario, desktop and Pi checks, Tauri build
smoke, and clippy. Its workspace Cargo test and clippy selections exclude
`rodio-engine-source`; CI separately runs that crate's tests and clippy. The
current CI Rust coverage script covers `platform-core`, `playback-runtime`, and
`realtime-engine`, not `rodio-engine-source`. CI also runs the corresponding
split TypeScript and Rust jobs and conditional parity scenario.

The audit enforces the 500-line source-file limit, warning above 300 LOC and failing above 500 LOC. JavaScript and TypeScript function metrics are syntax-aware Babel AST measurements; Rust function metrics remain approximate regex measurements. Function length, simple complexity, wide signatures, and behavior/behaviour naming drift remain informational staged warnings.

## TypeScript Baseline

- TypeScript is limited to desktop UI and shared bridge/runtime contracts.
- `strict`, `noUnusedLocals`, and `noUnusedParameters` are enabled through `tsconfig.base.json`.
- Tests use Node `node:test` through `tsx --test`; do not add Jest or Vitest.
- `apps/desktop` uses real ESLint and Prettier scripts for lint and format checks.
- `@octessera/device-contracts` still has no-op `lint` and `format:check` scripts;
  workspace-wide commands therefore provide mixed package coverage rather than
  a claim that every package is linted or formatted.

## Rust Baseline

- `platform-core` owns behavior/grid/interpretation/mapping logic plus generated platform capability and display palette constants.
- `playback-runtime` owns native runtime protocol, runner, menu, snapshots, platform effects, audio commands, and runtime status.
- `realtime-engine` owns synth/sample audio rendering, route/pan, FX buses, global FX, and final stereo mix.
- `apps/desktop/src-tauri` and `apps/pi-zero` are host adapters.
- `cargo clippy` warnings are errors for checked crates.

## Capability And Help Resources

- `resources/platform-capabilities.json` is the source of truth for grid size, layer count, instrument count, sample slots, bus count, global FX slots, Play-FX concurrency, scan sections, OLED size, and pan positions.
- Run `corepack pnpm run capabilities:generate` after editing platform capabilities.
- Run `corepack pnpm run capabilities:check` to verify generated TypeScript exports are current.
- Rust capability constants are generated at build time for `platform-core` and `realtime-engine`.
- `resources/display-palette.json` is the source of truth for the shared display/UI palette. Run `corepack pnpm run palette:generate` after editing it, and `corepack pnpm run palette:check` to verify generated TypeScript, CSS, and Rust exports are current. `platform-core` copies the tracked Rust output into `OUT_DIR` at build time.
- `config/defaults/base.json` and platform overrides in `config/defaults/` are the source of truth for shipped default configs.
- Run `corepack pnpm run config:generate` after editing default config sources, and `corepack pnpm run config:check` to verify generated platform defaults are current.
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
- Generated default config outputs are current when default config sources change.
- Generated capability outputs are current.
- Relevant TypeScript and Rust tests pass.
- Runtime/core boundary rules remain intact.
- Internal audio paths route through `realtime-engine`.
- Hardware/software input semantics remain aligned.
- Any unverified hardware behavior is recorded in `docs/open-work.md`.
