# Bluetooth keyboards

A Bluetooth keyboard plays the instrument just like a USB keyboard: the arrow
keys turn *Main*, *Enter* clicks it, *Space* is *Play*, and so on (the full map
is in the [controls cheat sheet](controls-cheat-sheet.md#usb-host-keyboard)).
The nice part is that there is no cable, so on a Raspberry board you can stay in
USB Gadget mode with the computer plugged in and still play from a keyboard on
your lap.

The radio stays off until you ask for it. Nothing listens, nothing searches,
and nothing gets in the way of the audio until you turn it on.

## Turn it on

Go to `System > Bluetooth` and set **Bluetooth** to On. Three more pages appear:

- **Devices** lists keyboards you have paired. A connected one ends in ` - on`.
  Click a row to connect it or disconnect it.
- **Pair New** looks for keyboards while the page is open.
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
- Speakers and headphones don't show up in **Pair New** yet. Bluetooth audio is
  a different beast with its own latency, and it hasn't earned its place in the
  instrument yet.
- The setting lives in the System save, so it survives a reboot.
