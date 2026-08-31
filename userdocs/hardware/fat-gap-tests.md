# FAT gap tests and closeout

These tests cover behavior that the broad [board end-to-end
paths](fat-board-end-to-end.md) deliberately do not repeat. Use the `$Evidence`
folder and safety gates from the [FAT orchestrator](fat-quick-run.md). USB
Audio/MIDI remains experimental local-bench validation, not public support.

## USB Audio/MIDI

**Time box: 01:25–01:45.** USB is a separate qualification gap. Do not count
the DAC/Jack sound in the end-to-end paths as USB evidence.

### Automated preparation and capture

- Copy the Orange passive USB state from the production-safe diagnostic evidence
  in `orange/00-fat-diagnostic/` into `usb/orange-passive-state.txt` for review.
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
   port role, VBUS/CC behavior, and no-backfeed path. For the fixed bench path,
   use a USB-A host or hub port with USB-A-to-USB-C for Orange USB0 or
   USB-A-to-Micro-USB for the Raspberry gadget port; avoid USB-C-to-USB-C/PD.
   These ordinary USB-A cables carry VBUS and are not themselves the no-backfeed
   control.
2. If any gate or identity is missing, write `NOT RUN — unsafe or unauthorized`
   in the result matrix and do not connect the host.
3. If authorized, enable USB Audio and/or USB MIDI in `System > Audio / USB`,
   use `Save / Reboot`, and test each board separately.
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
squeeze in an uncontrolled reflash. Use the standalone `System > Backup / Restore`
action on Pi; desktop is unsupported. See [Data Backup and Restore](../data-backup-restore.md)
for the complete transfer contract. This is separate from the setup-portal FAT.

### Automated preparation and capture

- Prepare `backup/pre-flash-data-only.sha256` and
  `backup/pre-flash-media.sha256`.
- Keep the `.oct` archives outside the shared evidence folder and record only
  their protected path and hash.
- Use the print-only destructive reminders in `00-destructive-commands.txt`.

### Operator action

1. On the source board, choose `System > Backup / Restore`. Read the regular IP,
   port, and 10-character code from the OLED Ready card, then export a data-only
   archive. Export media-inclusive data too when custom samples or recordings
   are part of the claim. Use the displayed values:

   ```sh
   URL="http://<regular-ip>:8081"
   CODE="<10-character code>"
   curl -fL -H "X-Octessera-Transfer-Code: $CODE" -o octessera-user-data.oct "$URL/export"
   curl -fL -H "X-Octessera-Transfer-Code: $CODE" -o octessera-user-data-media.oct "$URL/export?media=1"
   ```

2. Press Back and reopen the action; confirm the same URL and code remain with
   no lifetime extension. After export, select `> Stop service` and confirm the
   code is revoked. If a controlled no-address case is available, confirm the
   action is unavailable without binding or retrying.
3. Verify both archive hashes off-board. Flash the matching image to the
   controlled spare card, complete first boot and regular network setup, then
   choose `System > Backup / Restore` again for the new displayed URL and code.
4. Upload one archive using the [documented restore command](../data-backup-restore.md#pre-flash-and-restore-flow).
   Press Main to apply only after validation says confirmation is required; use
   Back for the cancel case if time allows. Wait for the terminal result.

### Observation and implicit coverage

Record `backup/export-result.txt`, `backup/restore-result.txt`, archive hashes,
physical Main/Back choice, image SHA, card lifetime/reopen behavior, service-stop
result, and final board state. Where testable, record expiry or authentication
revocation closing the service. A pass requires no-media restore, media-inclusive
restore when claimed, physical confirmation behavior, and recovery after the
controlled reflash. Do not record the transfer code or archive contents in
plain-text evidence.

The end-to-end paths already cover first boot, the setup AP, native handoff, and
ordinary runtime separately. This stage adds the standalone transfer, archive
validation, physical confirmation, and post-image-update data continuity.

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

Copy final logs into `closeout/`. Retain `orange/00-fat-diagnostic/` and
`raspberry/00-fat-diagnostic/` diagnostic evidence. Create `final-pass-fail.tsv`
from the [orchestrator's result matrix](fat-quick-run.md#result-matrix-and-closeout),
and do not delete failed evidence. Record every item as `PASS`, `FAIL`, or `NOT
RUN` with one sentence and an evidence filename.

Stop at unsafe power, uncertain wiring, a blank/flickering OLED, unexpected board
identity, an actual diagnostic error, or an enclosure that needs force. Do not
repeat boot, setup, one known sound, or basic runtime controls: the remaining
list exists because those details were not proven by the broad path.

The final attachment is an **open FAT result**. Do not write “FAT complete”
until all mandatory rows, including USB policy and Data Backup/Restore, have
exact-image, assembled-board evidence and a human release decision.
