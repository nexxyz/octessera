# Octessera

Octessera turns cellular automata into music you can play.

Create a dynamic, evolving beat in minutes. Let Conway's Life generate a shifting synth backdrop. Add a drumbeat with a classic grid-style sequencer. Make the drums duck the synth out of the way. Play a lead line live. Then jump into Dance mode and perform with live effects, change note probability, and use the XY pad and mixer controls to build up to a massive drop.

It is easy to start with, but deep. It rewards exploration and experimentation. Small changes to the grid can become rhythms, melodies, modulation, texture, or surprise.

The intended way of using it is a DIY standalone hardware instrument based on a Raspberry Pi. There is, however, a fully implemented desktop app that serves simulator for building, testing, and playing the same instrument on a computer - but it is also quite usable and effective, if you don't want another piece of gear to clutter your valuable desk real estate.

## What You Can Make

- **Generative synth patterns** from Life, Brain, Ant, Bounce, Raindrops, DLA, and other grid behaviors.
- **Hands-on drum patterns** with a sequencer-style grid and sample slots.
- **Layered arrangements** with up to eight parts/instruments.
- **Live leads** with the Keys behavior.
- **Evolving modulation** where grid motion changes pitch, filter, velocity, effects, and other parameters.
- **Internal synth and sampler sounds**, plus external MIDI output.
- **Performance scenes** with Dance mode: mix, pan, trigger probability, XY modulation, and momentary effects.
- **Happy accidents** from systems that keep moving after you set them in motion.

## A Quick Session

1. Pick a part in **L1: Life** and choose a behavior such as `life`, `brain`, or `raindrops`.
2. Draw or seed a few cells on the grid.
3. Press **Space** to start playback.
4. Go to **L3: Voice** and choose a synth, sampler or even MIDI output for the part.
5. Add a sequencer part for drums, have it play a sampler with your preferred drum samples (we even provide a small sample library for you!)
6. Then route the synth through an FX bus with ducking so the beat opens space in the mix.
7. Switch another part to **Keys** and play a lead line live.
8. Hold **Fn** for navigation and use the right grid column to enter **Dance** mode.
9. Perform: mute, pan, change probability, move XY controls, and punch in live effects.

You can treat it like a algorithmic groovebox, a generative sketchpad, or a small experimental performance instrument.

## Controls

The hardware (still under design) uses one clickable main encoder, three clickable aux encoders, four keys with LEDs, an 8x8 grid and a small OLED. The desktop simulator mirrors those controls with keyboard and UI inputs.

| Action | Hardware | Desktop |
|---|---|---|
| Move or change a value | Main encoder turn | Arrow keys |
| Enter, edit, or confirm | Main encoder press | Enter |
| Back or leave edit mode | Back button | Backspace / Esc |
| Play / pause | Space button | Space |
| Emergency stop | Shift + Space | Shift + Space |
| Clear active grid | Shift + Back | Shift + Backspace / Shift + Esc |
| Navigate parts | Fn + left grid column | Ctrl + left grid column |
| Navigate Dance pages | Fn + right grid column | Ctrl + right grid column |
| Access alternate aux binding | Fn + aux press | Fn + aux UI control |

The full control and menu reference is [`docs/menu-and-controls-spec.md`](docs/menu-and-controls-spec.md).

## Main Pages

- **L1: Life** — choose the active part's behavior and edit its grid state.
- **L2: Sense** — decide how grid motion becomes notes, velocity, filters, probability, and modulation.
- **L3: Voice** — choose synth, sampler, MIDI, mixer routing, FX buses, and global FX.
- **L4: Dance** — perform with mix, pan, trigger probability, XY, and momentary effects.
- **System** — presets, default/factory actions, sound, MIDI, brightness, sleep, and help.

## Build The Hardware

The intended build is a DIY standalone instrument around a custom PCB, Raspberry Pi Zero 2 W, NeoTrellis grid, NeoKey controls, OLED, DAC, and printed enclosure.

Start with the full assembly guide:

- [`hardware/docs/assembly-manual.md`](hardware/docs/assembly-manual.md) — BOM, PCB ordering, soldering, module setup, Pi flashing, first power-on, and enclosure assembly.

Related references:

1. [`hardware/docs/pinout-and-connections.md`](hardware/docs/pinout-and-connections.md) — wiring, pin ownership, buses, and hardware source of truth.
2. [`hardware/enclosure/README.md`](hardware/enclosure/README.md) — case, port access, print notes, and power rule.
3. [`hardware/docs/pi-bring-up.md`](hardware/docs/pi-bring-up.md) — Pi OS setup, preflight, build/deploy, and bring-up checklist.
4. [`docs/menu-and-controls-spec.md`](docs/menu-and-controls-spec.md) — runtime controls, menus, overlays, and display behavior.

## Desktop Simulator

The easiest way to play with this system is to just to download and launch the portable Windows EXE that is attached to the official releases.

It allows you try out Octessera without any special hardware.

You can also launch it in a different way:

```bash
corepack pnpm install
corepack pnpm --filter @octessera/desktop tauri:dev
```

To create a portable Windows build yourself:

```bash
corepack pnpm --filter @octessera/desktop tauri:build:exe
```

## For Contributors

Most users should not need this section. It is here for people changing the software or hardware docs.

Octessera keeps musical behavior in the native Rust runtime so the desktop simulator and Pi hardware stay aligned. TypeScript is only the desktop display/input layer and shared contracts.

Repository layout:

```text
octessera/
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
├── config/                       # Generated platform default configs and source overrides
├── docs/                         # Menu spec and secondary contributor docs
├── hardware/                     # Hardware source files, enclosure generator, and build docs
├── release-artifacts/            # End-user binaries, fabrication exports, and print files
└── tools/                        # Repository maintenance tools
```

Run the standard checks:

```bash
corepack pnpm run typecheck
corepack pnpm -r test
corepack pnpm -r lint
corepack pnpm -r format:check
cargo fmt --all --check
cargo test -p platform-core -p playback-runtime -p realtime-engine -p octessera-desktop
cargo clippy -p platform-core -p playback-runtime -p realtime-engine -p octessera-desktop --all-targets -- -D warnings
```

Build desktop release artifacts:

```bash
corepack pnpm --filter @octessera/desktop tauri:build
```

Build the Pi app with host stubs:

```bash
cargo build -p octessera-pi
```

See `docs/development-workflows.md` for complete contributor build, verification, capability-generation, and Pi hardware build notes.

## Documentation Map

Primary user and hardware docs:

- [`hardware/docs/pinout-and-connections.md`](hardware/docs/pinout-and-connections.md): Pi wiring, bus allocation, logical input mapping, and hardware source of truth.
- [`hardware/docs/assembly-manual.md`](hardware/docs/assembly-manual.md): hardware BOM, soldering, first power-on, and enclosure assembly.
- [`hardware/enclosure/README.md`](hardware/enclosure/README.md): enclosure ports, power rule, printing notes, and mechanical strategy.
- [`hardware/docs/pi-bring-up.md`](hardware/docs/pi-bring-up.md): Pi OS setup, preflight, build/deploy, bring-up, diagnostics, and update plan.
- [`docs/menu-and-controls-spec.md`](docs/menu-and-controls-spec.md): authoritative controls, menu structure, overlays, persistence, and display behavior.

Secondary contributor docs:

- [`docs/runtime-boundaries.md`](docs/runtime-boundaries.md): crate/host responsibilities and dependency boundaries.
- [`docs/development-workflows.md`](docs/development-workflows.md): current development, build, verification, and capability-generation commands.
- [`docs/engineering-quality-requirements.md`](docs/engineering-quality-requirements.md): current quality gates and definition of done.
- [`docs/open-work.md`](docs/open-work.md): current actionable work only.

## Samples

Repository sample content is sourced from the Stargate sample pack:

```text
https://github.com/stargatedaw/stargate-sample-pack
```

## Hardware model attributions

The standoff STL models in `release-artifacts/enclosure/` are based on Stackable PCB Standoff by theduckom, licensed under Creative Commons Attribution 4.0 International:

```text
https://www.printables.com/model/163087-stackable-pcb-standoff
https://creativecommons.org/licenses/by/4.0/
```

## License

Copyright (c) 2026 nexxyz.

Free for personal/non-commercial use: you may use, copy, modify, and build this software for personal purposes.

Commercial use or selling hardware devices requires prior written permission.

To request permission, contact: https://github.com/nexxyz

See `LICENSE` for full terms.
