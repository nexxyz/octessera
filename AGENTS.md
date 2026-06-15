# AGENTS.md — Conventions for AI Coding Assistants

## Project Overview

Cell Symphony is a monorepo (pnpm workspaces plus Cargo workspace) centered on a native Rust platform core/runtime, a Rust realtime synth, a Tauri desktop app, and a native Pi app. The app turns cellular automata algorithms into music through a native behavior engine system. TypeScript is limited to desktop UI and bridge/runtime contracts.

## Key Conventions

### Package Management
- Uses **pnpm** workspaces (not npm or yarn)
- After editing any `package.json`, run `pnpm install` to regenerate workspace symlinks
- Native runtime behavior changes belong in `crates/platform-core`; desktop TypeScript should not reintroduce behavior/runtime execution paths.

### Testing
- Node.js `node:test` + `tsx --test`, no Jest/Vitest
- Run: `pnpm --filter <package> test` or `pnpm -r test`
- Rust checks are package-scoped with Cargo, e.g. `cargo test -p platform-core`, `cargo test -p playback-runtime`, and `cargo test -p cellsymphony-desktop`
- On Windows, the default Pi build uses HAL stubs; real Pi builds use `-p cellsymphony-pi --features hardware-pi` or the cross-build workflow.

### Code Style
- No comments unless absolutely necessary; TypeScript strict mode
- Prefer `type` over `interface`; `export type` and `export function` pattern
- Arrow functions for closures, `function` keyword for top-level exports
- Centralize shared behavior behind helpers; prefer repeated data over repeated behavior
- Prefer minimal diffs — change only what is necessary

### Architecture
- Design for Change, not for Future
- `crates/platform-core/` is the canonical native core for behavior execution, grid state, interpretation, mapping, transforms, and native part engine logic.
- `crates/playback-runtime/` owns the native runner/runtime protocol, native menu model, transport/runtime status, snapshots, platform effects, audio commands, MIDI/store/sample-browser result handling, and desktop-facing runner contract.
- `apps/desktop/src-tauri/src/runtime_worker.rs` instantiates `NativeRunner`; desktop should not reintroduce Node/TypeScript runtime fallbacks for playback/control behavior.
- `crates/realtime-engine/` and `crates/rodio-engine-source/` own internal synth/sample audio rendering, instrument route/pan, FX buses, global FX, and final stereo mix.
- Native behaviors are registered in `crates/platform-core/src/behaviors/` and must cover desktop/Pi runtime behavior.

### Hardware/Software Parity
- Hardware behavior is canonical; software controls must mirror hardware input semantics
- Do not add desktop-only control paths or UI logic that bypasses `crates/playback-runtime` / `crates/platform-core`
- Native core/runtime should stay Tauri/hardware agnostic; Tauri and Pi code are adapters for input, display, storage, MIDI, and audio devices.
- Any parity-affecting change must update `docs/menu-and-controls-spec.md` in the same commit

### Documentation
- `docs/menu-and-controls-spec.md` is the single source of truth for menu structure and controls — update in the same commit as any control/menu change
- Keep `resources/menu-help-texts.tsv` in sync; coverage is enforced by native tests
- When any enum parameter changes, update its help text in the TSV in the same commit
- Keep `docs/runtime-boundaries.md` aligned with the native Rust runtime/core boundary.
- `docs/backlog.md` tracks current native migration regression work; remove/update obsolete TS-migration assumptions rather than adding new work under old migration requirements.

### Pre-Push Checks
- A git pre-push hook is installed at `.githooks/pre-push` (`git config core.hooksPath .githooks`)
- Before any push, the hook runs: `pnpm run lint`, `pnpm run typecheck`, `pnpm run format:check`, `cargo fmt --all --check`, `cargo clippy`
- The hook runs all checks and reports results; push is blocked if any fail
- To bypass temporarily: `git push --no-verify`

### Common Pitfalls
- Multi-line edits spanning `scripts` and `devDependencies` in package.json may accidentally delete the `dependencies` block
- After editing package.json, always run `pnpm install`
- Menu `enum` options for channel targets are strings (`"0"`, `"1"`, `"2"`, `"3"`), not numbers
- Native menu display values must preserve old UI semantics: named selectors should show labels like `I1: synth`, not raw numeric IDs.
- Grid coordinate conversion is parity-critical. Old core uses world-space lower-left coordinates and display-space conversion through `GRID_DOMAIN.toDisplayIndex`; native LED overlays/Dance/Fn/sample assignment code must preserve that orientation.
- Dance/Fn overlays, sample assignment overlays, trigger-probability overlays, and ghost cells have explicit priority/coordinate behavior in `docs/menu-and-controls-spec.md`.
- Internal synth/sample instruments must route through realtime-engine mixer path; MIDI instruments emit external MIDI and should not be routed through internal audio FX.
- Sample browser behavior should match old menu nodes: `..`, `[folder]`, file rows, `(empty)`, preview via preview input, and long names clipped/scrolled without OLED overlap.
- If you see changes in the repository that you did not make, always ask what to do with them.
- When I tell you something, and later correct it, take my later instructions as my real intention, even if they contradict earlier statements.
- Ensure that any bridging elements between TS and Rust are mapping correctly, e.g. when renaming or changing instrument type names, parameters or other shared structures.
- You might be running on Windows, Mac or Linux. Take this into account, especially on tool-use (e.g. some tooling might not be available or work differently, depending on the OS).
- No fallbacks for basic functionality or for mistakes in our own code, capability config, menu layout, native runtime wiring, or desktop bridge. The application is one cohesive product; expose these bugs and fix the source instead of masking them.
- Fallbacks are acceptable only for real external/compatibility conditions, such as loading older configs with missing/renamed parameters, disconnected MIDI devices, unavailable files, or missing saved resources. These fallbacks should preserve safety and trigger a user-visible toast/status notification where practical.

## AI Assistant Guidelines

### Context Efficiency
- Read only the files directly relevant to the current task; avoid broad directory reads
- Prefer `grep`/`find` to locate specific symbols before reading full files
- Work on one package at a time; do not span multiple packages in a single task unless explicitly asked
- When modifying a file, read only the relevant section first, not the whole file
- When in "Planning" mode, do not output full code passages, but output enough detail so that implementation in "Build" mode is straightforward.
- You're likely running on a local model. Execute subagent tasks sequentially.

### Task Scope
- When in Plan mode, break large tasks into explicit steps and confirm the plan before making changes
- Complete one step fully before moving to the next
- In Build mode, do not stop before you've reached definite end state — either a task completion or a roadblock requiring user intervention. In case of a necessary intervention, explicitly tell the user what is required.

### Output Discipline
- Keep explanations brief; code changes speak for themselves
- Do not summarize what you just did after making changes
- Do not add comments to source code (see Code Style)

### Structure Specifics
- The monorepo has many packages; use `pnpm --filter <package>` to scope commands and avoid cross-package side effects
- When tracing native behavior registration, start in `crates/platform-core/src/behaviors/`.
- We have a hard file LoC limit of 500 lines. Consider this when planning implementations.
- Current temporary file-length exceptions include native migration files; prefer further focused splits instead of expanding those files.

### Online Research
- When you are facing a problem that you cannot reliably solve, utilize the tools at your disposal to find a solution online, in related resources or communities.
