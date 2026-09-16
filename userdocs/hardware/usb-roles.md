# USB roles

Use this page after [flash and first boot](flash-and-first-boot.md) when you
need to connect a computer or USB device.

Keep the instrument powered through the enclosure USB-C power breakout. The
data cable is not a second power supply. Never connect two power sources and
never rely on a cable or adapter to prevent back-feed.

## Raspberry Pi

The Raspberry Pi USB data port starts in **Gadget** mode. In this mode, connect
the Pi's micro-USB data/gadget port to a computer with a USB-A-to-Micro-USB
cable.

To use the port with USB devices instead:

1. Disconnect the computer's USB data cable.
2. Turn USB Audio and USB MIDI output off.
3. Open `System > USB Role` and select **Host**.
4. Save the setting and wait for the OLED to report the reboot result.
5. After reboot, connect the device through an ID-grounded micro-USB OTG
   adapter. A powered hub is useful only when its upstream path cannot
   back-feed the instrument.

The role changes on the next boot; do not try to change it while the port is
live. Host mode disables gadget USB Audio, gadget USB MIDI, and new SD2
transfers. Gadget mode makes USB Audio, USB MIDI, and SD2 transfer available
again, but switching back to Gadget does not re-enable output settings that Host
disabled.
After reboot, turn desired USB outputs back on separately.

Use a USB-A host or hub connection for the bench cable. Avoid USB-C-to-USB-C
and USB Power Delivery cables for this connection.

To return to Gadget mode, disconnect the USB device, select
`System > USB Role > Gadget`, save, and reboot. Then connect the data port to a
computer with the USB-A-to-Micro-USB cable.

## Orange Pi

Orange has fixed image-defined USB roles and no `System > USB Role` switch. USB
data is optional for normal use.

For the physical openings, see [enclosure](enclosure.md). For the shared power
rules, see [safety and power](safety-and-power.md).
