# Data Backup and Restore

Use the standalone `System > Backup / Restore` action to move Octessera user
data before a reflash. It is separate from `Configure WiFi`: it never opens the
setup AP or uses the root setup coordinator. Desktop is unsupported.

## Start a transfer

1. On a Pi board, choose `System > Backup / Restore`.
2. The OLED Ready card shows the regular network `IP`, `PORT 8081`, a generated
   10-character `CODE`, and the remaining lifetime. Build `URL` as
   `http://<regular-ip>:8081` and use the displayed code.
3. Use the existing authenticated HTTP API. There is no new themed web app.

```sh
URL="http://<regular-ip>:8081"
CODE="<10-character code shown on the OLED>"
curl -fL -H "X-Octessera-Transfer-Code: $CODE" \
  -o octessera-user-data.oct "$URL/export"
```

To include optional user media, request `$URL/export?media=1` instead. Keep the
download somewhere safe before removing or flashing the source card.

If the board has no current usable regular `wlan0` IPv4 address, the action is
unavailable and does not bind or retry. Choosing the action again while the
service is active shows the same URL and code with its remaining lifetime; it
does not extend the session. Back hides the Ready card while the service keeps
running. Select `> Stop service` to close it and revoke the code. Expiry or
authentication revocation closes it automatically.

## Pre-flash and restore flow

1. Export from the old board before flashing it. Include media if you need
   custom samples or saved recordings.
2. Save `octessera-user-data.oct` off the board.
3. Flash the matching Raspberry or Orange image and complete its normal first
   boot and regular network setup.
4. On the fresh board, choose `System > Backup / Restore` again. Use the new
   dynamic `URL` and OLED code:

   ```sh
   curl -f -X POST --data-binary @octessera-user-data.oct \
     -H "X-Octessera-Transfer-Code: $CODE" "$URL/restore"
   ```

5. Uploading validates and prepares the restore, but does not change user data.
   When physical confirmation is required, press the Main encoder to apply it
   or Back to cancel it. During restore the OLED shows `Restoring...` and
   `Please wait`, and normal device input is blocked until the result.
6. Wait for the final restore result before stopping the service or powering
   down. The existing status endpoint is available at `$URL/restore/status`.

Restore validation checks the archive format and version, patch/settings
compatibility, safe names, manifest hashes, media sizes and hashes, and
available space before changing live data. After confirmation, the managed
store, sample, audio-recording, and screen-recording trees are replaced as one
guarded transaction; a swap failure rolls back earlier swaps, and a
pre-restore archive is kept.

Corrupt, incompatible, too-large, unsafe, or unsupported media and settings
are reported as a failed or invalid restore. They are not silently replaced
with defaults, and a rejected restore leaves the existing data untouched.

The `System > Saves > Default > Backups` setting remains the ordinary rolling
local safety-backup feature; it is separate from this user-data transfer.
The OLED SD2 `octessera/saves` directory is user-managed and is outside both
Backup and Restore; copy it manually when you want to preserve those files.
