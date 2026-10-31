# Recording audio and video

Octessera can record what it plays, either as sound alone or as a little video
of the instrument with the sound attached. Everything lives under
`System > Recording`.

## Start and stop

| Row | What it does |
|---|---|
| **Max Time** | Upper limit for either kind of take, 1 to 120 minutes (default 10). The take stops and saves itself when it's reached. |
| **Start Audio** | Records the final stereo mix as a WAV file: 44.1 kHz, stereo, 16-bit. |
| **Audio+Video** | Records an AVI with the same stereo sound plus a 272 x 128 picture at 20 fps. |
| **Stop** | Ends the take and finishes the file. |

The video is lossless (every frame is a PNG, so every pixel is exactly what
the instrument showed). The OLED sits on the left, the four NeoKeys make a
little column in the middle (Back at the top, Fn at the bottom, each wearing a
tiny keycap glyph), and the 8 x 8 grid fills the right as chunky blocks. LEDs
are drawn at full colour, so a video still reads fine if you play with Grid
Bright turned way down. Audio is the master clock, so picture and sound stay
in step.

Watching a take in VLC? Set `Video > Deinterlace` to **Off**. The video is made
of whole frames, but VLC's automatic deinterlacing splits each one into two
half-frames anyway, and the tiny pixel text comes out torn and smeared. With
deinterlacing off it looks exactly like the screen. Most other players get it
right on their own.

External MIDI instruments aren't in either recording. Their sound is made by
the other device, so record that separately.

What the OLED tells you:

- A small red disc in the bottom right, gently pulsing, for as long as a take
  is running.
- `Recording started` when a take begins, or `Recording is already running` if
  one is.
- `Recording saved` or `Recording incomplete` after you press Stop.
- `Max time: saved` when Max Time ended a clean take.

The OLED still goes to sleep on its usual `System > Display > OLED Sleep`
timer during a take, so a long unattended recording won't burn your menu into
the screen. The video then shows the screen as it really was: asleep. Touch
any control to wake it, or set OLED Sleep to 0 (always on) for a session where
you want the screen in every frame and you're there to play.

Press Stop before rebooting or shutting down. Never pull the power, the
recording storage or the board's microSD card while a take is running.

## Where files land

| Host | Audio (WAV) | Audio+Video (AVI) |
|---|---|---|
| Desktop | `Music/Octessera/recordings/` | `Music/Octessera/screen-recordings/` |
| Raspberry Pi | `/home/pi/recordings/` | `/home/pi/screen-recordings/` |
| Orange Pi | `/var/lib/octessera/recordings/` | `/var/lib/octessera/screen-recordings/` |

On the desktop, `Music` is your system's music folder. On the boards, the
[backup export](data-backup-restore.md) with media included carries your
recordings off the board too.

## Interrupted takes

Existing takes are never overwritten.

- A `.partial.wav` or `.partial.avi` file is a take that never finished, for
  example because the power went.
- An `.incomplete.wav` or `.incomplete.avi` file finished, but Octessera
  noticed dropped audio while recording it.

If the recorder has to skip a video frame, the picture holds a moment longer
and the take is still fine.

## Playing the result

WAV files open in Audacity, any DAW, VLC, `ffplay` and most everyday audio
tools. For an Audio+Video AVI, try VLC, mpv or `ffplay`. Some video editors
don't read PNG-frame AVIs; convert first with FFmpeg:

```sh
ffmpeg -i take.avi -c:v libx264 -crf 12 -pix_fmt yuv444p -c:a aac take.mp4
```

or keep it pixel-exact with `-c:v ffv1` instead.
