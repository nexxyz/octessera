# Data Backup and Restore

Data Backup/Restore moves your Octessera user data before a reflash. It is a
local, board-side transfer for Raspberry Pi Zero 2 W and Orange Pi Zero 2W.
Desktop transfer remains unsupported.

## What the backup contains

The archive contains:

- every named patch, with its exact display name preserved;
- the current state and saved default state;
- a delta from Octessera's canonical defaults for supported editable
  preferences, including audio outputs, output buffer, display/UI choices, and
  selected MIDI, HDMI, recording, autosave, and backup preferences; and
- optional media: non-standard user samples, plus persisted audio and screen
  recordings when those files are present.

Packaged samples are not duplicated. The archive is user data, not a board
image: it contains no application or device binaries, OS images, firmware,
Wi-Fi/SSH/admin credentials, hardware identity, or device-specific port/path
identity. It may carry a board profile and runtime version as compatibility
metadata.

## Start a transfer

1. On the board, choose `System > Configure WiFi` and confirm `Open Portal`.
2. Join the `Octessera Setup <4-character suffix>` hotspot shown on the OLED.
3. Build the transfer `URL` as `http://` plus the displayed host and port, and
   use the `Code` shown on the OLED. The transfer server is local and
   short-lived; it is not a cloud service. Use it promptly, before the
   setup/transfer session closes.

The current transfer service is a small authenticated HTTP endpoint. A local
HTTP client can export the data-only archive like this:

```sh
URL="http://192.168.42.1:8081" # use the displayed host and port
CODE="TRANSFER_CODE"            # use the transfer code shown on the OLED
curl -fL -H "X-Octessera-Transfer-Code: $CODE" \
  -o octessera-user-data.oct "$URL/export"
```

To include optional user media, request `$URL/export?media=1` instead. Keep the
download somewhere safe before removing or flashing the source card.

## Pre-flash and restore flow

1. Export from the old board before flashing it. Include media if you need
   custom samples or saved recordings.
2. Save `octessera-user-data.oct` off the board.
3. Flash the matching Raspberry or Orange image and complete its normal first
   boot and setup.
4. On the fresh board, open `System > Configure WiFi` again, join the hotspot,
   and upload the archive to the transfer URL:

   ```sh
   curl -f -X POST --data-binary @octessera-user-data.oct \
     -H "X-Octessera-Transfer-Code: $CODE" "$URL/restore"
   ```

5. Uploading only stages the restore. When the transfer reports that physical
   confirmation is required, press the Main encoder to apply it or Back to
   cancel it. No user data is changed before that confirmation.
6. Wait for the final restore result before closing the setup session.

Restore validation checks the archive format and version, patch/settings
compatibility, safe names, manifest hashes, media sizes and hashes, and
available space before changing live data. After confirmation, the managed
store, sample, audio-recording, and screen-recording trees are replaced as one
guarded transaction; a swap failure rolls back earlier swaps, and a
pre-restore archive is kept.

Corrupt, incompatible, too-large, unsafe, or unsupported media and settings
are reported as a failed or invalid restore. They are not silently replaced
with defaults, and a rejected restore leaves the existing data untouched.

Desktop's `Configure WiFi` status is `unsupported`, so it does not start this
transfer server or provide a desktop backup/restore path.
