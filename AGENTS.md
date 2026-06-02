# AGENTS.md — Conventions for AI Coding Assistants

## Project Overview

Cell Symphony is a monorepo (pnpm workspaces) combining a TypeScript core engine with a Rust realtime synth, packaged as a Tauri desktop app. The app turns cellular automata algorithms into music via a pluggable `BehaviorEngine` system. 

## Key Conventions

### Package Management
- Uses **pnpm** workspaces (not npm or yarn)
- After editing any `package.json`, run `pnpm install` to regenerate workspace symlinks
- All behavior packages depend on `@cellsymphony/behavior-api` and `@cellsymphony/device-contracts`

### Testing
- Node.js `node:test` + `tsx --test`, no Jest/Vitest
- Run: `pnpm --filter <package> test` or `pnpm -r test`

### Code Style
- No comments unless absolutely necessary; TypeScript strict mode
- Prefer `type` over `interface`; `export type` and `export function` pattern
- Arrow functions for closures, `function` keyword for top-level exports
- Centralize shared behavior behind helpers; prefer repeated data over repeated behavior
- Prefer minimal diffs — change only what is necessary

### Architecture
- Design for Change, not for Future
- `packages/platform-core/src/index.ts` = single-entry core module (menu, transport, config, behavior orchestration)
- `packages/behavior-api/` = `BehaviorEngine` interface + registry (`registerBehavior`, `getBehavior`, `listBehaviorIds`)
- All behaviors are registered at import time via top-level `registerBehavior()` calls

### Hardware/Software Parity
- Hardware behavior is canonical; software controls must mirror hardware input semantics
- Do not add desktop-only control paths or UI logic that bypasses `platform-core`
- Any parity-affecting change must update `docs/menu-and-controls-spec.md` in the same commit

### Documentation
- `docs/menu-and-controls-spec.md` is the single source of truth for menu structure and controls — update in the same commit as any control/menu change
- Keep `packages/platform-core/resources/menu-help-texts.tsv` in sync; coverage enforced by lint
- When any enum parameter changes, update its help text in the TSV in the same commit

### Common Pitfalls
- Multi-line edits spanning `scripts` and `devDependencies` in package.json may accidentally delete the `dependencies` block
- After editing package.json, always run `pnpm install`
- Menu `enum` options for channel targets are strings (`"0"`, `"1"`, `"2"`, `"3"`), not numbers
- `visibleChildren()` filters nodes using optional `visible` predicate on `RuntimeConfig`
- If you see changes in the repository that you did not make, always ask what to do with them.
- When I tell you something, and later correct it, take my later instructions as my real intention, even if they contradict earlier statements.

## AI Assistant Guidelines

### Context Efficiency
- Read only the files directly relevant to the current task; avoid broad directory reads
- Prefer `grep`/`find` to locate specific symbols before reading full files
- Work on one package at a time; do not span multiple packages in a single task unless explicitly asked
- When modifying a file, read only the relevant section first, not the whole file
- When in "Planning" mode, do not output full code passages, but output enough detail so that implementation in "Build" mode is straightforward.

### Task Scope
- When in Plan mode, break large tasks into explicit steps and confirm the plan before making changes
- Complete one step fully before moving to the next
- In Build mode, do not stop before you've reached definite end state — either a task completion or a roadblock requiring user intervention. In case of a necessary intervention, explicitly tell the user what is required.

### Output Discipline
- Keep explanations brief; code changes speak for themselves
- Do not summarize what you just did after making changes
- Do not add comments to source code (see Code Style)

### Structure Specifics
- `packages/platform-core/src/index.ts` is a barrel export; do not read it to understand scope — navigate directly to the relevant module instead
- The monorepo has many packages; use `pnpm --filter <package>` to scope commands and avoid cross-package side effects
- When tracing behavior registration, start from the specific behavior package, not from `platform-core` entry point

### Online Research
- When you are facing a problem that you cannot reliably solve, utilize the tools at your disposal to find a solution online, in related resources or communities.

### Task execution
- At the start of every task, write the original goal verbatim, and a todo list to `TASK.md` in the project root.
- Add instructions on the `TASK.md` lifecycle (as described here) to `TASK.md` itself.
- Update `TASK.md` after each completed step
- Re-read `TASK.md` before every tool call to verify the current action still serves the original goal
- If the current action cannot be directly traced back to the goal in `TASK.md`, stop and re-read before proceeding
- Delete `TASK.md` only when the task is fully complete and all tests pass
