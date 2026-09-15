# Open or reopen the setup portal

For the initial setup, follow [flash and first boot](flash-and-first-boot.md).
Use this page when the instrument is already running and you want to change its
network, password, SSH mode, or hostname.

## Steps

1. On the instrument, open `System > Setup > Configure WiFi > Open Portal`.
2. On a phone or computer, join `Octessera Setup <suffix>`.
3. Browse to `http://192.168.42.1`.
4. Choose the country and Wi-Fi network, set the local device password, choose
   an SSH mode, and optionally enter a hostname.
5. Press **Apply setup**.

The deliberately opened nearby setup hotspot can be joined by anyone in range.
Open it while present, complete setup promptly, and do not leave setup mode
running in public.

The portal sets the local board password you entered. SSH mode can be key,
password, or off. Root login is not available.

## What you should see

- The browser may disconnect while settings apply. Leave the instrument powered
  and wait for the OLED result.
- A successful result gives the board a usable Wi-Fi address. Find it later at
  `System > Sys. Info`; no reboot is needed.
- If the OLED reports a failure or timeout, dismiss it and start a new attempt
  with **Open Portal**.

For power and cable precautions while setting up, see [safety and
power](safety-and-power.md). For USB data roles, see [USB roles](usb-roles.md).
