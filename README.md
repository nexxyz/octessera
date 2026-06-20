# Cell Symphony

Cell Symphony is a native Rust music platform that turns cellular automata into musical events. It ships as a Tauri desktop hardware simulator and as a native Raspberry Pi app target using the same Rust runtime/core behavior path.

## Current State

- Native Rust runtime, menu, transport, behavior, interpretation, mapping, and config logic are canonical.
- TypeScript is limited to desktop UI and shared bridge/display/runtime contracts.
- Desktop uses Tauri as the host adapter for windowing, storage, MIDI, sample browsing, sample decoding, and audio output.
- Pi Zero uses a native Rust app with HAL stubs for host builds and `hardware-pi` for real hardware builds.
- Internal synth/sample audio routes through `crates/realtime-engine` and `crates/rodio-engine-source`; MIDI instruments emit external MIDI instead of entering the internal audio mixer.

## What It Does

- Runs an 8x8 cellular grid with native behaviors: none, sequencer, keys, Life, Brain, Ant, Bounce, Shapes, Raindrops, DLA, and Glider.
- Interprets cell changes and scan positions into trigger intents.
- Maps trigger intents into notes, CC values, sample playback, MIDI output, and parameter modulation.
- Provides eight instrument slots with synth, sampler, MIDI, and silent `none` modes.
- Provides mixer routing, FX buses, global FX, pan, volume, sample assignment, voice stealing policy, MIDI clock, preset/default storage, contextual help, and OLED/grid/NeoKey snapshots.
- Provides a Dance layer for live mix, pan, trigger probability/gate control, XY modulation, and mapped momentary FX.

Platform dimensions and limits are defined in `resources/platform-capabilities.json`. Generated TypeScript and Rust constants must stay in sync with that file.

## Controls

| Control | Desktop Key | Runtime Input | Function |
|---|---|---|---|
| Main encoder turn | Left / Right / Up / Down | `encoder_turn:main` | Move cursor or adjust edited value |
| Main encoder press | Enter | `encoder_press:main` | Enter group, toggle/edit value, confirm action |
| Back button | Backspace / Esc | `button_a` | Back, exit edit, exit assignment mode |
| Space button | Space | `button_s` | Play / pause |
| Shift | Shift | `button_shift` | Modifier for destructive and layer actions |
| Fn | Control | `button_fn` | Modifier for part/page overlays |
| Shift + Space | Shift + Space | `button_shift` + `button_s` | Emergency stop / panic |
| Shift + Back | Shift + Backspace / Shift + Esc | `button_shift` + `button_a` | Clear active part grid |
| Aux encoders | UI controls | `encoder_*:aux1..aux3` | Bound parameter/action control |

Fn overlays:

- Left grid column selects the active part.
- Right grid column toggles or selects the Dance page.
- Combined Shift + Fn activates combined-modifier behavior described in `docs/menu-and-controls-spec.md`.

## Menu Overview

The authoritative menu/control spec is `docs/menu-and-controls-spec.md`.

- `L1`: active part behavior, step rate, behavior parameters, saved grid state, and part naming.
- `L2`: per-part scan/sense settings, trigger mapping, pitch, velocity, filter lanes, probability maps, and param modulation.
- `L3`: instruments, synth/sample/MIDI settings, sample assignment, mixer routing, FX buses, and global FX.
- `L4`: Dance page selection, BPM, trigger mode grid, XY controls, momentary FX setup, and grid mapping.
- `System`: presets, default/factory actions, sound, MIDI, UI brightness/sleep, and context help.

## Repository Layout

```text
cellSymphony/
├── apps/
│   ├── desktop/                  # Tauri desktop host and UI
│   └── pi-zero/                  # Native Pi app target
├── crates/
│   ├── platform-core/            # Native behavior/grid/interpretation/mapping core
│   ├── playback-runtime/         # Native runner, protocol, snapshots, menu, platform effects
│   ├── realtime-engine/          # Rust synth/sample/FX mixer
│   ├── rodio-engine-source/      # Rodio source wrapper for desktop/Pi audio output
│   └── hal/                      # Pi hardware abstraction layer and host stubs
├── packages/
│   └── device-contracts/         # Shared TypeScript bridge/display/runtime contracts
├── resources/                    # Menu help text and platform capabilities
├── docs/                         # Current architecture, workflow, menu, and quality docs
├── hardware/                     # Pi pinout, enclosure, and bring-up docs
└── tools/                        # Repository maintenance tools
```

## Development

Install dependencies:

```bash
corepack pnpm install
```

Run the desktop app:

```bash
corepack pnpm --filter @cellsymphony/desktop tauri:dev
```

Run the standard checks:

```bash
corepack pnpm run typecheck
corepack pnpm -r test
corepack pnpm -r lint
corepack pnpm -r format:check
cargo fmt --all --check
cargo test -p platform-core -p playback-runtime -p realtime-engine -p cellsymphony-desktop
cargo clippy -p platform-core -p playback-runtime -p realtime-engine -p cellsymphony-desktop --all-targets -- -D warnings
```

Build desktop release artifacts:

```bash
corepack pnpm --filter @cellsymphony/desktop tauri:build
```

Build the Pi app with host stubs:

```bash
cargo build -p cellsymphony-pi
```

See `docs/development-workflows.md` for complete build, verification, capability-generation, and Pi hardware build notes.

## Documentation

- `docs/menu-and-controls-spec.md`: authoritative controls, menu structure, overlays, persistence, and display behavior.
- `docs/runtime-boundaries.md`: crate/host responsibilities and dependency boundaries.
- `docs/development-workflows.md`: current development, build, verification, and capability-generation commands.
- `docs/engineering-quality-requirements.md`: current quality gates and definition of done.
- `docs/open-work.md`: current actionable work only.
- `hardware/pinout-and-connections.md`: Pi wiring and logical input mapping.
- `hardware/pin-conflict-matrix.md`: GPIO/bus allocation audit.
- `hardware/hardware-integration-plan.md`: current Pi integration status and bring-up checklist.
- `hardware/enclosure-frontplate-revA-dimensions.md`: frontplate dimensions.

## Samples

Repository sample content is sourced from the Stargate sample pack:

```text
https://github.com/stargatedaw/stargate-sample-pack
```

## License

Copyright (c) 2026 nexxyz.

Free for personal/non-commercial use: you may use, copy, modify, and build this software for personal purposes.

Commercial use or selling hardware devices requires prior written permission.

To request permission, contact: https://github.com/nexxyz

See `LICENSE` for full terms.
