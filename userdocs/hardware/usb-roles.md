# USB roles

Use this page after [flash and first boot](flash-and-first-boot.md), when you
want to plug the instrument into a computer (as a USB audio/MIDI device) or plug
a USB device, such as a keyboard, into the instrument.

> **Power first:** Keep the instrument powered through the enclosure's USB-C
> power breakout. A USB data cable is not a second power supply. Never connect
> two power supplies, and never rely on a cable or adapter to prevent back-feed.
> The details are in [safety and power](safety-and-power.md).

## Raspberry Pi

The Raspberry Pi's USB data port can do one of two jobs, chosen at boot:

| Role | What it does | Connect |
|---|---|---|
| **Gadget** (default) | The instrument appears to a computer as USB Audio, USB MIDI and, on request, the SD2 card transfer. | The Pi's micro-USB data port to a computer's USB-A port, with a USB-A-to-micro-USB cable. |
| **Host** | The instrument talks to USB devices such as a keyboard or a MIDI controller. | A USB device through a micro-USB OTG adapter. |

For the computer connection, use a USB-A port on the computer or a hub. Avoid
USB-C-to-USB-C cables and USB Power Delivery chargers or ports for this.

### Switch to Host

1. Unplug the computer's USB cable.
2. Turn **USB Audio** and **USB MIDI** off.
3. Open `System > Connections > USB Role` and select **Host**.
4. Confirm the save. The OLED reminds you to unplug the computer and offers
   **Reboot now**; the new role only takes effect after a reboot.
5. After the reboot, connect your device through the OTG adapter. A powered hub
   is fine only if it cannot back-feed power into the instrument.

In Host mode, USB Audio, USB MIDI and starting new SD2 transfers are
unavailable.

### Switch back to Gadget

1. Unplug the USB device.
2. Select `System > Connections > USB Role > Gadget`, save, and reboot.
3. Connect the data port to a computer with the USB-A-to-micro-USB cable.
4. Turn USB Audio or USB MIDI back on yourself if you want them. Switching back
   to Gadget does not re-enable the features that Host mode switched off.

## Orange Pi

The Orange Pi's USB roles are fixed by the image, so there is no
`System > Connections > USB Role` setting. Its separate USB host port takes a
keyboard regardless of what the other (OTG/gadget) port is doing. USB data is
optional for normal use.

On Orange, while **USB MIDI** (the computer-facing USB Device MIDI) is on, it is
the active MIDI route. Any MIDI Host output you chose is remembered for when you
turn USB Device MIDI off; **Disconnect** forgets that remembered choice but does
not switch USB Device MIDI off.

## USB keyboard control

A USB keyboard can drive the instrument with the same controls as the
hardware. The key map is in the [controls cheat
sheet](../controls-cheat-sheet.md#usb-host-keyboard).

- **Where to plug it in:** on the Raspberry Pi, the USB role must be **Host**
  (see above). On the Orange Pi, use its USB host port.
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
