# Recording audio and OLED

Octessera can capture its sound, or a small picture-and-sound record of the
instrument. Keep the board powered and the storage card in place while
recording.

## Start and stop

Open `System > Recording`:

- **Max Time** sets the limit in minutes for either recording type.
- **Start Audio** writes the final stereo mix as a WAV file at 44.1 kHz, stereo,
  signed 16-bit PCM. External MIDI-only instruments are not included because
  their sound is made by the outside device.
- **St. Audio+OLED** writes an AVI with 128x128 MJPEG video at 10 fps and 44.1
  kHz stereo signed 16-bit PCM audio. It captures OLED frames, with audio as the
  master clock.
- **Stop** ends the active take and finalizes the file. Use it before rebooting
  or shutting down when possible.

Max Time also stops a take automatically. Never remove power, the recording
storage, or the board's microSD card while a take is active.

## Where files land

| Host | Audio WAV | Audio+OLED AVI |
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
audio tools. Try VLC, mpv, or `ffplay` for an Audio+OLED AVI. If one player
doesn't open it, try another common player or convert it with a current FFmpeg
build.
