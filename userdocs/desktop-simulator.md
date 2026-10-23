# Desktop simulator

You can try Octessera without a PCB, a Pi, an OLED, a sample card or a
soldering iron. The simulator runs the same native engine as the hardware, so
it's a good place to set up a few world-bubbles, hear them move and learn the
controls before you commit to any soldering.

## Download and launch

1. Open the project's [release page](https://github.com/nexxyz/octessera/releases).
2. Download the desktop package for your computer: a Windows installer, a
   Windows portable ZIP, or a Linux `.deb` or AppImage.
3. For the portable ZIP, extract it to a writable folder and run
   `octessera.exe`. Otherwise install it and start Octessera from your usual
   application menu.

Coming from a very old version (v0.8.6 or earlier)? Its single `default.json`
can't be opened any more, and the simulator says so at startup. Move that file
out of the folder the message names and start again: you get a fresh default
patch. Anything you want to keep from the old one, set up again and save.

## Make a first sound

The shipped patch already has Layer 1 running `life` into a synth.

1. In **Build**, stay on Layer 1 (or pick `brain` or `raindrops` for it).
2. Click a few cells on the grid.
3. Press **Space** to start playback.
4. Hold **Ctrl** (the simulator's *Fn*) and click the second cell from the
   bottom in the right-hand grid column to open **Play Pan**. Each row is an
   instrument; click left, centre or right on the synth's row (the bottom one)
   to move it across the stereo field.
5. Press **Backspace** to leave Play Pan.

The sampler slot comes with the bundled sample library. To add your own WAVs,
see [Samples](README.md#samples).

## Keyboard and mouse

| Simulator | Instrument control |
|---|---|
| Arrow keys or mouse wheel | Turn *Main* |
| *Enter* | Click *Main* |
| *Backspace* or *Escape* | *Back* |
| *Space* | *Play* |
| *Shift* | *Shift* |
| *Ctrl* | *Fn* |
| On-screen encoders | The three Aux encoders |

Everything else works the same as on the hardware, combos included, so the
[controls cheat sheet](controls-cheat-sheet.md) applies here too.
[Behaviors and Play pages](behaviors-and-play.md) explains the moving parts.

## Limits

The simulator plays through your computer's audio output, and its own audio
settings don't change the computer's default output. It doesn't reproduce the
feel of the physical grid and encoders, the LED brightness, the DAC, power
behaviour or USB gadget connections, and board-only rows (Bluetooth, SD Card 2,
HDMI Video, Live Show Mode) aren't in its System menu. Recording works and
saves to your Music folder; see [Recording](recording.md).
