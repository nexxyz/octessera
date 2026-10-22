# Development workflows

This page indexes the contributor commands and workflow references. If you are
here to play the thing rather than change it, the
[user guide](../userdocs/README.md) is the better door.

## Workflow map

Use the focused page for the responsibility you are changing:

- [Desktop development](workflows/desktop-development.md): simulator,
  hardware-free matrix, Tauri builds, and the heavy UI scenario.
- [Pi development and profiling](workflows/pi-development-and-profiling.md):
  host builds, Orange cross-build tools, the shared-path timing smoke, Pi
  profiling, and audio probes.
- [Image construction](workflows/image-construction-and-proof.md):
  pi-gen/Armbian construction, boot layers, image modes, and validators.
- [Release assembly](workflows/release-assembly.md): artifact formats, staging,
  update boundaries, and attribution companions.
- [Pi and board deployment](workflows/deployment.md): state-changing board
  actions, Raspberry and Orange development deployment, Orange input routing,
  and runtime debug loops.

The [Orange production reference](../hardware/docs/orange-pi-production-reference.md)
owns detailed production image, service, storage, audio, USB, and updater
contracts. The technical workbench/hardware-gate procedure remains in
[`orange-pi-armbian-bringup.md`](../hardware/docs/orange-pi-armbian-bringup.md).

## Install and documentation checks

```bash
corepack pnpm install
```

Use pnpm workspaces; npm and yarn will only confuse the lockfile.

The repository integrity gate checks generated assets (configs, capabilities,
palette, logos), legal inventories, internal Markdown links and anchors, and
the code-quality thresholds. Run it after documentation or generator changes:

```bash
bash tools/quality/repository-integrity.sh
git diff --check
```

Skip a class with `--skip-stale-assets`, `--skip-legal`, `--skip-links`, or
`--skip-quality`. The link check on its own is
`python tools/docs/check_links.py`.

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
- [`save-format-split-backlog.md`](workflows/save-format-split-backlog.md)
  records which settings belong to the System file and which to the musical
  patch, including the menu-location exceptions.
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
cargo test -p realtime-engine -p rodio-engine-source --features rodio-engine-source/routing-tree-executor
```

The last command covers the routing-tree executor that production Pi builds
use. Default-feature tests do not compile it, which is easy to forget and
annoying to rediscover.

These checks keep native runtime behavior in `platform-core` and
`playback-runtime`, internal Synth, FM, Plucked, Drum, and Sampler paths in
`realtime-engine`, and MIDI instruments on external MIDI paths. Desktop
TypeScript remains a UI and bridge layer.

## End-to-end scenarios

```bash
corepack pnpm run e2e
```

These play scripted device sessions through the real desktop runtime, host
adapter, sample preparation and engine, without a window or a sound card. The
`headless` feature of `octessera-desktop` pulls audio from the engine itself
and advances the runtime clock by exactly the audio it rendered, so a 40 s
scenario checks in a few seconds and its timing is sample-exact.

- Scenarios are written with `playback_runtime::scenario::ScenarioBuilder`
  (feature `test-support`): menu paths become the encoder turns and presses a
  player would make, with checkpoints on the OLED title, then timed inputs cued
  on transport pulses or seconds after B1.
- The tests live in `apps/desktop/src-tauri/tests/e2e/`. A failing scenario
  keeps its audio as a WAV under the system temp folder
  (`octessera-e2e/`) so you can listen to it.
- `record_teaser_v40_take` is ignored by default: it renders the teaser score
  in real time with the device's Audio+Video recorder running.

They are not part of the pre-push hook; run them after changes to playback,
menus, Link mapping, instruments or FX.

## Pre-push hook

`.githooks/pre-push` delegates to `tools/quality/pre-push.sh`; the check list
lives in `tools/quality/pre-push-checks.sh`. The full profile runs lint,
typecheck, format checks, JS tests and coverage, the quality audit, `cargo fmt`,
the file-length limit, the repository integrity fixtures and gate, workspace
and routing-tree-executor Rust tests, enum help coverage, the
[factory patch UI scenario](factory-patch-ui-scenario.md), Rust coverage, the
Tauri build smoke, desktop and Pi checks, and both Clippy passes. It takes a
while. Give `git push` a long timeout rather than skipping the hook.

If git does not already run hooks from `.githooks/`, point it there:

```bash
git config core.hooksPath .githooks
```

The hook refuses to run on a dirty working tree. Set
`OCTESSERA_PRE_PUSH_COMMITTED_TREE=1` for the push to validate the committed
`HEAD` in a temporary worktree instead. Run `tools/quality/pre-push.sh --fast`
by hand when you only want lint, typecheck, formatting, and file length.
