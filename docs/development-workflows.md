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
- [Image construction](workflows/image-construction.md):
  pi-gen/Armbian construction, boot layers, and validators.
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
  controls and menu behavior, including which settings belong to the System
  file and which to the musical patch.
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
  on transport pulses.
- The feature scenarios start from a fixed scenario patch
  (`tests/e2e/fixtures/scenario_patch.json`, the straight 120 BPM factory patch
  from before 0.9), so a new shipped patch does not move their ground;
  `default_patch.rs` plays the shipped default as a new install boots it.
- The tests live in `apps/desktop/src-tauri/tests/e2e/`. A failing scenario
  keeps its audio as a WAV under the system temp folder
  (`octessera-e2e/`) so you can listen to it.

The pre-push hook and the CI desktop job run them; run them yourself after
changes to playback, menus, Link mapping, MIDI, instruments or FX.

## Saved-key changes

The save loaders reject any key or value the current model does not know, so
removing, renaming, or re-ranging a saved key can stop a board from booting its
own store. Until a beta there are no concrete migrations: instead of teaching the
loader old shapes, normalize the existing stores (both boards and this host's
desktop store) to the new schema. Copy the board stores to `target/board-stores/`
(they hold your music, so keep them out of the tree):

```powershell
$PiTarget = "pi@<PI_HOST>"; $OrangeTarget = "octessera@<ORANGE_HOST>"
New-Item -ItemType Directory -Force target/board-stores | Out-Null
./tools/board-dev/with-board-ssh.ps1 ssh -Board rpi -Target $PiTarget "tar czf /tmp/stores.tgz --exclude='*.wav' -C /home/pi/presets ."
./tools/board-dev/with-board-ssh.ps1 scp -Board rpi -Target $PiTarget "${PiTarget}:/tmp/stores.tgz" target/board-stores/rpi.tgz
./tools/board-dev/with-board-ssh.ps1 ssh -Board opi -Target $OrangeTarget "sudo -n tar czf /tmp/stores.tgz -C /var/lib/octessera/presets . && sudo -n chown octessera /tmp/stores.tgz"
./tools/board-dev/with-board-ssh.ps1 scp -Board opi -Target $OrangeTarget "${OrangeTarget}:/tmp/stores.tgz" target/board-stores/opi.tgz
```

Unpack each archive into `target/board-stores/rpi/` and `target/board-stores/opi/`,
then compose every patch and rotated `backups/bak-*.json` against its board's
`system.json`. The test skips the folders under `backups/` (`pre-restore-<session>/`),
which keep raw copies of patches a restore could not archive:

```bash
OCTESSERA_STORE_COPY="$PWD/target/board-stores" cargo test -p playback-runtime --test board_store_compose -- --ignored --nocapture
```

The desktop store lives in `%APPDATA%/xyz.nex.octessera/` (or
`OCTESSERA_DESKTOP_STORE_DIR`); copy its `system.json`, `default.patch.json`,
and `presets/` into a third folder next to the boards to compose it too. With
`OCTESSERA_DESKTOP_STORE_DIR` set, user samples live in its `samples/` folder
too. A running simulator holds an exclusive lock on the store's
`octessera.lock`; a second instance on the same store shows a startup error and
exits before it reads the store or repairs a take. The lock covers that one store
only: a dev instance with a different `OCTESSERA_DESKTOP_STORE_DIR` runs next to
the installed app unguarded.

For every document that fails, rewrite only the offending keys or values to the
current schema with a throwaway script under `target/`, keep a backup of the
originals, and compose again until all pass. Then write the stores back: stop the
service (or close the desktop app), keep a copy of the old store on the board,
replace the files, and start it again. Leave patches, layers, samples, and
bindings alone.

## Factory default rollout

When the shipped default patch changes, install it on all three stores at once (both
boards and this host's desktop store, which only stands in for the boards). Never
install a default patch on a single device:

```powershell
$PiTarget = "pi@<PI_HOST>"; $OrangeTarget = "octessera@<ORANGE_HOST>"
./tools/board-dev/install-factory-default.ps1 -RaspberryTarget $PiTarget -OrangeTarget $OrangeTarget
```

Close the desktop app first. The script splits `config/generated/pi/default.json` (both
boards) and `config/generated/desktop/default.json` (desktop) with `split_default_config`,
fetches each store with its service stopped, and composes every patch and backup (but not the
`backups/pre-restore-*` raw copies) with the new `default.patch.json` through `board_store_compose`. If any of the three fails, nothing
changes. Otherwise it keeps backups (`<store>.pre-factory-default-<stamp>` on each board
and next to the desktop store, plus the fetched copies under `target/factory-default/`),
installs `default.patch.json` with each file's owner and mode, checks that all three
installed files hash the same as the split before restarting the boards, and prints that
hash. Each device keeps its own `system.json`; `-IncludeSystem` installs the factory one
too. `-WhatIf` stops after the compose.

`native_runner/retired_config_keys.rs` stays as the mechanism for dropping
retired keys on load once there are saves in other hands; until then it carries
no entries, so do not add one instead of normalizing the stores.

## Pre-push hook

`.githooks/pre-push` delegates to `tools/quality/pre-push.sh`; the check list
lives in `tools/quality/pre-push-checks.sh`. The full profile runs lint,
typecheck, format checks, the JS tests under coverage, `cargo fmt`, the
file-length limit, the repository integrity fixtures and gate (generated-output
freshness, legal inventories, links, and the quality audit), the `tools/legal`
packaging tests, the remaining workspace and routing-tree-executor Rust tests,
the [factory patch UI scenario](factory-patch-ui-scenario.md), Rust coverage
(which runs the `platform-core`, `playback-runtime`, and `realtime-engine`
tests), the Tauri build smoke, desktop tests, the desktop end-to-end
scenarios, the Pi crate's tests in both the production and the `profiling`
shape, both Clippy passes, and the Linux-only Pi feature tests (both boards'
`hardware-*` features with their production cognitive Clippy, as CI's Pi Zero
host tests job runs them) in a `rust:1.99.0-bookworm` container through WSL Docker,
with cargo target and registry cached in named volumes. If WSL or Docker is
unavailable the step warns and is skipped; CI still runs it. It takes a
while. Give `git push` a long timeout rather than skipping the hook.

If git does not already run hooks from `.githooks/`, point it there:

```bash
git config core.hooksPath .githooks
```

The hook refuses to run on a dirty working tree. Set
`OCTESSERA_PRE_PUSH_COMMITTED_TREE=1` for the push to validate the committed
`HEAD` in a temporary worktree instead. Run `tools/quality/pre-push.sh --fast`
by hand when you only want lint, typecheck, formatting, and file length.
