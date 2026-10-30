# USB roles

Use this page after [flash and first boot](flash-and-first-boot.md), when you
want to plug the instrument into a computer (as a USB audio/MIDI device) or plug
a USB device, such as a keyboard, into the instrument.

> **Power first:** Keep the instrument powered through the enclosure's USB-C
> power breakout. A USB data cable is not a second power supply. Never connect
> two power supplies, and never rely on a cable or adapter to prevent back-feed.
> The details are in [safety and power](safety-and-power.md).

## Raspberry Pi

The Raspberry Pi's USB data port can do one of two jobs, and you can swap
between them while the music keeps going:

| Role | What it does | Connect |
|---|---|---|
| **Gadget** (default) | The instrument appears to a computer as USB Audio, USB MIDI and, on request, the SD2 card transfer. | The Pi's micro-USB data port to a computer's USB-A port, with a USB-A-to-micro-USB cable. |
| **Host** | The instrument talks to USB devices such as a keyboard or a MIDI controller. | A USB device through a micro-USB OTG adapter. |

For the computer connection, use a USB-A port on the computer or a hub. Avoid
USB-C-to-USB-C cables and USB Power Delivery chargers or ports for this.

### Switch to Host

1. Unplug the computer's USB cable. Left plugged in, the computer and the
   instrument would both try to run the cable, and nothing on it gets recognised.
2. Open `System > Connections > USB Role` and select **Host**. USB Audio and
   USB MIDI switch themselves off on the way.
3. The OLED says `Applying...` and then `USB ready`, usually within a couple of
   seconds.
4. Connect your device through the OTG adapter. A powered hub is fine only if
   it cannot back-feed power into the instrument.

In Host mode, USB Audio, USB MIDI and starting new SD2 transfers are
unavailable. If an SD2 transfer is still running, stop it first; the instrument
won't switch to Host in the middle of one.

### Switch back to Gadget

1. Unplug the USB device.
2. Select `System > Connections > USB Role > Gadget` and wait for `USB ready`.
3. Connect the data port to a computer with the USB-A-to-micro-USB cable.
4. Turn USB Audio or USB MIDI back on yourself if you want them. Switching back
   to Gadget does not re-enable the features that Host mode switched off.

If the OLED ever says `USB apply failed, reboot to apply`, the new setting is
saved anyway, and a reboot puts it in place.

## Orange Pi

The Orange Pi has two USB-C data ports with fixed jobs, so there is no
`System > Connections > USB Role` setting. Look for the dots next to the USB
icons on the south wall of the case:

| Opening | Port | Connect |
|---|---|---|
| **USB, two dots** | USB0, the computer port | A computer, with a USB-C-to-USB-C cable. The instrument shows up as USB Audio and USB MIDI. |
| **USB, one dot** | USB1, the host port | A keyboard or a MIDI controller. |

Never plug a computer into the one-dot port: both ends would try to supply
power, and the cable can get hot. USB data is optional for normal use.

Two habits that keep the Orange happy:

- **Unplug the computer before you power-cycle the instrument.** USB0 also
  feeds the board, so a plugged-in computer half-powers it and it hangs on
  the way up.
- **If a cable gets warm, or the instrument switches off when you plug it in,
  unplug it and bin that cable.** We lost a cable that way; it's not worth
  finding out what else it can do.

On Orange, while **USB MIDI** (the computer-facing USB Device MIDI) is on, it is
the active MIDI route. Any MIDI Host output you chose is remembered for when you
turn USB Device MIDI off; **Disconnect** forgets that remembered choice but does
not switch USB Device MIDI off.

## What your computer calls it

Each instrument introduces itself by name, so two of them on one computer don't
blur into one: `Octessera (Raspberry 1a2f)` or `Octessera (Orange 1a2f)`. That
name is used for the USB device, its audio, and its MIDI port in your DAW. The
four characters at the end are the same ones the
[setup hotspot](setup-portal.md) shows as `Octessera Setup 1a2f`, so you
can check the OLED and know which box you're talking to. (On a board with no
working Wi-Fi chip they come from the system ID instead, and still stay put.)

If your computer met the instrument under an older name, it may keep showing
that one until you remove the old device entry and plug it back in.

## Plugging and unplugging

Plug and unplug MIDI gear and computers whenever you like, before boot or in
the middle of a jam. A MIDI device you picked stays picked while it's
unplugged and reconnects about a second after it comes back. Notes sent while
nothing is listening just go nowhere; no error pops up.

## Turning USB Audio or USB MIDI on and off

Both switch over right away, and so does **HDMI Audio**. For the two USB ones
the instrument rebuilds its USB connection to the computer, which takes a
second or so; your computer may notice the instrument leave and come straight
back. The jack keeps playing the whole time. With no computer plugged in, USB
Audio simply waits and starts playing once one turns up.

## USB keyboard control

A USB keyboard can drive the instrument with the same controls as the
hardware. The key map is in the [controls cheat
sheet](../controls-cheat-sheet.md#usb-host-keyboard).

- **Where to plug it in:** on the Raspberry Pi, the USB role must be **Host**
  (see above). On the Orange Pi, use the one-dot port (USB1).
- **When it works:** only while `System > Display > HDMI Video > Mode` is one of
  the graphical modes: `live-grid`, `plain-grid`, `active-behavior` or
  `cycle-behaviors`. `Terminal` releases the keyboard; switching back to a
  graphical mode picks it up again.
- **One at a time:** the first supported keyboard is captured exclusively. A
  second one waits until the first is unplugged. Plugging and unplugging works
  while the instrument is running.
- **Held keys:** arrow and Aux-turn keys repeat when held, like turning the
  knob further. Held Enter, Aux clicks, Back, Space, Shift and Control do not
  repeat, so combinations such as Shift+Space (Shift+Play) and Control+Space
  (Fn+Play) behave like the buttons.

A [Bluetooth keyboard](../bluetooth.md) uses the same map and works in either
Raspberry USB role.

For the physical openings, see [enclosure](enclosure.md).
