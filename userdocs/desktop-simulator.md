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

One simulator at a time, please: a second copy started on the same saves
says Octessera is already running and closes, so the two can't trample each
other's patches and recordings. The guard only watches one save folder, though.
A copy pointed at a different folder with `OCTESSERA_DESKTOP_STORE_DIR` runs
happily next to the first one, and each keeps to its own saves.

## Make a first sound

The shipped patch already has Layer 1 running `life` into a synth.

1. In **Build**, stay on Layer 1 (or pick `brain` or `raindrops` for it).
2. Click a few tiles on the grid.
3. Press **Space** to start playback.
4. Hold **Ctrl** (the simulator's *Fn*) and click the second tile from the
   bottom in the right-hand grid column to open **Play Pan**. Each row is an
   instrument; click left, center or right on the synth's row (the bottom one)
   to move it across the stereo field.
5. Press **Backspace** to leave Play Pan.

Auto Save is off here too: edits stay in memory until you use
`System > Saves > Default > Save Patch` or save a named preset under
`System > Saves > Library`.

The sampler slot comes with the bundled sample library. To add your own WAVs,
see [Samples](README.md#samples).

## Keyboard and mouse

| Simulator | Instrument control |
|---|---|
| Arrow keys or mouse wheel | Turn *Main* |
| *Enter* or number-pad *Enter* | Click *Main* |
| *Backspace* or *Escape* | *Back* |
| *Space* | *Play* |
| *Shift* | *Shift* |
| *Ctrl* | *Fn* |
| *1 2 3*, *Q W E*, *A S D* | *Aux 1*, *2*, *3*: left / click / right |
| Four rows of eight keys, left *Alt* for the top half | The grid tiles |
| On-screen encoders and grid | The same, with the mouse |

The keys are the same as for a [keyboard on the
instrument](controls-cheat-sheet.md#usb-host-keyboard), and like there they
count by position, so your layout doesn't matter. Left *Alt* won't wander off
into a window menu while the simulator has focus, and the right *Alt*
(*AltGr*) does nothing, even though Windows quietly sends a *Ctrl* press
along with it. If you click another window
while holding keys, the simulator lets go of every tile and button it was
holding.

Everything else works the same as on the hardware, combos included, so the
[controls cheat sheet](controls-cheat-sheet.md) applies here too.
[Behaviors and Play pages](behaviors-and-play.md) explains the moving parts.

## Limits

The simulator plays through your computer's audio output, and its own audio
settings don't change the computer's default output. Unplug the headphones it
was playing through and it tells you the audio output stopped; press Play and
it picks up whatever your computer's default output is now. It doesn't
reproduce the feel of the physical grid and encoders, the LED brightness, the
DAC, power behavior or USB gadget connections, and board-only rows (Bluetooth,
SD Card 2, HDMI Video, Live Show Mode) aren't in its System menu. Recording works and
saves to your Music folder; see [Recording](recording.md).
