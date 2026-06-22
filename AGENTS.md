# AGENTS.md

## Purpose

Cell Symphony is a pnpm workspace plus Cargo workspace built around a native Rust core/runtime, a Rust realtime synth, a Tauri desktop hardware simulator, and a native Pi app. The product turns cellular automata into music. TypeScript is limited to desktop UI and shared bridge/display/runtime contracts.

## Keep These Boundaries

- Native runtime behavior belongs in `crates/platform-core`; do not reintroduce behavior/runtime execution paths in desktop TypeScript.
- `crates/platform-core/` is the canonical core for behavior execution, grid state, interpretation, mapping, transforms, and native part engine logic.
- `crates/playback-runtime/` owns the native runner/runtime protocol, menu model, transport/runtime status, snapshots, platform effects, audio commands, MIDI/store/sample-browser handling, and the desktop-facing runner contract.
- `apps/desktop/src-tauri/src/runtime_worker.rs` instantiates `NativeRunner`; desktop must not add Node/TypeScript fallbacks for playback or control behavior.
- `apps/desktop/src/` is a simplified simulator UI layer: it renders snapshots, captures input, and emits device/runtime messages. Do not move menu, transport, interpretation, storage, MIDI, or audio logic into React/TypeScript.
- Desktop host adapters in `apps/desktop/src-tauri/src/` own Tauri-side storage, sample browsing/decoding, MIDI, and audio-device integration.
- `crates/realtime-engine/` and `crates/rodio-engine-source/` own internal synth/sample rendering, routing, FX buses, and final stereo mix.
- Native behaviors are registered in `crates/platform-core/src/behaviors/` and must cover both desktop and Pi runtime behavior.

## High-Value Rules

- Use `pnpm`, not npm or yarn.
- After editing any `package.json`, run `pnpm install`.
- Use package-scoped commands when possible: `pnpm --filter <package> ...` and `cargo test -p <crate>`.
- Node tests use `node:test` and `tsx --test`; do not introduce Jest or Vitest.
- TypeScript is strict mode.
- Prefer `type` over `interface`, `export type`, and `export function` for top-level exports.
- Use arrow functions for closures and `function` for top-level exports.
- Prefer minimal diffs. Centralize shared behavior only when it actually reduces duplication.
- Do not add source-code comments unless they are genuinely necessary.
- Keep files under the 500 line limit. If a file exceeds the limit, do a real extraction pass that improves single responsibility; do not make cosmetic line-count reductions.

## Hardware Parity

- Hardware behavior is canonical; software controls must mirror hardware input semantics.
- Do not add desktop-only control paths or UI logic that bypasses `crates/playback-runtime` or `crates/platform-core`.
- Native core/runtime must stay Tauri- and hardware-agnostic; Tauri and Pi code are adapters.
- Internal synth/sample instruments route through the realtime-engine mixer path. MIDI instruments emit external MIDI and do not route through internal audio FX.
- Grid coordinate conversion is parity-critical. Preserve world-space lower-left semantics and display conversion through `GRID_DOMAIN.toDisplayIndex`.
- Menu display semantics must stay stable. For example, named selectors should display labels like `I1: synth`, not raw IDs.
- Menu enum channel targets are strings: `"0"`, `"1"`, `"2"`, `"3"`.
- Sample browser behavior must preserve `..`, `[folder]`, file rows, `(empty)`, preview-input preview, and clipped/scrolled long names without OLED overlap.
- Overlay priority and coordinate behavior for Dance/Fn, sample assignment, trigger probability, and ghost cells are defined in `docs/menu-and-controls-spec.md`.

## Required Documentation Updates

- Any parity-affecting or control/menu change must update `docs/menu-and-controls-spec.md` in the same commit.
- Keep `resources/menu-help-texts.tsv` in sync with enum/help-text changes; native tests enforce coverage.
- Keep `resources/platform-capabilities.json` as the source of truth for platform dimensions and limits; regenerate TypeScript capability exports after edits.
- Run `corepack pnpm run capabilities:generate` after editing platform capabilities, and rely on the Rust build to regenerate native capability constants.
- Keep `docs/runtime-boundaries.md` aligned with native runtime/core ownership.
- `docs/open-work.md` tracks only current actionable follow-up work, not completed history.

## Pitfalls To Avoid

- Multi-line `package.json` edits can accidentally delete the `dependencies` block.
- Keep TS/Rust bridge mappings aligned when renaming shared types, parameters, or instrument names.
- Do not add fallbacks for our own broken wiring, configs, capability data, menu layout, or bridge logic. Fix the source.
- Fallbacks are acceptable only for real external compatibility cases such as old configs, disconnected MIDI devices, or missing files/resources, and should preserve safety with a user-visible status when practical.
- Account for the current OS when choosing commands or tooling.
- If the user corrects an earlier instruction, follow the latest instruction.

## Working Style

- Read only the files needed for the current task. Prefer targeted search before broad reads.
- Work in one package at a time unless the task clearly spans packages.
- In planning mode, outline concrete steps before editing.
- In build mode, continue until the task is completed or blocked by a real user decision.
- Keep explanations brief and avoid post-change recap unless it is useful.
- If you encounter repository changes you did not make and they conflict with the current task, stop and ask the user how to proceed.

## Useful Commands

- Desktop dev: `corepack pnpm --filter @cellsymphony/desktop tauri:dev`
- Verification: `corepack pnpm run typecheck`, `corepack pnpm -r test`, `corepack pnpm -r lint`, `corepack pnpm -r format:check`, `corepack pnpm run quality:audit`
- Rust checks: `cargo fmt --all --check`, `cargo test -p platform-core -p playback-runtime -p realtime-engine -p cellsymphony-desktop`, `cargo clippy -p platform-core -p playback-runtime -p realtime-engine -p cellsymphony-desktop --all-targets -- -D warnings`
- Desktop build smoke check: `corepack pnpm --filter @cellsymphony/desktop tauri:build:ci`
- Capabilities: `corepack pnpm run capabilities:generate`, `corepack pnpm run capabilities:check`
- Windows Pi builds use HAL stubs by default; real Pi builds use `-p cellsymphony-pi --features hardware-pi` or the cross-build workflow.
- Pre-push hook: `.githooks/pre-push` runs lint, typecheck, format checks, tests, file-length checks, and `cargo clippy`.
