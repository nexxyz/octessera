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

### Keyboard control

With the persisted role set to **Host**, connect one supported USB keyboard
through the OTG adapter. Octessera captures the first supported keyboard only
and grabs it exclusively while capture is active. A second keyboard waits until
the first one is unplugged. Plugging in or unplugging a keyboard takes effect
while the instrument is running.

Keyboard capture is available only when `System > HDMI Video > Mode` is a
graphical mode: `live-grid`, `plain-grid`, `active-behavior`, or
`cycle-behaviors`. `Terminal` releases capture; changing back to a graphical
mode picks up a supported connected keyboard again.

The base keys mirror simulator controls. USB-host capture additionally maps the
three AUX encoder rows:

| Keyboard key | Control |
|---|---|
| ← / ↑ | Main encoder turn left |
| → / ↓ | Main encoder turn right |
| Enter | Main encoder press |
| Q / W / E | Aux 1 turn left / click / right |
| A / S / D | Aux 2 turn left / click / right |
| Y/Z / X / C | Aux 3 turn left / click / right |
| Backspace / Escape | Back |
| Space | Play / Pause |
| Shift | Shift |
| Control | Fn |

Arrow and Aux turn repeats turn their encoder again. Repeats for Enter, Aux
clicks, Back, Space, Shift, and Control are ignored; their key releases still
release the matching native control. That keeps combinations such as
Shift+Space and Control+Space on the same native path as the buttons:
Shift+Space is Stop (or Resync arm under external sync), and Control+Space is
Reset stop.

The Aux 3 left key is the physical key immediately right of left Shift. Linux
binds that position as `KEY_Z`, whether the keycap says Y or Z. Space remains
the hardware S button; the letter S is the Aux 2 encoder click.

To return to Gadget mode, disconnect the USB device, select
`System > USB Role > Gadget`, save, and reboot. Then connect the data port to a
computer with the USB-A-to-Micro-USB cable.

## Orange Pi

Orange has fixed image-defined USB roles and no `System > USB Role` switch. USB
data is optional for normal use. Its fixed USB-A host connector accepts the
keyboard independently of USB0 OTG/gadget behavior; there is no Orange role
switch to change.

The same graphical HDMI gate and keyboard mapping apply on Orange. HDMI mode
changes and keyboard hotplug are live, and capture remains exclusive to the
first supported keyboard. The Aux 3 left key follows the same physical Y/Z
rule, and the letter S remains Aux 2 click rather than the hardware Space/S
button.

For the physical openings, see [enclosure](enclosure.md). For the shared power
rules, see [safety and power](safety-and-power.md).
