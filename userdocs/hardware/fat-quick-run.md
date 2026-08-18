# Two-board FAT quick run

This is a strict 2–3 hour first-pass Factory Acceptance Test (FAT) for one
Raspberry Pi Zero 2 W and one Orange Pi Zero 2W. It creates useful evidence
quickly; it does **not** close FAT or turn either board into a supported release.
Use the exact release images, assembled boards, power paths, and PCB revisions.

Run with the boards open and accessible. Read [safety and power](safety-and-power.md)
first. Never put a Wi-Fi password, SSH private key, transfer code, or backup
archive in shared evidence.

## Time box and evidence

| Time | Stage | Board order |
|---|---|---|
| 00:00–00:15 | Identity, image, checksum, and evidence setup | Both |
| 00:15–00:50 | First end-to-end path | Raspberry |
| 00:50–01:25 | First end-to-end path | Orange |
| 01:25–01:45 | Dedicated USB Audio/MIDI gap test | Both, only if safe and authorized |
| 01:45–02:00 | Reboot, shutdown, and recovery | Both |
| 02:00–02:25 | Data Backup/Restore around a controlled image update | Optional; otherwise mandatory follow-up |
| 02:25–02:40 | Remaining board-specific checks and closeout | Both |

The first end-to-end path is deliberately broad. Do not repeat its boot,
service, native handoff, basic controls, selected-route audio, or ordinary
runtime checks in later stages. Later stages cover gaps only.

Create the evidence folder from the repository root. The helper hashes the
exact image bytes and optional checksum assets; it never flashes, reboots,
changes a board, or runs a restore:

```powershell
.\tools\fat\prepare-evidence.ps1 `
  -Operator "NAME" `
  -Version "0.8.0" `
  -RaspberryImage ".\octessera-<version>-raspberry-pi-zero-2w.img.zip" `
  -OrangeImage ".\octessera-<version>-orange-pi-zero-2w.img.xz" `
  -RaspberryChecksum ".\SHA256SUMS.txt" `
  -OrangeChecksum ".\<matching-orange-image-checksum-asset>"
```

Use the actual release filenames. If a checksum asset has a different name,
pass that exact file. The helper creates `00-session.json`, `00-git-sha.txt`,
`00-version.txt`, `00-operator.txt`, `00-created-utc.txt`,
`00-image-hashes.tsv`, and `00-destructive-commands.txt` under
`artifacts/fat/<UTC-stamp>/` (or the explicit `-EvidenceRoot`).
For each image, also run the exact PowerShell image/checksum comparison in
[Orange first-boot image verification](orange-pi-first-boot.md#verify-the-selected-image),
using the matching checksum asset for that release.

Set `$Evidence` to the printed folder, then create the named subfolders before
the board run:

```powershell
$Evidence = "<path printed by prepare-evidence.ps1>"
New-Item -ItemType Directory -Force -Path @(
  (Join-Path $Evidence "raspberry"),
  (Join-Path $Evidence "orange"),
  (Join-Path $Evidence "usb"),
  (Join-Path $Evidence "lifecycle"),
  (Join-Path $Evidence "backup"),
  (Join-Path $Evidence "closeout")
) | Out-Null
```

## Step 1 — identity and bench safety (00:00–00:15)

### AUTOMATED PREP/CAPTURE

- Run the helper above and save its printed path as `00-evidence-path.txt`.
- Confirm the release asset names and recorded SHA-256 values against the
  release page before selecting a card in a flasher.
- Label the boards `RPI` and `OPI`. Record board revision, PCB/harness revision,
  power supply, card label, operator, image filename, image SHA-256, and start
  time in `00-inputs.txt`.

### OPERATOR ACTION

- Keep both boards unboxed. Confirm NeoTrellis/NeoKey `INT` is on the south side.
- Use the enclosure USB-C breakout as power input. Do not use Raspberry micro-USB
  power. Do not attach a host-data cable yet.
- Leave first-boot Wi-Fi unset so the setup portal can be exercised.

### OPERATOR OBSERVATION

- Stop before power if a connector, pin role, board identity, image, checksum,
  or no-backfeed path is unclear.
- Mark `identity=PASS` only when the physical board, exact image, checksum, and
  evidence record agree.

### ALREADY COVERED BY PRIOR TEST

- Nothing. This is the evidence baseline, not a runtime qualification.

## Step 2 — Raspberry end-to-end path (00:15–00:50)

### AUTOMATED PREP/CAPTURE

After the operator actions below give the board network access, run the existing
Raspberry preflight. Save the output as `raspberry/01-preflight.txt`:

```powershell
.\tools\pi\pi-preflight.ps1 -Target pi@192.168.0.218 | Tee-Object -FilePath (Join-Path $Evidence "raspberry\01-preflight.txt")
```

Capture the service result and recent journal as `raspberry/04-runtime-log.txt`
with the existing fixed SSH transport. Do not record credentials:

```powershell
.\tools\pi\with-pi-ssh.ps1 ssh pi@192.168.0.218 "systemctl --no-pager status octessera.service; journalctl -u octessera.service --since '20 minutes ago' --no-pager" | Tee-Object -FilePath (Join-Path $Evidence "raspberry\04-runtime-log.txt")
```

### OPERATOR ACTION

1. Flash the exact Raspberry asset with Raspberry Pi Imager. Insert the card and
   power only through the enclosure USB-C input.
2. Watch the OLED from power-on through the normal menu. Join `Octessera Setup`
   or `Octessera Setup <4-char code>` and open `http://192.168.42.1`.
3. Apply a test Wi-Fi network, hostname, and SSH key if needed. Wait for the
   hotspot to disappear and reconnect on the new network.
4. At the instrument, turn and press the Main encoder, press Back and Space,
   press one lower-left grid cell, start the default patch, and make one small
   parameter change. Do not start a full 64-cell or LED orientation sweep here.
5. With safe volume and the selected DAC connected, trigger one known default
   patch sound. If a direct route check is needed, use the existing tone command:

   ```bash
   timeout 15 speaker-test -D hw:0,0 -c 2 -t sine -f 440 -l 1
   ```

### OPERATOR OBSERVATION

Record `raspberry/02-boot-oled.jpg` or `.mp4`,
`raspberry/03-setup-portal.txt`, `raspberry/05-controls-audio.txt`, and the
runtime log. This one path credits:

| Observation | What it validates in this path |
|---|---|
| Exact card boots and reaches the menu | Boot image, service start, native handoff, and first runtime snapshot |
| OLED remains readable with one normal owner | OLED initialization and boot-to-native handoff |
| AP, page, apply, and network reconnect work | First-boot setup portal and network adapter path |
| Main/NeoKey/grid input changes the live UI | Basic controls, input routing, and runtime message handoff |
| Default patch produces the known sound on the selected DAC | Selected audio routing, audio device open, realtime audio, and runtime action |
| No service failure or restart loop appears in the capture | Service lifecycle and native runtime startup |

Mark this stage `PASS` only when all listed observations are present. A source
probe or desktop simulator cannot replace these observations.

### ALREADY COVERED BY PRIOR TEST

- None within this board. Do not repeat these Raspberry checks in the USB,
  lifecycle, or closeout stages; those stages test different gaps.

## Step 3 — Orange end-to-end path (00:50–01:25)

### AUTOMATED PREP/CAPTURE

Run the existing read-only Orange bring-up probe after SSH is available. It
copies its timestamped log into the named evidence directory and does not bind
the USB gadget by default:

```powershell
.\tools\orange-pi\run-opi-bringup.ps1 `
  -Target octessera@192.168.0.217 `
  -LocalOutputDir (Join-Path $Evidence "orange\01-bringup")
```

After the first sound, capture the production service and image metadata as
`orange/04-runtime-log.txt`:

```powershell
.\tools\orange-pi\with-orange-ssh.ps1 ssh octessera@192.168.0.217 "systemctl --no-pager status octessera.service; journalctl -u octessera.service --since '20 minutes ago' --no-pager; cat /etc/octessera/build-metadata.env" | Tee-Object -FilePath (Join-Path $Evidence "orange\04-runtime-log.txt")
```

### OPERATOR ACTION

1. Flash the exact Orange production image with the selected image flasher. Do
   not use the diagnostic image or a Raspberry asset.
2. Watch the OLED from power-on through the normal menu. Join the setup hotspot,
   open `http://192.168.42.1`, apply a test Wi-Fi network and SSH key, and wait
   for the reconnect.
3. Turn and press the Main encoder, press Back and Space, press one lower-left
   grid cell, start the default patch, and make one small parameter change.
4. With the selected Jack DAC connected and safe volume, trigger one known
   default patch sound. The documented selected Jack route is
   `hw:CARD=octesseradac,DEV=0`; use the existing short tone shape if needed:

   ```bash
   timeout 15 speaker-test -D hw:CARD=octesseradac,DEV=0 -c 2 -t sine -f 440 -l 1
   ```

### OPERATOR OBSERVATION

Record the probe output, `orange/02-boot-oled.jpg` or `.mp4`,
`orange/03-setup-portal.txt`, `orange/05-controls-audio.txt`, and the runtime
log. This path credits the same boot, service, OLED, setup, basic controls,
selected-route audio, and native runtime coverage as Step 2, plus Orange image
identity, Armbian service account, device nodes, and passive UDC/pin facts.

Stop if the probe reports an unexpected board/profile, missing required device,
unclear port role, unstable OLED, unsafe power, or unresolved I2S/DAC identity.

### ALREADY COVERED BY PRIOR TEST

- The Raspberry end-to-end path covered the shared native flow. This step adds
  Orange adapter, image, pin, service-account, and selected-route evidence; it
  does not require a second shared-runtime demonstration.

## Step 4 — dedicated USB Audio/MIDI gap test (01:25–01:45)

USB is a separate qualification gap. Do not count the DAC/Jack sound in Steps 2
or 3 as USB evidence. USB Audio and USB MIDI remain experimental local-bench
paths, not public support.

### AUTOMATED PREP/CAPTURE

- Save the Orange passive USB state from the Step 3 probe as
  `usb/orange-passive-state.txt`.
- On Orange, if the exact production service is running, capture:

  ```bash
  systemctl status octessera-orange-usb-gadget.service
  cat /sys/class/udc/musb-hdrc.4.auto/function
  ls /sys/kernel/config/usb_gadget/octessera-orange-pi/functions
  ```

- Prepare `usb/raspberry-host-lsusb-v.txt`, `usb/orange-host-lsusb-v.txt`,
  `usb/raspberry-midi.txt`, and `usb/orange-midi.txt`. Capture host output only
  after the electrical gate passes.

### OPERATOR ACTION

1. Before either host cable, confirm the authorized USB identity, the exact
   board port role, VBUS/CC behavior, and no-backfeed path. Use a data-only or
   power-isolating cable when the instrument has separate power.
2. If any gate or identity is missing, write `NOT RUN — unsafe or unauthorized`
   in the matrix and do not connect the host.
3. If authorized, enable USB Audio and/or USB MIDI in `System > Audio & USB`,
   use `Save & Reboot`, and test each board separately.
4. On the host, capture `lsusb -v`, confirm the intended audio device, and send
   one MIDI note from the intended host application. Unplug/replug once and
   record whether enumeration and the intended function recover.

### OPERATOR OBSERVATION

Pass requires all of: safe electrical behavior, expected host enumeration,
intended UAC2 audio, intended MIDI naming and note delivery, reconnect behavior,
and no mass-storage function. Record the authorized identity and host/board
names in `usb/identity-and-port.txt`, never credentials.

### ALREADY COVERED BY PRIOR TEST

- Steps 2 and 3 already covered ordinary selected-route audio. Do not replay a
  DAC/Jack tone here; only USB enumeration, USB audio, USB MIDI, and reconnect
  are new evidence.

## Step 5 — reboot, shutdown, and recovery (01:45–02:00)

### AUTOMATED PREP/CAPTURE

Prepare `lifecycle/raspberry-reboot.txt`, `lifecycle/orange-reboot.txt`,
`lifecycle/shutdown-recovery.txt`, and capture the service state after each
return. Do not use arbitrary administrative power commands as a substitute for
the confirmed instrument actions.

### OPERATOR ACTION

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

### OPERATOR OBSERVATION

Pass means the requested menu action shows the native lifecycle message, audio
and external MIDI stop safely, the OLED is not left with two writers, and the
board returns to the same known menu/runtime state. Record any power, brownout,
or recovery fault rather than trying another command path.

### ALREADY COVERED BY PRIOR TEST

- Boot and normal service start were covered in Steps 2 and 3. This step covers
  lifecycle teardown, power submission, and return/recovery only.

## Step 6 — optional Data Backup/Restore around a controlled image update (02:00–02:25)

Run this only when a spare/controlled card, verified off-board storage, and at
least 25 minutes remain. This slot covers one board; a full two-board result
needs a separate 25-minute slot for the other board. Otherwise mark the entire
row `NOT RUN — mandatory FAT follow-up`; do not squeeze in an uncontrolled
reflash. This is a board-side transfer path on Raspberry Pi and Orange Pi,
desktop transfer is unsupported. Use the [Data Backup and Restore](../data-backup-restore.md)
page for the exact upload command.

### AUTOMATED PREP/CAPTURE

- Prepare `backup/pre-flash-data-only.sha256` and
  `backup/pre-flash-media.sha256`. Keep the `.oct` archives outside the shared
  evidence folder and record only their protected path and hash.
- Print-only destructive reminders are in `00-destructive-commands.txt`.

### OPERATOR ACTION

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
3. Upload one archive using the documented restore shape. Press Main to apply
   only after the staged validation says confirmation is required; use Back for
   the cancel case if time allows. Wait for the terminal result.

### OPERATOR OBSERVATION

Record `backup/export-result.txt`, `backup/restore-result.txt`, the archive
hashes, physical Main/Back choice, image SHA, and final board state. A pass
requires no-media restore, media-inclusive restore when claimed, physical
confirmation behavior, and recovery after the controlled reflash. Do not record
the transfer code or archive contents in plain-text evidence.

### ALREADY COVERED BY PRIOR TEST

- Steps 2 and 3 covered first boot, setup AP, native handoff, and ordinary
  runtime after setup. This step adds only export, archive validation, staged
  restore, physical confirmation, and post-image-update data continuity.

If the time box expires, the mandatory follow-up still includes invalid and
incompatible archive reporting, rejected-restore data preservation, no-media and
media-inclusive restore, Main/Back confirmation, and recovery after a reflash.

## Step 7 — remaining board-specific checks and closeout (02:25–02:40)

### AUTOMATED PREP/CAPTURE

Copy the final logs into `closeout/`. For Orange, retain the probe directory;
for Raspberry, retain the preflight output. Create `final-pass-fail.tsv` from
the matrix below. Do not delete failed evidence.

### OPERATOR ACTION

Use the remaining minutes only for checks not covered above:

- Raspberry: physical four-corner/lower-left grid orientation, full LED color
  and coordinate sweep, all four NeoKeys, all encoder directions/clicks, and
  enclosure/port fit as time permits.
- Orange: live H618 pinmux/interrupt ownership, I2S/DAC identity, USB-C role and
  no-backfeed gate, full grid/NeoKey/encoder checks, and enclosure fit.
- Either board: long OLED sleep/resume, brightness, repeated lifecycle, and
  sustained LED/display behavior remain separate evidence, not implied by one
  clean boot.

### OPERATOR OBSERVATION

Record each item as `PASS`, `FAIL`, or `NOT RUN` with one sentence and an
evidence filename. Stop at unsafe power, uncertain wiring, blank/flickering OLED,
unexpected board identity, actual diagnostic error, or an enclosure that needs
force.

### ALREADY COVERED BY PRIOR TEST

- Do not repeat boot, setup, one known sound, or basic runtime controls. The
  remaining list exists because those details were not proven by the broad path.

## Compact pass/fail matrix

Use one row per board where the check is board-specific. `PASS` means physical
evidence exists in this run; `NOT RUN` is not a pass.

| Check | Raspberry | Orange | Evidence |
|---|---|---|---|
| Identity, exact image, checksum |  |  | `00-session.json`, `00-image-hashes.tsv`, `00-inputs.txt` |
| Boot, service, native handoff |  |  | `raspberry/01-preflight.txt`, `orange/01-bringup/` |
| OLED first boot and normal menu |  |  | `raspberry/02-boot-oled.*`, `orange/02-boot-oled.*` |
| Setup portal and reconnect |  |  | `*/03-setup-portal.txt` |
| Basic controls and native runtime |  |  | `*/05-controls-audio.txt`, `*/04-runtime-log.txt` |
| Selected ordinary audio route |  |  | `*/05-controls-audio.txt` |
| USB Audio and USB MIDI gap |  |  | `usb/` |
| Reboot |  |  | `lifecycle/*-reboot.txt` |
| Shutdown and cold recovery |  |  | `lifecycle/shutdown-recovery.txt` |
| Data Backup/Restore around image update |  |  | `backup/` |
| Full coordinates, LEDs, keys, encoders |  |  | `closeout/` |
| Long OLED lifecycle/brightness |  |  | `closeout/` |
| Board-specific port, pinmux, I2S, enclosure |  |  | `closeout/`, Orange probe |

At the end, attach the matrix and evidence to the release record only as an
**open FAT result**. Do not write “FAT complete” until the remaining mandatory
rows, including USB policy and Data Backup/Restore, have their exact-image,
assembled-board evidence and a human release decision.
