# Bluetooth keyboards and speakers

A Bluetooth keyboard plays the instrument the same way a USB keyboard does:
the arrow keys turn *Main*, *Enter* clicks it, *Space* is *Play*, and so on
(the full map is in the [controls cheat sheet](controls-cheat-sheet.md#usb-host-keyboard)).
There's no cable, so on the Raspberry you can stay in USB Gadget mode with
the computer plugged in and still play from a keyboard on your lap.

A Bluetooth speaker or pair of headphones can listen along, too.

Bluetooth is on both boards, not in the desktop simulator. The radio stays off
until you ask for it: nothing listens, nothing searches and nothing gets in the
way of the audio.

## Turn it on

Go to `System > Connections > Bluetooth` and set **Bluetooth** to On. A few
more rows appear:

| Row | What it does |
|---|---|
| **Audio Out** | Sends a copy of the music to a connected speaker (see below). |
| **Devices** | Paired keyboards and speakers. A connected one ends in ` - on`. Click a row to connect or disconnect it. |
| **Pair New** | Searches for keyboards and speakers while the page is open. |
| **Forget** | Removes a pairing. |

Turning Bluetooth Off powers the radio down again. Pairings are remembered,
so next time a paired keyboard usually reconnects on its own once you wake it.
The setting lives in the System save, so it survives a reboot.

## Pair a keyboard

1. Set `System > Display > HDMI Video > Mode` to any mode except `Terminal`.
   Keyboards only control the instrument in a graphical HDMI mode, and
   `Terminal` is the shipped default, so this one catches everybody once.
2. Put the keyboard into pairing mode. That's usually a long press on a
   Bluetooth or Fn key combination until a light blinks; the keyboard's manual
   knows the exact trick.
3. Open `System > Connections > Bluetooth > Pair New` and wait a few seconds.
   Keyboards show up as `name [kbd]`. The search stops when you leave the page,
   or after a minute.
4. Click the keyboard's row.
5. Some keyboards ask for a code. The OLED shows six digits: type them on the
   keyboard and press *Enter*. *Back* or a click on *Main* cancels.

When it works, the OLED says `Paired …` and then `Connected …`, and the
keyboard moves to **Devices**. If it says `Pairing failed`, put the keyboard
back into pairing mode and try once more. Keyboards are a bit picky about
timing.

Only the first keyboard Octessera picks up gets the controls.

## Listen on a Bluetooth speaker

A speaker plays along as a monitor: a copy of the same final mix that goes to
the DAC, for listening on the sofa or practising without cables.

1. Put the speaker into pairing mode, open
   `System > Connections > Bluetooth > Pair New` and click its row
   (`name [audio]`). Speakers don't ask for a code.
2. Set `System > Connections > Bluetooth > Audio Out` to On.

Whenever the paired speaker is connected, it plays along. Switch it off or
walk out of range and the monitor stops; the wired outputs carry on
untouched.

- Bluetooth audio is late, usually by a couple of hundred milliseconds. Fine
  for listening, terrible for playing in time, so keep your ears on the wired
  output when you play live.
- The monitor is the least important thing the instrument does. If the board
  gets busy, the speaker hiccups first, so the DAC, USB and HDMI outputs never
  have to.
