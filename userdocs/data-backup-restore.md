# Data backup and restore

Octessera keeps two kinds of data apart:

- **Music:** the patch (layers, Link, instruments, FX, Play setup) and your
  named presets.
- **System settings:** device preferences such as brightness, Auto Save, USB
  role, audio outputs and MIDI ports.

Saving, autosave and rolling backups all work on one or the other, never
both. The board's `Backup / Restore` export carries your music plus a few
portable preferences off the board so you can reflash without losing anything.

## Saving music

| Menu | What it does |
|---|---|
| `System > Saves > Default > Save Patch` | Saves the current patch as this instrument's default patch. |
| `System > Saves > Default > Load Patch` | Loads it back, after asking. |
| `System > Saves > Default > Auto Save` | Saves patch edits automatically. Off by default. |
| `System > Saves > Default > Backups` | Keeps rolling backups of the patch. |
| `System > Saves > Library` | Named presets: **Save As**, **Load**, **Rename**, **Delete**, **Save Current**. |
| `System > Load Preset` | Shortcut to load a named preset. |

A confirmed load stops and rewinds playback and silences held notes. If a save
is still being written (or, with Auto Save on, an edit hasn't been saved yet)
the load refuses with `Save pending, try again`; give it a moment. With Auto
Save off, loading throws away unsaved changes, so save first if you care.

Patches remember their samples by path, including your own and SD-card
samples.

## Saving system settings

Most System settings save themselves once when you finish editing the row
(with *Main* or *Back*), not on every turn, and only if the value actually
changed. Settings that need a restart save only after you confirm **Apply**.

You can also use `System > Saves > System > Save System` and **Load System**.
Load System asks first, but doesn't stop the music or reboot. While a System
save is still being written, **Reboot** and **Shutdown** refuse with
`System save pending, try again`; press again once it's done.

Changing Auto Save is itself a System setting. It saves the preference, not
the patch.

## Back up a board

Backup and restore run on the Raspberry and Orange boards over Wi-Fi. The
desktop simulator doesn't have it.

1. On the board, choose `System > Maintenance > Backup / Restore`.
2. The OLED shows the board's `IP`, `PORT 8081`, a 10-character `CODE` and how
   long is left (15 minutes). If the board has no network address, it can't
   start.
3. On a computer on the same network, download the backup:

   ```sh
   URL="http://<board-ip>:8081"
   CODE="<10-character code shown on the OLED>"
   curl -fL -H "X-Octessera-Transfer-Code: $CODE" \
     -o octessera-user-data.oct "$URL/export"
   ```

   Use `$URL/export?media=1` instead to include media: samples you added to
   the board (folders too, up to four levels deep) and your recordings. The
   bundled library and samples on the SD2 card stay where they are.

4. Keep `octessera-user-data.oct` somewhere safe, and don't erase or reuse the
   old card until a restore from it has worked.

*Back* hides the OLED card while the service keeps running. Choose
**> Stop service** when you're finished; the code also expires on its own.

## Restore onto a board

Restoring replaces the destination's user data, so export it first if there's
anything on that board you want to keep.

1. Flash the matching Raspberry or Orange image and finish its normal
   [first boot and network setup](hardware/flash-and-first-boot.md).
2. Choose `System > Maintenance > Backup / Restore` on the fresh board and
   note its new address and code.
3. Upload the backup:

   ```sh
   curl -f -X POST --data-binary @octessera-user-data.oct \
     -H "X-Octessera-Transfer-Code: $CODE" "$URL/restore"
   ```

4. The upload only prepares the restore. When the OLED asks, click *Main* to
   apply it or press *Back* to cancel. While it runs, the OLED shows
   `Restoring...` and `Please wait`, and other input is blocked.
5. Wait for the final result before stopping the service or powering down.

A broken upload, or a failure before applying, leaves the board's data as it
was. If something fails after the data has been replaced, the instrument
reports it and stops saving; it doesn't roll back on its own. That's why you
keep the source card and the downloaded file until you're happy.

If a restored patch uses a sample that isn't on the new board, that slot stays
silent and shows `N/A-<name>`, and a popup lists what's missing. Copy the WAVs
across (or restore a `media=1` export) and the slots come back, or pick
replacements.

Never pull the power or either microSD card during a transfer or restore.

## What the backup doesn't carry

The `.oct` file is portable by design, not a clone of the instrument. These
stay with whichever board you restore onto:

- USB role, MIDI and audio device choices, and favourite sample folders.
- Any other System settings beyond the few portable preferences (such as
  brightness) the archive carries.
- The SD2 card's `octessera/saves` folder. Copy that by hand if you want it.

Rolling patch backups (`System > Saves > Default > Backups`) protect the patch
only, not System settings.
