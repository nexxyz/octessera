# Recording audio and video

Octessera can capture its sound, or a small picture-and-sound record of the
instrument. Keep the board powered and the storage card in place while
recording.

## Start and stop

Open `System > Recording`:

- **Max Time** sets the limit in minutes for either recording type.
- **Start Audio** writes the final stereo mix as a WAV file at 44.1 kHz, stereo,
  signed 16-bit PCM. External MIDI-only instruments are not included because
  their sound is made by the outside device.
- **Audio+Video** writes an AVI with the same stereo sound plus a 272x128
  lossless picture at 20 fps (each frame is a PNG, so every pixel is exactly
  what the instrument showed): the OLED on the left, the four NeoKeys as a little
  column in the middle (Back at the top, Fn at the bottom, each wearing a tiny
  black keycap glyph), and the 8x8 grid on the right as chunky blocks. The
  LEDs are shown at full color, so a video still reads fine if you play with
  Grid Bright turned way down. Audio is the master clock, so picture and sound
  stay in step.
- A successful start shows **Recording started**. Starting again while a take is
  active shows **Recording is already running**.
- While a take is active, the OLED stays awake temporarily. This does not change
  the saved **OLED Sleep** setting; normal idle sleep timing restarts after
  recording ends.
- **Stop** ends the active take and finalizes the file. Manual stop shows
  **Recording saved** or **Recording incomplete**. A clean max-time completion
  shows **Max time: saved**. Use Stop before rebooting or shutting down when
  possible.

Max Time also stops a take automatically. Never remove power, the recording
storage, or the board's microSD card while a take is active.

## Where files land

| Host | Audio WAV | Audio+Video AVI |
|---|---|---|
| Desktop | `Music/Octessera/recordings/` | `Music/Octessera/screen-recordings/` |
| Raspberry Pi | `/home/pi/recordings/` | `/home/pi/screen-recordings/` |
| Orange Pi | `/var/lib/octessera/recordings/` | `/var/lib/octessera/screen-recordings/` |

## Interrupted takes

An interrupted take may leave a `.partial.wav` or `.partial.avi` file. It is
not finished. An `.incomplete.wav` or `.incomplete.avi` file means Octessera
detected dropped audio or OLED timeline data, so treat it as an incomplete
recording. Existing takes are not overwritten.

## Playing the result

WAV files work with Audacity, a normal DAW, VLC, `ffplay`, and most everyday
audio tools. Try VLC, mpv, or `ffplay` for an Audio+Video AVI. Some video
editors don't read PNG-frame AVIs directly; convert first with FFmpeg, e.g.
`ffmpeg -i take.avi -c:v libx264 -crf 12 -pix_fmt yuv444p -c:a aac take.mp4`,
or keep it pixel-exact with `-c:v ffv1`.
