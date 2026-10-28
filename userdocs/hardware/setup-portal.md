# Open or reopen the setup portal

For the very first setup, follow [flash and first boot](flash-and-first-boot.md#4-configure-wi-fi-and-access);
it walks through every field. Use this page when the instrument is already
running and you want to change its Wi-Fi network, password, SSH access or name.
Moved house, changed router, lost the SSH key down the back of the sofa: same
steps.

## Steps

1. On the instrument, open `System > Connections > Wi-Fi > Setup Portal` and
   confirm **Open Portal**. If you had switched Wi-Fi off, this turns it back
   on.
2. On a phone or computer, join `Octessera Setup <suffix>`, exactly as shown on
   the OLED.
3. Browse to `http://192.168.42.1`.
4. Fill in the country and Wi-Fi network, set the local device password, choose
   the SSH mode, and optionally enter a hostname.
5. Press **Apply setup**.

> **Heads-up:** The setup hotspot is open to anyone in range until setup
> finishes. Open it while you are there, finish promptly, and do not leave it
> running in a public place. The portal closes by itself after 10 minutes.

## What the portal sets

| Setting | Details |
|---|---|
| Wi-Fi | Country code, then a typed (Manual SSID) or scanned network name and its password (or open network). |
| Local device password | For `pi` on the Raspberry Pi or `octessera` on the Orange Pi. At least 8 characters. Required even with SSH off. |
| SSH | **Key** (recommended; password login stays off), **password** (uses the local device password), or **none** (SSH stays off). Root login is never allowed. |
| Hostname | Optional: letters, digits and hyphens, up to 63 characters, not starting or ending with a hyphen. Leave blank to keep the current name; a fresh card uses `octessera-rpi` or `octessera-opi`. |

## What you should see

- The browser may disconnect while the settings apply. Leave the instrument
  powered and wait for the result on the OLED.
- On success, the OLED jumps to `System > Maintenance > Sys. Info` and shows
  the board's new Wi-Fi address; no reboot is needed.
- If the OLED reports a failure or a timeout, close the message and open the
  portal again.
- If it says **Wi-Fi chip stopped**, restart the instrument before trying again.
  The Orange Pi's Wi-Fi chip sometimes gives up when the hotspot is started a
  second time without a restart in between, and only a restart wakes it.

For power and cable precautions, see [safety and power](safety-and-power.md).
For USB data connections, see [USB roles](usb-roles.md).
