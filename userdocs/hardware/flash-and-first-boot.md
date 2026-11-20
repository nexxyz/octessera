# Flash and first boot

This is the moment the soldering turns into an instrument. Do it with the
[open assembly](assembly-manual.md#4-open-assembly) wired up and the case still
off, so anything that misbehaves is still within reach.

The Raspberry Pi Zero 2 W and the Orange Pi Zero 2W use different images, but
from here on the steps are the same.

## 1. Download the board image

Open the [releases page](https://github.com/nexxyz/octessera/releases) and
download the image for **your** board:

| Board | Image file |
|---|---|
| Raspberry Pi Zero 2 W | `octessera-<version>-raspberry-pi-zero-2w.img.zip` |
| Orange Pi Zero 2W | `octessera-<version>-orange-pi-zero-2w.img.xz` |

If you want to check the download, compare it against its line in
`SHA256SUMS.txt`, which covers both images. The two boards' images are not interchangeable: an Orange card
in a Raspberry (or the other way round) won't boot.

## 2. Flash the microSD card

1. Put the boot microSD card into your computer or card reader.
2. Open [BalenaEtcher](https://etcher.balena.io/). It reads the `.img.zip` and
   `.img.xz` files directly; no need to unpack them first.
3. Select the downloaded image, select the microSD card, and start flashing.
   Double-check the target drive. Flashers erase whatever you point them at.
4. When it finishes, eject the card safely.

BalenaEtcher is the normal path for both boards, and the one to use for the
Orange Pi.

### Optional: Raspberry Pi Imager (Raspberry only)

The Raspberry release also includes
`octessera-<version>-raspberry-pi-zero-2w.rpi-imager-manifest`, which lets
Raspberry Pi Imager show Octessera as an OS choice:

1. Open Imager and choose **App Options**.

   ![Raspberry Pi Imager App Options button](images/assembly/rpi-imager-options.png)

2. Next to **Content Repository**, choose **Edit**.

   ![Content Repository Edit button](images/assembly/rpi-content-repository.png)

3. Choose **Use custom file** (and browse to the downloaded manifest) or **Use
   custom URL**, then **Apply & Restart**.

   ![Custom repository source choices](images/assembly/rpi-repository-source.png)

> **Important:** The easiest route is to flash the image as-is and set up
> Wi-Fi, the password and SSH with Octessera's own [setup
> portal](#4-configure-wi-fi-and-access). If you do use Imager's customisation
> instead, keep the username exactly `pi`. **Do not change it.** The Raspberry
> runtime depends on that account.

Imager can configure SSH, the hostname and Wi-Fi. A plain flash without Imager
customisation leaves SSH switched off until you turn it on in the setup portal.

## 3. Power the open assembly

1. Put the flashed card into the compute board.
2. Keep the enclosure open for this first boot.
3. Connect audio if you want to hear the first result.
4. Power the instrument through the USB-C power breakout, as
   [safety and power](safety-and-power.md#power-input) explains.
5. Wait for the Octessera boot animation and then the normal menu. From a cold
   start the menu is ready after about 21 seconds on the Raspberry Pi Zero 2 W
   and about 39 seconds on the Orange Pi Zero 2W.

Nothing on the OLED, or flickering? Unplug, then check these in order:

1. The image matches the board (a Raspberry image on an Orange won't start,
   and vice versa) and the card is pushed all the way into the compute board.
2. Power comes in through the USB-C power breakout, as in step 4 above.
3. The OLED's cable is seated at both ends.

Still nothing? If the OLED shows a message, [troubleshooting](../troubleshooting.md)
has it by name.

## 4. Configure Wi-Fi and access

Wi-Fi is optional for making music, but it is how you get updates, backups and
SSH. A fresh image does not start a hotspot or SSH on its own; you ask for it.

On the instrument, open:

`System > Connections > Wi-Fi > Setup Portal`

and confirm **Open Portal**. The OLED then shows the hotspot name, its
password, the address to open, and a 10-minute countdown for the portal.

> **Heads-up:** Until setup finishes, anyone who can read the OLED can join the
> setup hotspot and configure the instrument. Open it while you are there and
> finish promptly.

On a phone or computer:

1. Join the Wi-Fi network `Octessera Setup <suffix>` shown on the OLED, using
   the password on the line below it. Every portal gets a fresh one, so last
   time's password won't work. Type it exactly as shown, capitals and dash
   included. The tiny screen font has a few look-alikes, so the password
   simply never uses them; no squinting at 0 versus O.
2. Many phones and laptops pop the setup page up by themselves. If yours
   doesn't, browse to `http://192.168.42.1`.
3. Enter your two-letter country code and press **Scan for networks**. Despite
   the name, it loads the list the instrument scanned when the portal started
   (strongest signal first); if your network is missing, close the portal on
   the instrument and open it again. Pick your network and enter its password.
   Open networks don't need one. Hidden network? Pick **Other network…** and
   type its name.
4. Set the local device password, at least 8 characters. The page tells you
   which account it is for: `pi` on the Raspberry Pi, `octessera` on the Orange
   Pi. You only need it for maintenance, but the page always asks for it, so
   write it down.
5. Choose how you want SSH:
   - **SSH password** (the default): SSH accepts the local device password
     from step 4.
   - **No SSH setup:** remote access stays off.
   - **SSH key**, under **Advanced:** paste your public key; password login
     stays off.
6. Optionally enter a device name (hostname). Leave it blank to keep the current
   one: `octessera-rpi` on the Raspberry Pi or `octessera-opi` on the Orange Pi
   on a fresh card.
7. Press **Apply setup**.

The browser will probably lose its connection while the settings apply, because
the hotspot goes away. That is expected. Leave the instrument powered and trust
the OLED, not the browser; it wakes up by itself as setup moves along. On
success, a Sys. Info popup shows the board's address; no reboot is needed. If the OLED reports a failure or a timeout, close the message
and open the portal again. If it says **Wi-Fi chip stopped**, restart the
instrument first: the Orange Pi's Wi-Fi chip can give up on a second hotspot
start, and only a restart brings it back.

To change the network, password, SSH or name later, see [setup
portal](setup-portal.md).

## 5. Finish the open-air checks

With setup done (or skipped), close the portal message if it is still showing,
and check the encoders, OLED and NeoTrellis connection while everything is still
in reach. The full checklist, and why the NeoKey switches wait for the
enclosure top, are in the [assembly manual](assembly-manual.md#5-flash-and-setup).

Then carry on with the [enclosure steps](assembly-manual.md#6-enclosure) and the
[final check](assembly-manual.md#7-final-check).

For USB connections to a computer or USB devices, see [USB roles](usb-roles.md).

One habit worth forming early: Auto Save is off, so the instrument only keeps
what you save (`System > Saves > Default > Save Patch`, or a named preset under
`System > Saves > Library`). Reboot and Shutdown from the menu save a recovery
copy first; yanking the power cable doesn't.
