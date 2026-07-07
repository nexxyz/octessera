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
- Keep files under the 500 line limit. Treat files drifting toward 300 lines as a design smell to review, not a limit to enforce. Extract early when it improves single responsibility and cohesion.
- Ask oracle/QA reviews to consider single responsibility and cohesion. Do not accept extraction that only moves code to satisfy a line count.
- Do not extract vague `helper` modules or functions. Extract functionality into single-responsibility modules with domain names that describe the operation or ownership.

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

- Documentation is end-user hardware build, assembly, bring-up, and operation first. Contributor/collaboration docs are secondary and should not bury the hardware/user path. Point-in-time audits or completed-work history are lowest priority and should be pruned or moved out of the main docs set.
- Any parity-affecting or control/menu change must update `docs/menu-and-controls-spec.md` in the same commit. If the full menu tree changes, also update the canonical split-out tree in `docs/menu-tree-spec.md`.
- Keep `resources/menu-help-texts.tsv` in sync with enum/help-text changes; native tests enforce coverage.
- Keep `resources/platform-capabilities.json` as the source of truth for platform dimensions and limits; regenerate TypeScript capability exports after edits.
- Run `corepack pnpm run capabilities:generate` after editing platform capabilities, and rely on the Rust build to regenerate native capability constants.
- Keep `docs/runtime-boundaries.md` aligned with native runtime/core ownership.
- `docs/open-work.md` tracks only current actionable follow-up work, not completed history.

## Pitfalls To Avoid

- Multi-line `package.json` edits can accidentally delete the `dependencies` block.
- Keep TS/Rust bridge mappings aligned when renaming shared types, parameters, or instrument names.
- Do not add fallbacks for our own broken wiring, configs, capability data, menu layout, or bridge logic. Fix the source.
- Prefer clean source-model fixes over patching symptoms or filtering/removing exception fields after the fact, unless the clean design is clearly disproportionate to the task.
- Fallbacks are acceptable only for real external compatibility cases such as old configs, disconnected MIDI devices, or missing files/resources, and should preserve safety with a user-visible status when practical.
- Account for the current OS when choosing commands or tooling.
- If the user corrects an earlier instruction, follow the latest instruction.
- Rebuild the portable desktop exe after significant changes that affect desktop-visible behavior, native runtime behavior, audio behavior, config/default payloads, Tauri host integration, or runtime contracts. Do not rebuild it for Rust-only changes that are clearly internal and not desktop/runtime/audio observable, such as isolated tests, docs, formatting, refactors with no behavior change, or Pi/HAL-only work. When unsure whether a change is observable through the desktop app, rebuild the portable exe.
- Prefer PC-side Pi cross-builds for hardware/profile loops: `./tools/pi/build-pi-cross.ps1` uses WSL2 Docker automatically when available and writes `target/pi-cross/cellsymphony-pi`. Deploy with `./tools/pi/deploy-pi-fast.ps1 -LocalBinary target/pi-cross/cellsymphony-pi -NoTail` instead of building on the Pi.
- Preserve build caches during Pi deploy/profile loops. Prefer cross-built binary deploys. If source sync is needed, use `tools/pi/deploy-pi-fast.ps1` without `-CleanRemote`; it uses cache-preserving remote sync so unchanged source mtimes and Cargo `target/` fingerprints survive. Use `-CleanRemote` only when intentionally discarding the Pi build cache.

## Working Style

- Read only the files needed for the current task. Prefer targeted search before broad reads.
- Work in one package at a time unless the task clearly spans packages.
- In planning mode, outline concrete steps before editing.
- In build mode, continue until the task is completed or blocked by a real user decision.
- Every code-change loop should leave the codebase in a potentially shippable, production-quality state unless the user explicitly approves otherwise. Do not defer known cleanup, dead code removal, stale tests, obsolete commands, required docs, or required validation as optional follow-up.
- Keep explanations brief and avoid post-change recap unless it is useful.
- If a first fix for a desktop-visible menu/control/runtime bug fails, reproduce the reported phenomenon with a full UI-level or device-input replay before attempting another fix. Prefer tests that follow the user-visible flow over direct internal state mutation alone.
- Always prefer fast paths for live/runtime/menu/audio/control changes. Use slow paths only when they are absolutely necessary; when a slow path is necessary, inform the user explicitly and clearly, including why the slow path cannot be avoided.
- For menu/control changes that affect playback priority, avoid broad `apply_menu_state()` on high-frequency edit paths. Prefer key-specific fast paths, delayed autosave payload generation, full `cargo test -p playback-runtime` after targeted tests, and the portable desktop exe rebuild when desktop-visible.
- When committing and immediately pushing, run targeted confidence checks and required artifact builds before committing, then rely on the pre-push hook for exhaustive CI-like validation. Do not manually run a hook-equivalent full suite immediately before `git push` unless the change is high-risk, the user asks, or the hook cannot run.
- If you encounter repository changes you did not make and they conflict with the current task, stop and ask the user how to proceed.

## Useful Commands

- Desktop dev: `corepack pnpm --filter @cellsymphony/desktop tauri:dev`
- Verification: `corepack pnpm run typecheck`, `corepack pnpm -r test`, `corepack pnpm -r lint`, `corepack pnpm -r format:check`, `corepack pnpm run quality:audit`
- Rust checks: `cargo fmt --all --check`, `cargo test -p platform-core -p playback-runtime -p realtime-engine -p cellsymphony-desktop`, `cargo clippy -p platform-core -p playback-runtime -p realtime-engine -p cellsymphony-desktop --all-targets -- -D warnings`
- Desktop build smoke check: `corepack pnpm --filter @cellsymphony/desktop tauri:build:ci`
- Portable desktop exe: `corepack pnpm --filter @cellsymphony/desktop tauri:build:exe` writes `apps/desktop/dist-desktop/CellSymphony.exe`
- Capabilities: `corepack pnpm run capabilities:generate`, `corepack pnpm run capabilities:check`
- Windows Pi builds use HAL stubs by default; real Pi builds use `-p cellsymphony-pi --features hardware-pi` or the cross-build workflow.
- Pi cross-build/deploy: `./tools/pi/build-pi-cross.ps1`; then `./tools/pi/deploy-pi-fast.ps1 -Target pi@192.168.0.211 -LocalBinary target/pi-cross/cellsymphony-pi -NoTail`.
- Pi source sync fallback: `./tools/pi/deploy-pi-fast.ps1 -Target pi@192.168.0.211 -SyncOnly -NoTail` preserves the remote Cargo cache; avoid ad hoc full tar extraction over the repo.
- Pre-push hook: `.githooks/pre-push` runs lint, typecheck, format checks, tests, file-length checks, and `cargo clippy`.
- Git push: use a long timeout because the pre-push hook runs CI-like checks. Do not skip the hook; fix failures and retry.
