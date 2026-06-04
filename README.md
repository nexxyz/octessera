# Cell Symphony

Cell Symphony is a desktop-first hardware-instrument prototype that turns cellular automata into music. The Tauri desktop app is a 1:1 simulator for the planned hardware control surface, not a separate product UX.

> **Work in progress.** This is an active hobby project. Interfaces and behavior can change quickly, but the README should reflect the current implemented state.

---

## What It Is

Cell Symphony combines a pluggable cellular-behavior engine, a hardware-parity menu/control layer, and a realtime synth/sample backend:

- An **8x8 grid** runs one of 11 pluggable algorithms via the shared `BehaviorEngine` API.
- A **sense/mapping layer** turns cell transitions (`activate`, `stable`, `deactivate`, `scanned`) into musical events.
- A **voice layer** manages instruments, synth/sample/MIDI slots, mixer routing, FX buses, and sample-grid assignments.
- A **Dance performance layer** provides grid pages for mix, pan, trigger-gate, and mapped momentary FX.
- A **desktop simulator** mirrors the hardware interface, including OLED, grid LEDs, encoders, buttons, transport, MIDI, and audio bridge behavior.

---

## Current Status

Implemented and functional:

- 11 behaviors: none, sequencer, keys, Conway Life, Brian's Brain, Langton's Ant, Bounce, Shapes, Raindrops, DLA, and Glider.
- Multi-part L1/L2 architecture with per-part behavior, scan/sense settings, mapping, names, and saved grid state.
- Scanning modes, sectioned scanning, scan direction, state/event triggers, and pitch/velocity/filter modulation lanes.
- Internal synth, sample slots, MIDI output/input, external sync, voice stealing policy, and audio load indicators.
- L3 Voice instruments with synth/sample/MIDI types, mixer volume/pan/route, FX buses, and sample assignment mode.
- L4 Dance performance layer with Mix, Pan, Trigger Gate, and FX pages.
- Dance FX mapping: select effect type/params, Map to Grid, press cells for momentary activate/release effects.
- Preset/default storage, factory/default load/save, auto-save, contextual help, OLED rendering, toast feedback, and aux encoder bindings.
- CI-style TypeScript/Rust lint, typecheck, test, and build scripts.

Planned/follow-up:

- Global/master FX workflow refinements.
- Aux mapping enhancements for the Dance context.
- Hardware prototype on Raspberry Pi Zero 2 W with NeoTrellis/NeoKeys/OLED/audio DAC.

---

## Controls

| Hardware Control | Simulator Key | Function |
|---|---|---|
| Main encoder turn | Left / Right | Move cursor or adjust edited value |
| Main encoder press | Enter | Enter group, toggle/edit value, confirm action |
| Back | Backspace / Esc | Back/exit edit/exit assignment mode |
| Space | Space | Play / pause |
| Shift + Space | Shift + Space | Emergency stop / panic |
| Shift + Back | Shift + Backspace / Shift + Esc | Clear active part grid |
| Fn + left grid column | Ctrl/Fn + left column | Select active part |
| Fn + right grid column | Ctrl/Fn + right column | Toggle Dance layer on/off |
| Touch right grid column | right column, no Fn | Select Dance page: mix, pan, trigger-gate, fx |
| Aux encoder press | simulator aux control | Bind/unbind current menu item |
| Aux encoder turn | simulator aux control | Adjust bound parameter |

When Fn is held, the left grid column shows part options and the right grid column shows Dance page options. The active part/page is highlighted.

---

## Menu Overview

The authoritative menu/control spec lives in `docs/menu-and-controls-spec.md`.

- **L1: Life** — per-part behavior, step rate, behavior config, saved grid state, part naming.
- **L2: Sense** — per-part scan mode, scan axis/unit/direction/sections, trigger routing, note mapping, modulation lanes.
- **L3: Voice** — instruments, synth/sample/MIDI settings, sample assignment, mixer volume/pan/route, FX buses.
- **L4: Dance** — Dance Page, BPM, Trigger Gate controls, Dance FX type/params, Map to Grid.
- **Playback** — BPM.
- **System** — presets/defaults/factory, sound settings, MIDI, UI settings, contextual help.

---

## Dance Layer

`Fn + rightmost grid column` selects Dance pages. `Fn + leftmost grid column` selects a part to display and exits Dance.

- **mix**: columns are instruments, y=0 mutes and y=7 sets 100% volume.
- **pan**: rows are instruments, x=0 is hard left and x=7 is hard right.
- **fx**: assigned cells trigger momentary effects while held.

Dance FX maps cells to global-output momentary DSP in the Rust realtime engine:

- `stutter`
- `freeze`
- `filter_sweep`
- `pitch_shift`

To map FX, go to `L4: Dance > FX Page`, select an effect type and parameters, choose `Map to Grid`, then press a grid cell. The platform capability limit is 4 simultaneous held effects; same effect type presses replace the existing active cell.

- **trigger-gate**: columns are parts, rows enable/disable gate per cell. `Fn+Shift` column clears part gates, `Shift` row toggles cell gate. `L4: Dance > Target Part` controls which part(s) the gate applies to.

---

## Algorithms

| Algorithm | Package | Description |
|---|---|---|
| none | `@cellsymphony/behaviors-none` | Empty/no-op behavior |
| sequencer | `@cellsymphony/behaviors-sequencer` | Manual grid toggle |
| keys | `@cellsymphony/behaviors-keys` | Momentary key grid |
| life | `@cellsymphony/behaviors-life` | Conway's Game of Life with optional spawning |
| brain | `@cellsymphony/behaviors-brain` | Brian's Brain 3-state automaton |
| ant | `@cellsymphony/behaviors-ant` | Langton's Ant |
| bounce | `@cellsymphony/behaviors-bounce` | Bouncing particles |
| shapes | `@cellsymphony/behaviors-pulse` | Expanding ring/shape pulses |
| raindrops | `@cellsymphony/behaviors-raindrops` | Falling drops and splash rings |
| dla | `@cellsymphony/behaviors-dla` | Diffusion-limited aggregation |
| glider | `@cellsymphony/behaviors-glider` | Conway glider spawning |

---

## Documentation

- `docs/menu-and-controls-spec.md` — source of truth for menu/control behavior.
- `docs/backlog.md` — requirement status and phase planning.
- `docs/runtime-boundaries.md` — layer responsibilities.
- `docs/engineering-quality-requirements.md` — CI, coverage, and quality gates.
- `docs/implementation-done.md` — implementation summary for the initial 11-algorithm phase.

---

## Sample Library Attribution

The repository `samples/` content is sourced from the Stargate sample pack:

`https://github.com/stargatedaw/stargate-sample-pack`

---

## Hardware Plan

The long-term goal is a standalone hardware device:

- Raspberry Pi Zero 2 W
- 128x128 OLED display
- 5 clickable rotary encoders
- 4 NeoKey buttons
- 8x8 NeoTrellis grid
- PCM5102 I2S audio DAC
- USB-C power
- MIDI in/out where practical

---

## Development

### Prerequisites

- Node.js 20+
- pnpm 9.12.0 (`corepack enable` recommended)
- Rust stable
- Tauri v2 prerequisites for desktop development

### Install

```bash
pnpm install
```

### Run Desktop Simulator

```bash
pnpm --filter @cellsymphony/desktop tauri:dev
```

On Windows you can also use:

```bash
cellSymphony.bat
```

### Verify

```bash
pnpm run build
pnpm run lint
pnpm run typecheck
pnpm run test
pnpm run test:coverage
cargo fmt --all --check
cargo clippy --workspace --exclude cellsymphony-pi --exclude cellsymphony-desktop --exclude midi-io-sidecar --exclude rodio-engine-source --all-targets -- -D warnings
cargo test --workspace --exclude cellsymphony-pi --exclude cellsymphony-desktop --exclude midi-io-sidecar --exclude rodio-engine-source
```

Run focused package tests with:

```bash
pnpm --filter @cellsymphony/platform-core test
pnpm --filter @cellsymphony/desktop test
```

The current platform-core suite has 179+ tests.

### Project Structure

```text
cellSymphony/
├── apps/desktop/                  # Tauri desktop simulator and audio bridge
├── packages/
│   ├── platform-core/             # Core runtime, menu, transport, input routing, simulator frames
│   ├── behavior-api/              # BehaviorEngine interface and registry
│   ├── device-contracts/          # Shared device/display/grid contracts
│   ├── interpretation-core/       # Grid transitions and scan interpretation
│   ├── mapping-core/              # Musical intent to event mapping
│   ├── musical-events/            # Musical event contracts
│   ├── behaviors-*/               # Pluggable behavior packages
│   └── midi-headless-runner/      # Headless MIDI integration support
├── crates/
│   ├── realtime-engine/           # Rust realtime synth/sample/FX engine
│   ├── rodio-engine-source/       # Rodio source wrapper
│   ├── cellsymphony-hal/          # Hardware abstraction layer
│   └── cellsymphony-pi/           # Raspberry Pi app target
├── docs/                          # Specs, backlog, architecture, quality docs
├── hardware/                      # Hardware prototype resources
└── tools/                         # Auxiliary tools
```

---

## License

Copyright (c) 2026 nexxyz (Thomas Steirer).

**Free for personal/non-commercial use**: you may use, copy, modify, and build this software for personal purposes.

**Commercial use or selling hardware devices** requires prior written permission.

To request permission, contact: https://github.com/nexxyz

See the [LICENSE](LICENSE) file for full terms.
