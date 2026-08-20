# Two-board FAT quick run

This is a strict 2–3 hour first-pass Factory Acceptance Test (FAT) for one
Raspberry Pi Zero 2 W and one Orange Pi Zero 2W. It creates useful evidence
quickly; it does **not** close FAT or turn either board into a supported release.
Use the exact release images, assembled boards, power paths, and PCB revisions.

The detailed procedures are split by responsibility:

- [software and musical-function coverage](fat-software-coverage.md);
- [fixed-board diagnostic harness](fat-diagnostic-harness.md);
- [Raspberry and Orange end-to-end paths](fat-board-end-to-end.md); and
- [USB, lifecycle, Backup/Restore, and remaining gap tests](fat-gap-tests.md).

## Time box

| Time | Stage | Board order | Procedure |
|---|---|---|---|
| 00:00–00:15 | Software matrix, identity, image, checksum, evidence setup | Both | [Software coverage](fat-software-coverage.md) |
| 00:15–00:50 | First end-to-end path | Raspberry | [Board end-to-end](fat-board-end-to-end.md#raspberry-end-to-end) |
| 00:50–01:25 | First end-to-end path | Orange | [Board end-to-end](fat-board-end-to-end.md#orange-end-to-end) |
| 01:25–01:45 | Dedicated USB Audio/MIDI gap | Both, only if safe and authorized | [Gap tests](fat-gap-tests.md#usb-audiomidi) |
| 01:45–02:00 | Reboot, shutdown, and recovery | Both | [Gap tests](fat-gap-tests.md#reboot-shutdown-and-recovery) |
| 02:00–02:25 | Data Backup/Restore around a controlled image update | Optional; otherwise mandatory follow-up | [Gap tests](fat-gap-tests.md#data-backuprestore) |
| 02:25–02:40 | Remaining board-specific checks and closeout | Both | [Gap tests](fat-gap-tests.md#remaining-board-specific-checks-and-closeout) |

The first end-to-end path is deliberately broad. Do not repeat its boot, native
handoff, controls, selected-route audio, or runtime checks; later stages cover
gaps only.

## Prerequisites, safety, and evidence

Run with both boards open and accessible. Read [safety and power](safety-and-power.md)
first. Never put a Wi-Fi password, SSH private key, transfer code, or backup
archive in shared evidence.

- Label the boards `RPI` and `OPI`. Record board revision, PCB/harness revision,
  power supply, card label, operator, image filename, image SHA-256, and start
  time in `00-inputs.txt`.
- Confirm release asset names and recorded SHA-256 values against the release
  page before selecting a card in a flasher. Use the matching image for each
  board; do not use the Orange diagnostic image or a Raspberry asset on Orange.
- Keep NeoTrellis/NeoKey `INT` on the south side. Use the enclosure USB-C
  breakout as power input; do not use Raspberry micro-USB power and do not
  attach a host-data cable yet.
- Leave first-boot Wi-Fi unset so the setup portal can be exercised.
- Stop before power if a connector, pin role, board identity, image, checksum, or
  no-backfeed path is unclear.
- Mark `identity=PASS` only when the physical board, exact image, checksum, and
  evidence record agree.

### Prepare the evidence folder

Run this from the repository root. The [evidence preparation
script](../../tools/fat/prepare-evidence.ps1) hashes the exact image bytes and
optional checksum assets; it never flashes, reboots, changes a board, or runs a
restore:

```powershell
.\tools\fat\prepare-evidence.ps1 `
  -Operator "NAME" `
  -Version "0.8.0" `
  -ReleaseTag "v0.8.0" `
  -RaspberryImage ".\octessera-<version>-raspberry-pi-zero-2w.img.zip" `
  -OrangeImage ".\octessera-<version>-orange-pi-zero-2w.img.xz" `
  -RaspberryChecksum ".\SHA256SUMS.txt" `
  -OrangeChecksum ".\<matching-orange-image-checksum-asset>"
```

Use the actual release filenames. If a checksum asset has a different name,
pass that exact file. `-ReleaseTag` must resolve to the current checkout HEAD;
use `-ExpectedSourceSha` with the full 40-character commit SHA instead when
working from a source checkout without that tag. The helper creates
`00-session.json`, `00-git-sha.txt`, `00-version.txt`, `00-operator.txt`,
`00-created-utc.txt`, `00-image-hashes.tsv`, and
`00-destructive-commands.txt` under `artifacts/fat/<UTC-stamp>/` (or the
explicit `-EvidenceRoot`).

For each image, run the exact PowerShell image/checksum comparison in [Orange
first-boot image verification](orange-pi-first-boot.md#verify-the-selected-image)
with that release's matching checksum asset.

Set `$Evidence` to the printed folder, record the path as
`00-evidence-path.txt`, and create these destinations before the board run. The
parent evidence root and every destination must be new, ordinary directories;
do not add `-Force` or reuse an earlier run:

```powershell
$Evidence = "<path printed by prepare-evidence.ps1>"
foreach ($name in @("software", "raspberry", "orange", "usb", "lifecycle", "backup", "closeout")) {
  $path = Join-Path $Evidence $name
  if (Test-Path -LiteralPath $path) { throw "Evidence destination already exists: $path" }
  New-Item -ItemType Directory -Path $path | Out-Null
  $item = Get-Item -LiteralPath $path
  if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw "Evidence destination is a reparse point: $path"
  }
}
```

Keep `.oct` archives outside the shared evidence tree and record only their
protected path and hash. Do not copy credentials, transfer codes, raw service
logs, or unsanitized metadata into shared evidence. If raw logs are needed for
debugging, keep them outside the shared tree and review/redact excerpts first.

## Ordered stages

1. Run the [software coverage matrix](fat-software-coverage.md) from the exact
   release checkout and save its four command results under `software/`. Do not
   replay passed algorithms.
2. Complete identity and bench safety above. Run the [fixed-board diagnostic
   harness](fat-diagnostic-harness.md) once per board after SSH is available.
   Harness output is evidence, not physical qualification.
3. Complete the [Raspberry end-to-end](fat-board-end-to-end.md#raspberry-end-to-end)
   path, then the [Orange end-to-end](fat-board-end-to-end.md#orange-end-to-end)
   path. Record every required observation before moving on.
4. Run the distinct [USB Audio/MIDI, lifecycle, and Backup/Restore gap
   tests](fat-gap-tests.md). Do not use a DAC/Jack tone as USB evidence.
5. Use the remaining slot for [board-specific checks and
   closeout](fat-gap-tests.md#remaining-board-specific-checks-and-closeout).

## Implicit-coverage rule

Count a check only once, against the evidence that actually proves it:

| Earlier evidence | It credits | It does not credit |
|---|---|---|
| Software commands | Automated musical, algorithmic, protocol, data, image, and routing contracts | Physical controls, OLED readability, sound, image flashing, or board wiring |
| One Raspberry end-to-end path | Raspberry boot, native handoff, setup, basic controls, selected-route audio, and one runtime sound | Full coordinates/LEDs/keys/encoders, USB, lifecycle teardown, or Backup/Restore |
| One Orange end-to-end path | The same shared path plus Orange image, adapter, pin, service-account, device-node, and passive UDC facts | Host USB behavior, full physical sweep, or Backup/Restore |
| Diagnostic harness | Sanitized automated board/profile, service, readiness, route, store, and passive USB facts where status is `pass` | Exact flashed image/constructor, sound, OLED appearance, or physical input |
| DAC/Jack sound | Ordinary selected-route audio | USB Audio or USB MIDI |
| Normal boot/service start | Startup | Reboot, shutdown, cold recovery, or repeated lifecycle |

The end-to-end procedures contain their own implicit-coverage tables. Later gap
tests must test only the new behavior. Operator notes never turn an automated
`operator_required`, `not_run`, or `fail` into `pass`.

## Stop conditions

Stop the run, preserve evidence, and record `FAIL` or `NOT RUN` rather than
trying an uncontrolled second path when there is:

- unsafe power, uncertain wiring, a blank or flickering OLED, unexpected board
  identity/profile, an actual diagnostic error, or an enclosure that needs force;
- a missing required device, unclear port role, unstable OLED, unsafe power, or
  unresolved Orange I2S/DAC identity;
- no authorized USB identity, port role, VBUS/CC behavior, or no-backfeed gate;
  never connect a host cable when that gate is missing;
- insufficient time, a missing controlled spare card, or unverified off-board
  storage for Backup/Restore; do not squeeze in an uncontrolled reflash.

## Result matrix and closeout

Use one row per board where the check is board-specific. `PASS` means physical
evidence exists in this run. `NOT RUN` is not a pass. Record `PASS`, `FAIL`, or
`NOT RUN` with one sentence and an evidence filename.

| Check | Raspberry | Orange | Evidence |
|---|---|---|---|
| Automated software and musical-function matrix |  |  | `software/` |
| Live board/profile and compiled binary identity |  |  | `*/00-fat-diagnostic/01-identity.txt`, `00-session.json` |
| Exact image flashed and constructor proof | `PENDING` unless exact-card and constructor evidence is attached | `PENDING` unless exact-card and constructor evidence is attached | `00-image-hashes.tsv`, `00-inputs.txt`, image/constructor records |
| Boot, service, native handoff |  |  | `raspberry/00-fat-diagnostic/`, `orange/01-bringup/` |
| OLED first boot and normal menu |  |  | `raspberry/02-boot-oled.*`, `orange/02-boot-oled.*` |
| Setup portal and reconnect |  |  | `*/03-setup-portal.txt` |
| Setup-status hygiene | `PENDING` unless `05-setup-status.txt` is collected and passes | `PENDING` unless `05-setup-status.txt` is collected and passes | `*/00-fat-diagnostic/05-setup-status.txt` |
| Basic controls and native runtime |  |  | `*/05-controls-audio.txt`, `*/04-runtime-log.txt` |
| Selected ordinary audio route |  |  | `*/05-controls-audio.txt` |
| USB Audio and USB MIDI gap |  |  | `usb/` |
| Reboot |  |  | `lifecycle/*-reboot.txt` |
| Shutdown and cold recovery |  |  | `lifecycle/shutdown-recovery.txt` |
| Data Backup/Restore around image update |  |  | `backup/` |
| Full coordinates, LEDs, keys, encoders | `PENDING` operator evidence | `PENDING` operator evidence | `closeout/` |
| Long OLED lifecycle/brightness |  |  | `closeout/` |
| Board-specific port, pinmux, I2S, enclosure |  |  | `closeout/`, Orange probe |

Copy final logs into `closeout/`; retain the Orange probe directory and
Raspberry diagnostic evidence, and do not delete failed evidence. Attach the matrix
and evidence to the release record only as an **open FAT result**. Do not write
“FAT complete” until the remaining mandatory rows, including USB policy and Data
Backup/Restore, have exact-image, assembled-board evidence and a human release
decision.
