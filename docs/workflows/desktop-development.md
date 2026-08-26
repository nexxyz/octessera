# Desktop development

The desktop app is a hardware-free simulator and a Tauri host adapter. It
renders native snapshots, captures input, and does not qualify GPIO, OLED, DAC,
power, or USB behavior.

## Start the simulator

```bash
corepack pnpm --filter @octessera/desktop tauri:dev
```

## Hardware-free verification matrix

These checks validate source, desktop, and host-build behavior only. None is
board qualification.

| Check | Command | Confirms | Does not confirm |
| --- | --- | --- | --- |
| Documentation links | `python tools/docs/check_links.py` | Local Markdown targets resolve | Release downloads or hardware paths |
| Desktop contract | `corepack pnpm --filter @octessera/desktop typecheck` | Desktop TypeScript contracts compile | Physical input, display, or audio output |
| Desktop lint | `corepack pnpm --filter @octessera/desktop lint` | Desktop ESLint checks pass | Runtime behavior or hardware integration |
| Desktop format | `corepack pnpm --filter @octessera/desktop format:check` | Desktop Prettier checks pass | Runtime behavior or hardware integration |
| Desktop tests | `corepack pnpm --filter @octessera/desktop test` | Simulator/runtime-facing test cases pass | Board timing, GPIO, DAC, or USB behavior |
| Native host tests | `cargo test -p platform-core -p playback-runtime -p realtime-engine` | Native behavior and rendering logic pass on the host | A particular board, enclosure, power supply, or assembled control surface |
| Pi default host tests | `cargo test -p octessera-pi` | Default Pi host-stub tests pass without board hardware | Boot images, peripheral wiring, or physical qualification |
| Raspberry-feature host tests | `cargo test -p octessera-pi --no-default-features --features hardware-raspberry-pi-zero-2w` | Canonical Raspberry code and board-neutral host tests pass | Raspberry boot, GPIO, OLED, audio-device, or physical qualification behavior |
| Orange-feature host tests | `cargo test -p octessera-pi --no-default-features --features hardware-orange-pi-zero-2w` | Canonical Orange code and board-neutral host tests pass | Orange boot, GPIO, OLED, audio-device, or physical qualification behavior |
| Pi host-stub build | `cargo build -p octessera-pi` | The Pi application builds without hardware | Boot images, peripheral wiring, or physical qualification |

Keep those limits visible in reports. The desktop lint and format rows run real
ESLint and Prettier checks; root recursive commands also visit packages whose
scripts are no-ops.

## Desktop builds

CI smoke build without bundling:

```bash
corepack pnpm --filter @octessera/desktop tauri:build:ci
```

Portable desktop executable:

```bash
corepack pnpm --filter @octessera/desktop tauri:build:exe
```

The portable executable is copied to `apps/desktop/dist-desktop/octessera.exe`.
The Tauri bundle uses its configured `bundle.resources` entry for the legal
resource tree. Release checks inspect that configured resource contract and the
portable notice ZIP; they do not extract an installer to prove it.

On Windows, use the cached wrapper while iterating:

```powershell
./tools/desktop/desktop-exe-fast.ps1
```

Rebuild the portable executable after changes affecting desktop-visible,
native-runtime, audio, config/default, Tauri-host, or runtime-contract behavior.
Do not rebuild it for clearly internal Rust-only, docs, formatting, or Pi/HAL
changes with no desktop/runtime/audio visibility.

Release executable and NSIS installer:

```bash
corepack pnpm --filter @octessera/desktop tauri:build
```

Release outputs are written under `target/release/`.

## Heavy runtime UI scenario

The factory patch UI scenario drives `NativeRunner` through protocol messages
and simulated device input, not private menu state. Run it when changing menu
traversal, runtime modulation, sampler assignment, Play, or factory-patch setup:

```bash
cargo test -p playback-runtime factory_patch_ui_scenario -- --ignored
```

The documented input recipe is [`../factory-patch-ui-scenario.md`](../factory-patch-ui-scenario.md).
The pre-push hook runs this scenario. CI runs it for parity-sensitive native
inputs and records an explicit successful skip for other pull requests.
