# Bluetooth keyboards and speakers

A Bluetooth keyboard plays the instrument just like a USB keyboard: the arrow
keys turn *Main*, *Enter* clicks it, *Space* is *Play*, and so on (the full map
is in the [controls cheat sheet](controls-cheat-sheet.md#usb-host-keyboard)).
The nice part is that there is no cable, so on a Raspberry board you can stay in
USB Gadget mode with the computer plugged in and still play from a keyboard on
your lap.

The radio stays off until you ask for it. Nothing listens, nothing searches,
and nothing gets in the way of the audio until you turn it on.

## Turn it on

Go to `System > Bluetooth` and set **Bluetooth** to On. A few more rows appear:

- **Audio Out** sends a copy of the music to a paired speaker (see below).
- **Devices** lists keyboards and speakers you have paired. A connected one
  ends in ` - on`. Click a row to connect it or disconnect it.
- **Pair New** looks for keyboards and speakers while the page is open.
- **Forget** removes a pairing.

Turning Bluetooth Off powers the radio down again. Pairings are remembered, so
next time a paired keyboard usually reconnects on its own once you wake it up.

## Pair a keyboard

1. Put the keyboard into pairing mode. This is usually a long press on a
   Bluetooth or Fn key combination until a light blinks; the keyboard's manual
   knows the exact trick.
2. Open `System > Bluetooth > Pair New` and wait a few seconds. Keyboards show
   up as `name [kbd]` as they are found. The search stops when you leave the
   page, or after a minute.
3. Click the keyboard's row.
4. Some keyboards ask for a code. The OLED shows six digits: type them on the
   Bluetooth keyboard and press *Enter*. *Back* or a click on *Main* cancels.

When it works, the OLED says `Paired …` and then `Connected …`, and the keyboard
moves to **Devices**. If it says `Pairing failed`, put the keyboard back into
pairing mode and try once more; keyboards are a bit picky about timing.

## Good to know

- The keyboard follows the same rule as a USB keyboard: it only controls the
  instrument while `System > HDMI Video > Mode` is a graphical mode, not
  `Terminal`.
- Only the first keyboard Octessera picks up gets the controls.
- The setting lives in the System save, so it survives a reboot.

## Listen on a Bluetooth speaker

A Bluetooth speaker or pair of headphones can play along as a **monitor**: a
copy of the same final mix that goes to the DAC, for listening on the sofa or
practising without cables.

1. Put the speaker into pairing mode, open `System > Bluetooth > Pair New`, and
   click its row (`name [audio]`). Speakers don't ask for a code.
2. Set `System > Bluetooth > Audio Out` to On.

That's it: whenever the paired speaker is connected, it plays along. Turn the
speaker off or walk out of range and the monitor simply stops; the wired
outputs carry on untouched.

Two honest warnings:

- Bluetooth audio is **late**, usually by a couple of hundred milliseconds. It
  is fine for listening and terrible for playing in time, so keep your ears on
  the wired output when you play live.
- The monitor is the least important thing the instrument does. If the board
  gets busy, the speaker hiccups first, so the notes on the DAC, USB and HDMI
  outputs never have to.
