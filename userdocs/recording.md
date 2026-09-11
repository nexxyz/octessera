# Recording audio and OLED

Octessera can capture the sound it makes, or a small picture-and-sound record
of the instrument. Keep the board powered and the storage card in place while
recording; this is not the moment to discover whether a loose cable is a
feature.

## Start and stop

Open `System > Recording`:

- **Max Time** sets the limit in minutes. It applies to both recording types.
- **Start Audio** writes the final internal stereo mix as a WAV file. It is
  44.1 kHz stereo signed 16-bit PCM. External MIDI-only instruments are not
  included because their sound is made by the outside device.
- **St. Audio+OLED** writes an AVI containing 128x128 MJPEG video at 10 fps
  and PCM audio at 44.1 kHz stereo signed 16-bit. It captures accepted native
  OLED frames; audio is the master clock, so the picture follows the sound.
- **Stop** ends the active take and finalizes the file. Press it deliberately
  when you are finished, even though Max Time will stop a take automatically.

Audio+OLED also stops at its file-size ceiling. Reboot and Shutdown finalize an
active take before continuing, but pressing **Stop** is the sensible habit.
Never remove power, the recording storage, or the board's microSD card while a
take is active.

## Where files land

| Host | Audio WAV | Audio+OLED AVI |
|---|---|---|
| Desktop | `Music/Octessera/recordings/` | `Music/Octessera/screen-recordings/` |
| Raspberry Pi | `/home/pi/recordings/` | `/home/pi/screen-recordings/` |
| Orange Pi | `/var/lib/octessera/recordings/` | `/var/lib/octessera/screen-recordings/` |

## File names and interrupted takes

While a take is active, its file ends in `.partial.wav` or `.partial.avi`.
Once finalization succeeds, an atomic rename makes it the ordinary `.wav` or
`.avi` file.
An `.incomplete.wav` or `.incomplete.avi` means Octessera detected dropped
audio or OLED timeline data. Keep it as useful evidence rather than quietly
renaming it.

Existing takes are never overwritten. If a name is already in use, Octessera
adds a collision suffix. A leftover `.partial` file after an interruption is
not a finished recording; leave it in place until you have checked the logs or
decided it is safe to remove.

## Playing the result

WAV files should be friendly to Audacity, a normal DAW, VLC, `ffplay`, and most
other everyday audio tools. VLC, mpv, and `ffplay` are good first attempts for
the Audio+OLED AVI. `ffprobe` gives a quick stream check:

```sh
ffprobe -hide_banner path/to/octessera-*.avi
```

The AVI uses MJPEG, a practical format rather than a promise that every ancient
media player will love it. If one player refuses, try another common player or
convert the file with a current FFmpeg build.
