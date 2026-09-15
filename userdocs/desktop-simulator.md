# Desktop simulator

You can try Octessera without a PCB, Pi, OLED, sample card, or soldering iron.
The simulator is a quick place to set up musical world-bubbles, hear them move,
and learn the controls.

## Download and launch

1. Open the project's [release page](https://github.com/nexxyz/octessera/releases).
2. Download the desktop package for your computer.
3. For a portable ZIP, extract it to a writable folder and launch
   `octessera.exe`. For an installer or Linux package, install it and launch
   Octessera from your usual application menu.

## Make a first sound

1. In **Build**, select a layer and choose `life`, `brain`, or `raindrops`.
2. Draw a few cells on the grid.
3. Open **Shape** and choose a **synth** for the layer.
4. Press **Space** to start playback.
5. Open **Play Mix** to change a layer's level. Use **Back** or the normal
   navigation controls to leave it.

Start with a **synth** for the simplest first sound. A sampler is also
available with the default library's 318 WAV files; two AIFF files sit outside
the WAV-only browser. You can add your own samples through the sample browser.

The simulator's keyboard and UI controls mirror the hardware. The
[controls cheat sheet](controls-cheat-sheet.md) has the complete map, and
[behaviors and Play pages](behaviors-and-sparks.md) explains the moving parts.

## Limits

The simulator uses the computer's available audio output. Its saved audio
settings do not change the computer's default output. It does not reproduce the
physical grid, encoders, LEDs, OLED, DAC, power behavior, or USB gadget
connections.
