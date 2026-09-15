# Data backup and restore

Use `System > Setup > Backup / Restore` on a Pi before reflashing. It moves your
Octessera settings, patches, and other user data from one installation to
another.

## Back up your data

1. On the Pi, choose `System > Setup > Backup / Restore`.
2. The OLED Ready card shows the regular network `IP`, `PORT 8081`, a generated
   10-character `CODE`, and the remaining lifetime. Build `URL` as
   `http://<regular-ip>:8081` and use the displayed code.
3. Download the backup:

   ```sh
   URL="http://<regular-ip>:8081"
   CODE="<10-character code shown on the OLED>"
   curl -fL -H "X-Octessera-Transfer-Code: $CODE" \
     -o octessera-user-data.oct "$URL/export"
   ```

   To include optional user media, use `$URL/export?media=1` instead. Keep the
   downloaded `octessera-user-data.oct` somewhere safe before removing or
   flashing the source card.

If no regular network address is available, the action cannot start. **Back**
hides the Ready card while the transfer remains available. Choose **> Stop service**
when you are finished; the code also expires automatically.

## Restore your data

Restoring replaces the destination's current user data. Export it first if
there is anything on the fresh board you want to keep.

1. Export from the old board before flashing it. Include media if you need custom
   samples or saved recordings.
2. Save `octessera-user-data.oct` somewhere off the board.
3. Flash the matching Raspberry or Orange image and complete its normal first
   boot and network setup.
4. On the fresh board, choose `System > Setup > Backup / Restore` again. Use the
   new `URL` and OLED code:

   ```sh
   curl -f -X POST --data-binary @octessera-user-data.oct \
     -H "X-Octessera-Transfer-Code: $CODE" "$URL/restore"
   ```

5. The upload prepares the restore but does not change your data yet. When the
   OLED asks for confirmation, press the Main encoder to apply it or **Back** to
   cancel. During the restore, the OLED shows `Restoring...` and `Please wait`,
   and normal input is blocked.
6. Wait for the final result before stopping the service or powering down. An
   invalid or failed restore leaves the existing data in place.

Never remove power, the storage card, or the board's microSD card during a
transfer or restore. Do not erase the source card until the backup file is
downloaded and safely stored.

The `System > Saves > Default > Backups` setting is a separate rolling local
backup. The OLED SD2 `octessera/saves` directory is also separate; copy it
manually when you want to preserve those files.
