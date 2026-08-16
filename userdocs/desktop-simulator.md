# Desktop simulator: a hardware-free first session

You can try Octessera without a PCB, Pi, OLED, samples card, or soldering iron.
The desktop app is a simulator for the same instrument: it gives you a quick
place to set up musical world-bubbles, hear what they do, and learn the
controls before deciding whether you want another box on your desk.

## Release and download first

1. Open the project's [current release page](https://github.com/nexxyz/octessera/releases).
2. Choose the newest release you intend to try.
3. Download the standalone portable EXE when the selected release offers one
   and launch it directly. Extract first only when the selected release offers
   that portable build as a ZIP; then launch the included `octessera.exe` from a
   writable location. Do not run an image or board runtime for this path.

The release page owns current platform and format availability. Ubuntu DEB and
AppImage packages may be listed there; do not assume either format is published
for every release. macOS distribution is paused until it can be properly signed
and notarized.

## Make a first sound

1. In **Build**, select a layer and choose `life`, `brain`, or `raindrops`.
2. Draw a few cells on the grid.
3. Open **Shape** and choose a **synth** for the layer.
4. Press **Space** to start playback.
5. Open one **Play** page, starting with **Play Mix**, to change a layer's
   level. Use **Back** or the normal navigation controls to leave it.

Synth is the guaranteed first sound path. A sampler is also available with the
complete 320-file inventory in release packages; you can add your own samples
through the host/sample browser too.

The simulator's keyboard and UI controls mirror the hardware ideas. The
[controls cheat sheet](controls-cheat-sheet.md) has the complete map, and
[behaviors and Play pages](behaviors-and-sparks.md) explains the moving parts.

The simulator also persists the same three desired-next-boot audio toggles as
the Pi profiles: Jack Audio, USB Audio, and HDMI Audio. It does not change the
desktop host's default audio endpoint. Keep at least one output enabled; the
native menu refuses the final-output-off edit.

The desktop simulator does not provide a USB gadget. USB Audio and USB MIDI in
the shared menu are experimental/local bench-validation policy items, not public
desktop support claims; their defaults remain disabled. For the board policy and
the power warning, see the [release support matrix](release-support.md).

## What this path can and cannot tell you

The simulator is a proper place to explore patches, playback, menus, and
desktop input. It is not a board qualification substitute: it cannot prove
GPIO, encoder or LED behavior, OLED readability, DAC output, power stability,
USB gadget behavior, or the feel and timing of a physical control surface. For
those checks, use the [hardware build and bring-up docs](README.md).

## Contributor checkout path

Use this path when you want to change the application or run the latest source
instead of downloading a release:

```bash
corepack pnpm install
corepack pnpm --filter @octessera/desktop tauri:dev
```

It needs the [Tauri host prerequisites](https://v2.tauri.app/start/prerequisites/)
for your operating system, plus the repository's Rust and Node tooling. The host
also needs an available audio output endpoint. No Octessera board, PCB, or
hardware peripherals are required.
