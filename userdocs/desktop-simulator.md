# Desktop simulator: a hardware-free first session

You can try Octessera without a PCB, Pi, OLED, samples card, or a soldering
iron. The desktop app is a simulator for the same instrument: it gives you a
quick place to set up musical world-bubbles, listen to what they do, and learn
the controls before deciding whether you want another box on your desk.

## Fastest path: use a release build

1. Open the project's [official releases](https://github.com/nexxyz/octessera/releases).
2. Choose the newest release you intend to try.
3. Download the standalone Windows portable `.exe` when that asset is available,
   save it somewhere you can write to, and launch it. If a future release offers
   a portable `.zip` instead, extract it somewhere writable and launch the
   included `octessera.exe`. Do not run an image or board runtime for this path.

Release pages may also offer macOS or Linux builds. They are useful experiments,
but Windows portable builds are the primary documented simulator path. Treat
other host platforms as unverified until you have run them yourself.

## Contributor path: run from the checkout

Install the workspace once, then start the desktop simulator:

```bash
corepack pnpm install
corepack pnpm --filter @octessera/desktop tauri:dev
```

This path needs the [Tauri host prerequisites](https://v2.tauri.app/start/prerequisites/)
for your operating system, plus the repository's Rust and Node tooling. The host
also needs an available audio output endpoint. No Octessera board, PCB, or
hardware peripherals are required.

## Make a first little patch

1. In **Build**, select a layer and choose `life`, `brain`, or `raindrops`.
2. Draw a few cells on the grid.
3. Open **Shape** and choose a **synth** for the layer.
4. Press **Space** to start playback.
5. Try **Link** to change how motion becomes pitch, velocity, or modulation.
6. Use **Play** to mute, pan, change trigger probability, move the XY controls,
   and try a momentary effect.

Synth is the guaranteed first sound path. A sampler is also available, but it
needs samples supplied by you through the host/sample browser; release packages
do not include a sample library.

The simulator's keyboard and UI controls mirror the hardware ideas. The
[controls cheat sheet](controls-cheat-sheet.md) has the complete map, and
[behaviors and Sparks](behaviors-and-sparks.md) explains the moving parts.

## What this path can and cannot tell you

The simulator is a proper place to explore patches, playback, menus, and
desktop input. It is not a board qualification substitute: it cannot prove
GPIO, encoder or LED behavior, OLED readability, DAC output, power stability,
USB gadget behavior, or the feel and timing of a physical control surface.
For those checks, use the [hardware build and bring-up docs](README.md).
