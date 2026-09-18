# Flash and first boot

Use this shared flow after the open assembly is wired and ready to power. The
Raspberry Pi Zero 2 W and Orange Pi Zero 2W use different images, but the
user-facing first boot is the same.

## 1. Download the board image

Open the [current release page](https://github.com/nexxyz/octessera/releases)
and download the image for the board in your instrument:

- **Raspberry Pi Zero 2 W:** choose the Raspberry Pi Zero 2 W image.
- **Orange Pi Zero 2W:** choose the Orange Pi Zero 2W image.

If the release provides a checksum, use the checksum belonging to that exact
image. Do not use the other board's image.

## 2. Flash the microSD card

1. Insert the instrument's boot microSD card into your computer or card reader.
2. Open [BalenaEtcher](https://etcher.balena.io/) or another suitable image
   flasher.
3. Select the downloaded image, select the microSD card, and start the flash.
4. Eject the card safely when the flasher finishes.

### Optional Raspberry Pi Imager alternative

On the Raspberry Pi path, Raspberry Pi Imager can use the release's
`.rpi-imager-manifest` as a custom repository or the image as a custom OS.
This is optional; BalenaEtcher or another image flasher is the normal path.
To use it, open Imager's **App Options**, edit **Content Repository**, choose
**Use custom file** or **Use custom URL**, select the release manifest, and
choose **Apply and Restart**.
> **Preferred Raspberry setup:** Flash the image as-is, then use Octessera's
> built-in Wi-Fi setup below to configure Wi-Fi, the device password, and SSH.
> If you customize with Raspberry Pi Imager instead, keep the username exactly
> `pi`. **Do not change it.** Octessera's Raspberry runtime requires that fixed
> account.

Imager may configure SSH, the hostname, and Wi-Fi. An unconfigured/raw
Raspberry flash keeps SSH disabled by default. The shared setup portal below
remains available for either board.

## 3. Power the open assembly

1. Insert the flashed boot card into the selected compute board.
2. Keep the enclosure open for this first boot.
3. Connect audio if you want to hear the first result.
4. Power the instrument through the enclosure USB-C power breakout. Do not
   power the Raspberry Pi through its own micro-USB power connector.
5. Wait for the Octessera splash screen and normal menu.

## 4. Configure Wi-Fi and access

On the instrument, open:

`System > Setup > Configure WiFi > Open Portal`

The setup hotspot can be joined by anyone nearby. Open it while you are present,
complete setup promptly, and do not leave setup mode running in public.

Then, on a phone or computer:

1. Join `Octessera Setup <suffix>`.
2. Browse to `http://192.168.42.1`.
3. Choose the country and the network. Select a scanned network or enter an
   SSID manually, then enter its password or choose an open network.
4. Set the local device password.
5. Choose an SSH mode: key, password, or off.
6. Enter an optional hostname.
7. Press **Apply setup**.

The browser may disconnect while the settings apply. That is expected. Trust
the result shown on the OLED rather than the browser's temporary state. After a
successful setup, `System > Sys. Info` shows the device address. If setup fails,
dismiss the OLED message and run **Open Portal** again.

## 5. Make music

Once the OLED reports a successful setup, close the portal if it is still open,
connect headphones or speakers, and make music. Keep the assembly accessible
until you have checked the display, grid, keys, encoders, and audio.

For a later network or credential change, use [Open or reopen the setup
portal](setup-portal.md). For USB data roles, see [USB roles](usb-roles.md).
