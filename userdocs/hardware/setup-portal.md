# Open or reopen the full setup portal

Current Raspberry Pi Zero 2 W and Orange Pi Zero 2W image definitions include
the same full setup portal. Physical qualification is still pending, but the
flow is the same whether the board is fresh from the image or already living in
your musical box.

For hotspot, apply, or credential problems, use [troubleshooting](../troubleshooting.md).
For power and host-cable handling while setup is running, see [safety and
power](safety-and-power.md).
For moving user data before a reflash, see [Data Backup and Restore](../data-backup-restore.md).

## Open it from the instrument

1. Stop playback before you begin. If it is still playing, Octessera stops and
   resets it when you confirm the action, then clears held notes and MIDI.
2. Choose `System > Configure WiFi` and confirm `Open Portal`.
3. Join `Octessera Setup <4-char code>` from your phone or laptop.
4. Browse to `http://192.168.42.1`.
5. Choose the Wi-Fi network and settings to apply. The portal can change
   Wi-Fi, hostname, SSH access, and the board's admin login (`pi` on Raspberry
   Pi Zero 2 W; `octessera` on Orange Pi Zero 2W).

The portal lasts 30 minutes. Do not power off the instrument while it says it
is applying settings. The hotspot disappearing after a successful connection is
expected: the instrument is joining the chosen Wi-Fi. Success needs no reboot.

## Data Backup/Restore during setup

On Raspberry Pi and Orange Pi, the active `Configure WiFi` session also starts a
short-lived local transfer server. The OLED shows its host, port, and transfer
code; build the URL as `http://` plus the displayed host and port. Use that
service to export or upload an archive; uploading pauses at a physical
confirmation step. Press the Main encoder to restore or Back to cancel. The
transfer does not include Wi-Fi, SSH, or admin credentials. Desktop reports this
transfer path as unsupported.

## OLED modal behaviour

`Hide` hides the current OLED phase. It does not cancel setup; the portal keeps
running. The modal reappears when setup advances, including its final result.
Wait for that result before trying again. A failed attempt may leave some
settings applied, so do not assume an all-or-nothing rollback.

If setup times out or fails, keep the instrument powered, close the terminal
message, reconnect to the available network, and choose `System > Configure
WiFi` again. A retry requires that new action. If the Wi-Fi or login changed,
use the new network and credentials; if the board is not reachable, use an
existing local console or admin recovery path rather than interrupting an
in-progress apply.

The desktop simulator reports setup as unsupported because it has no board
hotspot or root-owned setup service. It does not emulate this network flow.
