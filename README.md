# octessera

Octessera turns cellular automata into music you can play.

It is a collection of little algorithmic musical world-bubbles: small systems you set up, nudge, combine, and occasionally interrupt. You are not programming fixed notes on a piano roll. You are giving tiny rule-based worlds something to do, adding a bit of manual sequencing when you want an anchor, and then playing the result in real time.

Start with the user docs: [`userdocs/README.md`](userdocs/README.md).

Create a dynamic, evolving beat in minutes. Let Conway's Life generate a shifting synth backdrop. Add a drumbeat with a classic grid-style sequencer. Make the drums duck the synth out of the way. Play a lead line live. Then jump into Sparks mode and perform with live effects, change trigger probability, and use the XY pad and mixer controls to build something that you and octessera found together.

It is easy to start with, but deep. It rewards exploration and experimentation. Small changes to the grid can become rhythms, melodies, modulation, texture, or surprise.

The intended way of using it is a DIY standalone hardware instrument based on a Raspberry Pi. There is, however, a fully implemented desktop app that serves simulator for building, testing, and playing the same instrument on a computer - but it is also quite usable and effective, if you don't want another piece of gear to clutter your valuable desk real estate.

## What You Can Make

- **Generative synth patterns** from Life, Brain, Ant, Bounce, Raindrops, DLA, and other grid behaviors.
- **Hands-on drum patterns** with a sequencer-style grid and sample slots.
- **Layered arrangements** with up to eight layers/instruments.
- **Live leads** with the Keys behavior.
- **Evolving modulation** where grid motion changes pitch, filter, velocity, effects, and other parameters.
- **Internal synth and sampler sounds**, plus external MIDI output.
- **Performance scenes** with Sparks mode: mix, pan, trigger probability, XY modulation, and momentary effects.
- **Happy accidents** from systems that keep moving after you set them in motion.

## A Quick Session

1. Pick a layer in **1: Worlds** and choose a behavior such as `life`, `brain`, or `raindrops`.
2. Draw or seed a few cells on the grid.
3. Press **Space** to start playback.
4. Go to **3: Tones** and choose a synth, sampler or even MIDI output for the layer.
5. Add a sequencer layer for drums, have it play a sampler with your preferred drum samples (we even provide a small sample library for you!)
6. Then route the synth through an FX bus with ducking so the beat opens space in the mix.
7. Switch another layer to **Keys** and play a lead line live.
8. Hold **Fn** for navigation and use the right grid column to enter **Sparks** mode.
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
| Navigate layers | Fn + left grid column | Ctrl + left grid column |
| Navigate Sparks pages | Fn + right grid column | Ctrl + right grid column |
| Access alternate aux binding | Fn + aux press | Fn + aux UI control |

The friendly control guide starts at [`userdocs/controls-cheat-sheet.md`](userdocs/controls-cheat-sheet.md). The canonical control/menu spec is [`docs/menu-and-controls-spec.md`](docs/menu-and-controls-spec.md).

## Main Pages

- **1: Worlds** — choose the active layer's behavior and edit its grid state.
- **2: Pulses** — decide how grid motion becomes notes, velocity, filters, probability, and modulation.
- **3: Tones** — choose synth, sampler, MIDI, mixer routing, FX buses, and global FX.
- **4: Sparks** — perform with mix, pan, trigger probability, XY, and momentary effects.
- **System** — presets, default/factory actions, sound, MIDI, brightness, sleep, and help.

## Build The Hardware

The intended build is a DIY standalone instrument around a custom PCB, Raspberry Pi Zero 2 W, NeoTrellis grid, NeoKey controls, OLED, DAC, and printed enclosure.

Start with the full assembly guide:

- [`userdocs/hardware/assembly-manual.md`](userdocs/hardware/assembly-manual.md) — BOM, PCB ordering, soldering, module setup, Pi flashing, first power-on, and enclosure assembly.

Related references:

1. [`userdocs/hardware/pinout-and-connections.md`](userdocs/hardware/pinout-and-connections.md) — wiring, pin ownership, buses, and hardware source of truth.
2. [`userdocs/hardware/enclosure.md`](userdocs/hardware/enclosure.md) — case, port access, print notes, and power rule.
3. [`docs/menu-and-controls-spec.md`](docs/menu-and-controls-spec.md) — runtime controls, menus, overlays, and display behavior.

## Desktop Simulator

The easiest way to play with this system is to download and launch the portable Windows EXE attached to the official releases.

Releases may also include macOS and Linux desktop simulator builds. Those are currently untested; treat them as experimental until someone has done the boring part and actually run them on those systems.

It allows you try out Octessera without any special hardware.

## Documentation Map

Primary user docs:

- [`userdocs/README.md`](userdocs/README.md): start here for build, bring-up, controls, printable sheets, and references.
- [`userdocs/hardware/assembly-manual.md`](userdocs/hardware/assembly-manual.md): hardware BOM, soldering, first power-on, and enclosure assembly.
- [`userdocs/hardware/pinout-and-connections.md`](userdocs/hardware/pinout-and-connections.md): Pi wiring, bus allocation, logical input mapping, and hardware source of truth.
- [`userdocs/hardware/enclosure.md`](userdocs/hardware/enclosure.md): enclosure ports, power rule, printing notes, and mechanical strategy.
- [`userdocs/controls-cheat-sheet.md`](userdocs/controls-cheat-sheet.md): hardware and simulator controls.
- [`userdocs/behaviors-and-sparks.md`](userdocs/behaviors-and-sparks.md): behavior overview and Sparks page reference.
- [`userdocs/print/quick-reference.pdf`](userdocs/print/quick-reference.pdf): two-page printable controls, behaviors, Sparks, and signal-flow sheet.

Canonical specs:

- [`docs/menu-and-controls-spec.md`](docs/menu-and-controls-spec.md): authoritative controls, menu structure, overlays, persistence, and display behavior.
- [`docs/menu-tree-spec.md`](docs/menu-tree-spec.md): canonical menu tree.

Secondary contributor docs:

- [`docs/runtime-boundaries.md`](docs/runtime-boundaries.md): crate/host responsibilities and dependency boundaries.
- [`docs/development-workflows.md`](docs/development-workflows.md): current development, build, verification, and capability-generation commands.
- [`docs/engineering-quality-requirements.md`](docs/engineering-quality-requirements.md): current quality gates and definition of done.
- [`docs/open-work.md`](docs/open-work.md): current actionable work only.

## Samples

Repository sample content is sourced from the [Stargate sample pack](https://github.com/stargatedaw/stargate-sample-pack).

## Hardware model attributions

The standoff STL models in `release-artifacts/enclosure/` are based on [Stackable PCB Standoff by theduckom](https://www.printables.com/model/163087-stackable-pcb-standoff), licensed under [Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/).

## License

Copyright (c) 2026 nexxyz.

Free for personal/non-commercial use: you may use, copy, modify, and build this software for personal purposes.

Commercial use or selling hardware devices requires prior written permission.

To request permission, contact: https://github.com/nexxyz

See [LICENSE](LICENSE) for full terms.
