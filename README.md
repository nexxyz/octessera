# octessera

Octessera turns cellular automata into music you can play.

It is a collection of little algorithmic musical world-bubbles: small systems you set up, nudge, combine, and occasionally interrupt. Instead of programming fixed notes on a piano roll or tracker-track, you are giving tiny rule-based worlds something to do, adding a bit of manual sequencing when you want an anchor, and then playing the result in real time.

Start with the user docs: [`userdocs/README.md`](userdocs/README.md). If you
have no hardware yet, use the [hardware-free desktop simulator path](userdocs/desktop-simulator.md).

Create a dynamic, evolving beat in minutes. Let Conway's Life generate a shifting synth backdrop. Add a drumbeat with a classic grid-style sequencer. Make the drums duck the synth out of the way. Play a lead line live. Then open a Play page and perform with Play FX, change trigger probability, and use the XY pad and mixer controls to build something that you and octessera found together.

It is easy to start with, but deep. It rewards exploration and experimentation. Small changes to the grid can become rhythms, melodies, modulation, texture, or surprise.

The intended way of using it is a DIY standalone hardware instrument built around one of two fixed compute-board paths: Raspberry Pi Zero 2 W or Orange Pi Zero 2W. There is also a fully implemented desktop simulator for building, testing, and playing the same instrument on a computer.

## What You Can Make

- **Generative synth patterns** from Life, Brain, Ant, Bounce, Raindrops, DLA, and other grid behaviors.
- **Hands-on drum patterns** with a sequencer-style grid and sample slots.
- **Layered arrangements** with up to eight layers/instruments.
- **Live leads** with the Keys behavior.
- **Evolving modulation** where grid motion changes pitch, filter, velocity, effects, and other parameters.
- **Internal synth and sampler sounds**, plus external MIDI output.
- **Performance scenes** with Play pages: mix, pan, trigger probability, XY modulation, and momentary effects.
- **Happy accidents** from systems that keep moving after you set them in motion.

## A Quick Session

1. Pick a layer in **Build** and choose a behavior such as `life`, `brain`, or `raindrops`.
2. Draw or seed a few cells on the grid.
3. Press **Space** to start playback.
4. Go to **Shape** and choose a synth, sampler or even MIDI output for the layer.
5. Add a sequencer layer for drums and have it play a sampler with samples you provide.
6. Then route the synth through an FX bus with ducking so the beat opens space in the mix.
7. Switch another layer to **Keys** and play a lead line live.
8. Hold **Fn** for navigation and use the right grid column to enter **Play** pages.
9. Perform: mute, pan, change probability, move XY controls, and punch in live effects.

You can treat it like a algorithmic groovebox, a generative sketchpad, or a small experimental performance instrument.

## Controls

The standalone hardware uses one clickable main encoder, three clickable aux encoders, four keys with LEDs, an 8x8 grid, and a small OLED. The enclosure is still an active v21 test-fit design; the desktop simulator mirrors those controls with keyboard and UI inputs.

| Action | Hardware | Desktop |
|---|---|---|
| Move or change a value | Main encoder turn | Arrow keys |
| Enter, edit, or confirm | Main encoder press | Enter |
| Back or leave edit mode | Back button | Backspace / Esc |
| Play / pause | Space button | Space |
| Emergency stop | Shift + Space | Shift + Space |
| Clear active grid | Shift + Back | Shift + Backspace / Shift + Esc |
| Navigate layers | Fn + left grid column | Ctrl + left grid column |
| Navigate Play pages | Fn + right grid column | Ctrl + right grid column |
| Access alternate aux binding | Fn + aux press | Fn + aux UI control |

The friendly control guide starts at [`userdocs/controls-cheat-sheet.md`](userdocs/controls-cheat-sheet.md). The canonical control/menu spec is [`docs/menu-and-controls-spec.md`](docs/menu-and-controls-spec.md).

## Main Pages

- **Build** — choose the active layer's behavior and edit its grid state.
- **Link** — decide how grid motion becomes notes, velocity, filters, probability, and modulation.
- **Shape** — choose synth, sampler, MIDI, mixer routing, FX buses, and global FX.
- **Play** — perform with mix, pan, trigger probability, XY, and momentary Play FX.
- **System** — presets, default/factory actions, sound, MIDI, brightness, sleep, and help.

## Build The Hardware

The intended build is a DIY standalone instrument around a custom PCB, one of
the two supported compute boards, NeoTrellis grid, NeoKey controls, OLED, DAC,
and printed enclosure. Choose either Raspberry Pi Zero 2 W or Orange Pi Zero
2W; their images, pinouts, port roles, and physical checks are not
interchangeable.

Parts cost follows the current BOM, suppliers, shipping, taxes, and printing.
Check those inputs before ordering; the project does not promise a fixed total.

Start with the full assembly guide:

- [`userdocs/hardware/assembly-manual.md`](userdocs/hardware/assembly-manual.md) — BOM, PCB ordering, soldering, module setup, board flashing, first power-on, and enclosure assembly.

Related references:

1. [`userdocs/hardware/pinout-and-connections.md`](userdocs/hardware/pinout-and-connections.md) — wiring, pin ownership, buses, and hardware source of truth.
2. [`userdocs/hardware/enclosure.md`](userdocs/hardware/enclosure.md) — case, port access, print notes, and mechanical status.
3. [`docs/menu-and-controls-spec.md`](docs/menu-and-controls-spec.md) — runtime controls, menus, overlays, and display behavior.
4. [`userdocs/hardware/safety-and-power.md`](userdocs/hardware/safety-and-power.md) — power input, USB backfeed, orientation, and fit stop conditions.
5. [`userdocs/troubleshooting.md`](userdocs/troubleshooting.md) — symptom router for the desktop and both board paths.

## Desktop Simulator

The easiest way to play with this system is the [hardware-free desktop simulator path](userdocs/desktop-simulator.md),
which starts with the portable Windows build on the [current release page](https://github.com/nexxyz/octessera/releases).

Windows is the documented desktop release path. The current release page also
lists Ubuntu DEB and AppImage builds when available. macOS distribution is
paused until it can be properly signed and notarized. You can still run the
simulator from the checkout when you want to tinker.

It lets you try out Octessera without any special hardware. The simulator is
excellent for musical and desktop-runtime exploration, but it does not qualify
the physical board, controls, display, DAC, power, or USB paths.

## Documentation Map

Primary user docs:

- [`userdocs/README.md`](userdocs/README.md): start here for build, bring-up, controls, printable sheets, and references.
- [`userdocs/desktop-simulator.md`](userdocs/desktop-simulator.md): start a hardware-free desktop session and understand its limits.
- [`userdocs/hardware/assembly-manual.md`](userdocs/hardware/assembly-manual.md): hardware BOM, soldering, first power-on, and enclosure assembly.
- [`userdocs/hardware/pinout-and-connections.md`](userdocs/hardware/pinout-and-connections.md): Raspberry wiring, bus allocation, logical input mapping, and Orange routing warning.
- [`userdocs/hardware/raspberry-pi-first-boot.md`](userdocs/hardware/raspberry-pi-first-boot.md): Raspberry image, UART/SW3, OLED, and saved-settings behavior.
- [`userdocs/hardware/orange-pi-first-boot.md`](userdocs/hardware/orange-pi-first-boot.md): Orange production image, setup, samples, and bench recovery.
- [`userdocs/hardware/safety-and-power.md`](userdocs/hardware/safety-and-power.md): the concise safety owner page.
- [`userdocs/troubleshooting.md`](userdocs/troubleshooting.md): symptom-first recovery links.
- [`userdocs/hardware/enclosure.md`](userdocs/hardware/enclosure.md): enclosure ports, power rule, printing notes, and mechanical strategy.
- [`userdocs/controls-cheat-sheet.md`](userdocs/controls-cheat-sheet.md): hardware and simulator controls.
- [`userdocs/behaviors-and-sparks.md`](userdocs/behaviors-and-sparks.md): behavior catalog and Play page reference.
- [`userdocs/print/quick-reference.pdf`](userdocs/print/quick-reference.pdf): two-page printable controls, behaviors, Play, and signal-flow sheet.

Canonical specs:

- [`docs/menu-and-controls-spec.md`](docs/menu-and-controls-spec.md): authoritative controls, menu structure, overlays, persistence, and display behavior.
- [`docs/menu-tree-spec.md`](docs/menu-tree-spec.md): canonical menu tree.

Contributor/reference docs:

- [`docs/runtime-boundaries.md`](docs/runtime-boundaries.md): crate/host responsibilities and dependency boundaries.
- [`docs/development-workflows.md`](docs/development-workflows.md): contributor workflow index and shared verification/source-of-truth commands.
- [`docs/workflows/desktop-development.md`](docs/workflows/desktop-development.md): desktop simulator, builds, and hardware-free checks.
- [`docs/workflows/pi-development-and-profiling.md`](docs/workflows/pi-development-and-profiling.md): Pi host builds and profiling.
- [`docs/workflows/image-construction-and-proof.md`](docs/workflows/image-construction-and-proof.md): board image construction and proof.
- [`docs/workflows/release-assembly.md`](docs/workflows/release-assembly.md): release asset and populated-draft contract.
- [`docs/workflows/deployment.md`](docs/workflows/deployment.md): board deployment and hardware debug loops.
- [`docs/engineering-quality-requirements.md`](docs/engineering-quality-requirements.md): current quality gates and definition of done.
- [`docs/open-work.md`](docs/open-work.md): current actionable work only.

## Samples

The bundled sample library contains 320 media files. The sampler-loadable default library contains 318 WAV files; two AIFF files remain outside the WAV-only browser/decoder. The portable desktop package and both production-image constructors stage the complete library. The technical [`samples/MANIFEST.tsv`](samples/MANIFEST.tsv) records each file's path, size, and SHA-256 digest. The canonical default patch uses sampler-loadable WAV paths, and user-supplied samples remain supported.

The concise sample acknowledgement is [`samples/SOURCE.md`](samples/SOURCE.md); the pack's CC0 text is retained in [`samples/upstream/LICENSE`](samples/upstream/LICENSE).

## Hardware model attributions

See [`hardware/ATTRIBUTIONS.md`](hardware/ATTRIBUTIONS.md) for the standoff, module-footprint, Raspberry Pi CAD, KiCad, and hardware-reference notices.

AI assistance was used during the creation of Octessera.

## License

Copyright (c) 2026 Thomas Steirer (nexxyz).

Free for personal/non-commercial use: you may use, copy, modify, and build this software for personal purposes.

Commercial use or selling hardware devices requires prior written permission.

To request permission, contact: https://github.com/nexxyz

See [LICENSE](LICENSE) for full terms.

## Licensing and release review

- [`NOTICE`](NOTICE) — compact notice for standalone release archives.
- [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) — reviewed known-material notices; not an exhaustive dependency inventory.
- [`docs/release-licensing.md`](docs/release-licensing.md) — attribution and future public-image source/licensing review.
