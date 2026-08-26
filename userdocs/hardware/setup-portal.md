# Open or reopen the full setup portal

Current Raspberry Pi Zero 2 W and Orange Pi Zero 2W image definitions include
the same setup portal. Physical qualification is still pending, but the flow is
the same on either board.

For hotspot, apply, or credential problems, use [troubleshooting](../troubleshooting.md).
For power and host-cable handling while setup is running, see [safety and
power](safety-and-power.md).

## Open it from the instrument

1. Stop playback before you begin. If it is still playing, Octessera stops and
   resets it when you confirm the action, then clears held notes and MIDI.
2. Choose `System > Configure WiFi` and confirm `Open Portal`.
3. Join `Octessera Setup <4-char suffix>` from your phone or laptop.
4. Browse to `http://192.168.42.1`.
5. Choose the country, a scanned or manual SSID, Wi-Fi password or open network,
   SSH key/password/none, and an optional hostname, then press `Apply setup`.
   SSH mode configures the board account: `pi` on Raspberry and `octessera` on
   Orange.

The AP stays available for 10 minutes after it is ready. Do not power off the
instrument while the OLED says it is applying settings. The browser's Applying
screen is provisional: an AP disconnect is expected and is not a success or
failure result. The OLED is authoritative.

Success requires a usable global `wlan0` IPv4 address. It does not require
Internet access, a default route, DNS, or ICMP. After success, choose
`System > Info` to see the IP; no reboot is needed.

On the OLED, success and timeout cards hide automatically. A failure card stays
dismissible. To try again, close it and choose `System > Configure WiFi > Open
Portal` again. This action is for network and access setup only; it does not
start or advertise Data Backup/Restore.

When Internet is available, `System > Updates` only checks, applies, or rolls back
the Octessera runtime. It does not update the Armbian OS/image, kernel, device
tree, or other full-image assets; those remain manual image operations.

## OLED modal behaviour

`Hide` hides an in-progress OLED card without cancelling setup. Wait for the
terminal result before deciding what happened. A new `Open Portal` menu action
starts another attempt after a failure or timeout.

The desktop simulator reports setup as unsupported because it has no board
hotspot or root-owned setup service. It does not emulate this network flow.
