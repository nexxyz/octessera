# FAT gap tests and closeout

These tests cover behavior that the broad [board end-to-end
paths](fat-board-end-to-end.md) deliberately do not repeat. Use the `$Evidence`
folder and safety gates from the [FAT orchestrator](fat-quick-run.md). USB
Audio/MIDI remains experimental local-bench validation, not public support.

## USB Audio/MIDI

**Time box: 01:25–01:45.** USB is a separate qualification gap. Do not count
the DAC/Jack sound in the end-to-end paths as USB evidence.

### Automated preparation and capture

- Save the Orange passive USB state from the end-to-end probe as
  `usb/orange-passive-state.txt`.
- On Orange, if the exact production service is running, capture:

  ```bash
  systemctl show octessera-orange-usb-gadget.service --no-pager --property=ActiveState,SubState,MainPID,InvocationID,UnitFileState
  cat /sys/class/udc/musb-hdrc.4.auto/function
  ls /sys/kernel/config/usb_gadget/octessera-orange-pi/functions
  ```

- Prepare `usb/raspberry-host-lsusb-v.txt`,
  `usb/orange-host-lsusb-v.txt`, `usb/raspberry-midi.txt`, and
  `usb/orange-midi.txt`. Capture host output only after the electrical gate
  passes.

### Operator action

1. Before either host cable, confirm the authorized USB identity, exact board
   port role, VBUS/CC behavior, and no-backfeed path. Use a data-only or
   power-isolating cable when the instrument has separate power.
2. If any gate or identity is missing, write `NOT RUN — unsafe or unauthorized`
   in the result matrix and do not connect the host.
3. If authorized, enable USB Audio and/or USB MIDI in `System > Audio & USB`,
   use `Save & Reboot`, and test each board separately.
4. On the host, capture `lsusb -v`, confirm the intended audio device, and send
   one MIDI note from the intended host application. Unplug/replug once and
   record whether enumeration and the intended function recover.

Pass requires safe electrical behavior, expected host enumeration, intended
UAC2 audio, intended MIDI naming and note delivery, reconnect behavior, and no
mass-storage function. Record the authorized identity and host/board names in
`usb/identity-and-port.txt`; never record credentials.

The end-to-end DAC/Jack checks already cover ordinary selected-route audio. This
stage adds only USB enumeration, USB audio, USB MIDI, and reconnect evidence.

## Reboot, shutdown, and recovery

**Time box: 01:45–02:00.** Prepare
`lifecycle/raspberry-reboot.txt`, `lifecycle/orange-reboot.txt`,
`lifecycle/shutdown-recovery.txt`, and capture service state after each return.
Do not use arbitrary administrative power commands as a substitute for the
confirmed instrument actions.

1. On Raspberry, choose `System > Reboot`. On Orange, choose `System > Reboot`.
   Record the native `Rebooting` presentation, OLED behavior, return to service,
   and absence of a restart loop.
2. On the board with time remaining, choose `System > Shutdown`. Wait for the
   action to finish, remove and restore power, and confirm a clean cold boot.
3. If Orange reaches `start-limit-hit` after a real service failure, use the
   documented recovery only after the board is stable:

   ```bash
   sudo systemctl reset-failed octessera.service
   sudo systemctl start octessera.service
   ```

Pass means the requested menu action shows the native lifecycle message, audio
and external MIDI stop safely, the OLED is not left with two writers, and the
board returns to the same known menu/runtime state. Record any power, brownout,
or recovery fault rather than trying another command path.

Normal boot and service start were covered in the end-to-end paths. This stage
covers lifecycle teardown, power submission, and return/recovery only.

## Data Backup/Restore

**Time box: 02:00–02:25.** Run this only with a spare or controlled card,
verified off-board storage, and at least 25 minutes remaining. This slot covers
one board; a full two-board result needs a separate 25-minute slot for the other
board. Otherwise mark the entire row `NOT RUN — mandatory FAT follow-up`; do not
squeeze in an uncontrolled reflash. This is a board-side transfer path on
Raspberry Pi and Orange Pi; desktop transfer is unsupported. See [Data Backup
and Restore](../data-backup-restore.md) for the complete transfer contract.

### Automated preparation and capture

- Prepare `backup/pre-flash-data-only.sha256` and
  `backup/pre-flash-media.sha256`.
- Keep the `.oct` archives outside the shared evidence folder and record only
  their protected path and hash.
- Use the print-only destructive reminders in `00-destructive-commands.txt`.

### Operator action

1. On the source board, open `System > Configure WiFi`, join the displayed local
   portal, and export a data-only archive. Export media-inclusive data too when
   custom samples or recordings are part of the claim. Use the documented
   transfer shape, replacing placeholders with the displayed values:

   ```sh
   URL="http://192.168.42.1:8081"
   CODE="TRANSFER_CODE"
   curl -fL -H "X-Octessera-Transfer-Code: $CODE" -o octessera-user-data.oct "$URL/export"
   curl -fL -H "X-Octessera-Transfer-Code: $CODE" -o octessera-user-data-media.oct "$URL/export?media=1"
   ```

2. Verify both archive hashes off-board. Flash the matching image to the
   controlled spare card, complete first boot/setup, and open the transfer
   service again.
3. Upload one archive using the [documented restore command](../data-backup-restore.md#pre-flash-and-restore-flow).
   Press Main to apply only after staged validation says confirmation is
   required; use Back for the cancel case if time allows. Wait for the terminal
   result.

### Observation and implicit coverage

Record `backup/export-result.txt`, `backup/restore-result.txt`, archive hashes,
physical Main/Back choice, image SHA, and final board state. A pass requires
no-media restore, media-inclusive restore when claimed, physical confirmation
behavior, and recovery after the controlled reflash. Do not record the transfer
code or archive contents in plain-text evidence.

The end-to-end paths already covered first boot, setup AP, native handoff, and
ordinary runtime after setup. This stage adds export, archive validation, staged
restore, physical confirmation, and post-image-update data continuity.

If the time box expires, the mandatory follow-up still includes invalid and
incompatible archive reporting, rejected-restore data preservation, no-media and
media-inclusive restore, Main/Back confirmation, and recovery after a reflash.

## Remaining board-specific checks and closeout

**Time box: 02:25–02:40.** Use remaining minutes only for checks not covered
above:

- **Raspberry:** physical four-corner/lower-left grid orientation, full LED
  color and coordinate sweep, all four NeoKeys, all encoder directions/clicks,
  and enclosure/port fit as time permits.
- **Orange:** live H618 pinmux/interrupt ownership, I2S/DAC identity, USB-C role
  and no-backfeed gate, full grid/NeoKey/encoder checks, and enclosure fit.
- **Either board:** long OLED sleep/resume, brightness, repeated lifecycle, and
  sustained LED/display behavior remain separate evidence, not implied by one
  clean boot.

Copy final logs into `closeout/`. Retain the Orange probe directory and
Raspberry diagnostic evidence. Create `final-pass-fail.tsv` from the [orchestrator's
result matrix](fat-quick-run.md#result-matrix-and-closeout), and do not delete
failed evidence. Record every item as `PASS`, `FAIL`, or `NOT RUN` with one
sentence and an evidence filename.

Stop at unsafe power, uncertain wiring, a blank/flickering OLED, unexpected board
identity, an actual diagnostic error, or an enclosure that needs force. Do not
repeat boot, setup, one known sound, or basic runtime controls: the remaining
list exists because those details were not proven by the broad path.

The final attachment is an **open FAT result**. Do not write “FAT complete”
until all mandatory rows, including USB policy and Data Backup/Restore, have
exact-image, assembled-board evidence and a human release decision.
