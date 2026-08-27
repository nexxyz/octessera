# Two-board FAT quick run

This is a strict 2–3 hour first-pass Factory Acceptance Test (FAT) for one
Raspberry Pi Zero 2 W and one Orange Pi Zero 2W. It creates useful evidence
quickly; it does **not** close FAT or turn either board into a supported release.
Use the exact release images, assembled boards, power paths, and PCB revisions.

The detailed procedures are split by responsibility:

- [software and musical-function coverage](fat-software-coverage.md);
- [fixed-board diagnostic harness](fat-diagnostic-harness.md);
- [Raspberry and Orange end-to-end paths](fat-board-end-to-end.md); and
- [USB, lifecycle, standalone Backup/Restore, and remaining gap tests](fat-gap-tests.md).

## v0.8.1 exact inputs

Use a clean, detached checkout/worktree at `v0.8.1` for the software matrix and
source checks. Its `HEAD` must be
`256efe5ceb5095b83e7e784b66b15f9eada57d25`; `git status --porcelain` must be
empty. For physical FAT, keep the downloaded release assets outside the repo
and the evidence directory outside both the repo and those assets.

| Input | Exact value |
|---|---|
| Release tag | `v0.8.1` |
| Raspberry image | `octessera-0.8.1-raspberry-pi-zero-2w.img.zip` — `27207ec0d6d55b15a34266e96970160350a7aa644712c3fdc085e8b0c90132d9` |
| Orange image | `octessera-0.8.1-orange-pi-zero-2w.img.xz` — `3ac6766d540981c4b20596b30ca8df41de1dbb3f629eb92748e28096ad36dcc8` |
| Root checksum file | `SHA256SUMS.txt` — `725c210eaab4b59f3312ec711f2f00cfabb21fe73144c6fa92623b88ec231d01` |
| Release evidence | `octessera-0.8.1-release-evidence.zip` — `aef00af247014bf82474c89f060580cb6e665c3c814d705d4cd2131faa3d9685` |

The release record contains the [strict diagnostic and release run links](../release-records/v0.8.1.md).
Run `33037951901` is only the Orange strict diagnostic-image build. Run
`33045139129` is the full release run for both production images and the 14
uploaded assets.

## Time box

| Time | Stage | Board order | Procedure |
|---|---|---|---|
| 00:00–00:15 | Software matrix, identity, image, checksum, evidence setup | Both | [Software coverage](fat-software-coverage.md) |
| 00:15–00:50 | First end-to-end path | Raspberry | [Board end-to-end](fat-board-end-to-end.md#raspberry-end-to-end) |
| 00:50–01:25 | First end-to-end path | Orange | [Board end-to-end](fat-board-end-to-end.md#orange-end-to-end) |
| 01:25–01:45 | Dedicated USB Audio/MIDI gap | Both, only if safe and authorized | [Gap tests](fat-gap-tests.md#usb-audiomidi) |
| 01:45–02:00 | Reboot, shutdown, and recovery | Both | [Gap tests](fat-gap-tests.md#reboot-shutdown-and-recovery) |
| 02:00–02:25 | Standalone Data Backup/Restore around a controlled image update | Optional; otherwise mandatory follow-up | [Gap tests](fat-gap-tests.md#data-backuprestore) |
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

The clean detached `v0.8.1` checkout is needed for the software matrix and
source checks only. For physical FAT, keep the downloaded assets outside the
repo and create one new evidence directory outside both the repo and those
assets:

```powershell
$ReleaseRoot = (Resolve-Path "<directory containing the downloaded v0.8.1 assets>").Path
$EvidenceParent = (Resolve-Path "<existing directory outside this repo and the downloaded assets>").Path
$EvidenceStamp = [DateTime]::UtcNow.ToString("yyyyMMddTHHmmssZ")
$Evidence = Join-Path $EvidenceParent ("octessera-fat-" + $EvidenceStamp)
if (Test-Path -LiteralPath $Evidence) { throw "Evidence root already exists: $Evidence" }
$ExpectedHashes = [ordered]@{
  "SHA256SUMS.txt" = "725c210eaab4b59f3312ec711f2f00cfabb21fe73144c6fa92623b88ec231d01"
  "octessera-0.8.1-raspberry-pi-zero-2w.img.zip" = "27207ec0d6d55b15a34266e96970160350a7aa644712c3fdc085e8b0c90132d9"
  "octessera-0.8.1-orange-pi-zero-2w.img.xz" = "3ac6766d540981c4b20596b30ca8df41de1dbb3f629eb92748e28096ad36dcc8"
}

New-Item -ItemType Directory -Path $Evidence | Out-Null

$facts = @("release_tag=v0.8.1", "source_sha=256efe5ceb5095b83e7e784b66b15f9eada57d25")
foreach ($name in $ExpectedHashes.Keys) {
  $path = Join-Path $ReleaseRoot $name
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Missing exact release file: $path" }
  $actual = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($actual -ne $ExpectedHashes[$name]) { throw "SHA256 mismatch for ${name}: $actual" }
  $facts += "$name`tsha256=$actual`tstatus=PASS"
}

$verificationFile = Join-Path $Evidence "00-artifact-verification.txt"
$facts | Set-Content -LiteralPath $verificationFile -Encoding UTF8

foreach ($name in @("software", "raspberry", "orange", "usb", "lifecycle", "backup", "closeout")) {
  New-Item -ItemType Directory -Path (Join-Path $Evidence $name) | Out-Null
}
```

This writes the three direct SHA-256 results to `00-artifact-verification.txt`
and creates the seven stage directories. It verifies downloaded bytes only; it
does not prove that either image was flashed to a board or close physical FAT.

Keep `.oct` archives outside the shared evidence tree and record only their
protected path and hash. Do not copy credentials, transfer codes, raw service
logs, or unsanitized metadata into shared evidence. If raw logs are needed for
debugging, keep them outside the shared tree and review/redact excerpts first.

## Ordered stages

1. Run the [software coverage matrix](fat-software-coverage.md) from the exact
   release checkout and save its four command results under `software/`. Do not
   replay passed algorithms.
2. Complete identity and bench safety above.
3. Complete the [Raspberry end-to-end](fat-board-end-to-end.md#raspberry-end-to-end)
   path, then the [Orange end-to-end](fat-board-end-to-end.md#orange-end-to-end)
   path. Each path owns exactly one profile diagnostic, after setup has provided
   normal-WLAN network access and SSH, using the [fixed-board diagnostic
   harness](fat-diagnostic-harness.md). Record every required observation before
   moving on.
4. Run the distinct [USB Audio/MIDI, lifecycle, and standalone Backup/Restore gap
   tests](fat-gap-tests.md). Do not use a DAC/Jack tone as USB evidence.
5. Use the remaining slot for [board-specific checks and
   closeout](fat-gap-tests.md#remaining-board-specific-checks-and-closeout).

## Implicit-coverage rule

Count a check only once, against the evidence that actually proves it:

| Earlier evidence | It credits | It does not credit |
|---|---|---|
| Software commands | Automated musical, algorithmic, protocol, data, image, and routing contracts | Physical controls, OLED readability, sound, image flashing, or board wiring |
| One Raspberry end-to-end path | Raspberry boot, native handoff, setup, basic controls, selected-route audio, and one runtime sound | Full coordinates/LEDs/keys/encoders, USB, lifecycle teardown, or Backup/Restore |
| One Orange end-to-end path | The same shared path plus the recorded Orange boot path, adapter, service-account/state, required device paths, route/readiness, and passive UDC state | Exact flashed-card identity, host USB behavior, pinmux/wiring, full physical sweep, or Backup/Restore |
| Diagnostic harness | Sanitized automated board/profile, service, readiness, route, store, and passive USB facts where status is `pass` | Exact flashed image/constructor, sound, OLED appearance, or physical input |
| DAC/Jack sound | Ordinary selected-route audio | USB Audio or USB MIDI |
| Normal boot/service start | Startup | Reboot, shutdown, cold recovery, or repeated lifecycle |

The end-to-end procedures contain their own implicit-coverage tables. Later gap
tests must test only the new behavior. Operator notes never turn an automated
`operator_required`, `not_run`, or `fail` into `pass`.

The production-safe Orange diagnostic does not establish pinmux or wiring. Keep
those checks in board-specific operator evidence.

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
| Live board/profile and compiled binary identity |  |  | `*/00-fat-diagnostic/01-identity.txt`, `*/00-fat-diagnostic/fat-diagnostic.json` |
| Exact image and flashed-card identity | `NOT RUN — exact-card evidence is not attached` | `NOT RUN — exact-card evidence is not attached` | `00-artifact-verification.txt`, `00-inputs.txt` |
| Boot, service, native handoff |  |  | `raspberry/00-fat-diagnostic/`, `orange/00-fat-diagnostic/` |
| OLED first boot and normal menu |  |  | `raspberry/02-boot-oled.*`, `orange/02-boot-oled.*` |
| Setup portal and reconnect |  |  | `*/03-setup-portal.txt` |
| Setup-status hygiene | `NOT RUN — setup-status artifact is not collected` | `NOT RUN — setup-status artifact is not collected` | `*/00-fat-diagnostic/05-setup-status.txt` |
| Basic controls and native runtime |  |  | `*/05-controls-audio.txt`, `*/04-runtime-log.txt` |
| Selected ordinary audio route |  |  | `*/05-controls-audio.txt` |
| USB Audio and USB MIDI gap |  |  | `usb/` |
| Reboot |  |  | `lifecycle/*-reboot.txt` |
| Shutdown and cold recovery |  |  | `lifecycle/shutdown-recovery.txt` |
| Data Backup/Restore around image update |  |  | `backup/` |
| Full coordinates, LEDs, keys, encoders | `NOT RUN — operator sweep is not recorded` | `NOT RUN — operator sweep is not recorded` | `closeout/` |
| Long OLED lifecycle/brightness |  |  | `closeout/` |
| Board-specific port, pinmux, I2S, enclosure |  |  | `closeout/` |

Copy final logs into `closeout/`; retain `orange/00-fat-diagnostic/` and
`raspberry/00-fat-diagnostic/` diagnostic evidence, and do not delete failed
evidence. Attach the matrix and evidence to the release record only as an **open
FAT result**. Do not write
“FAT complete” until the remaining mandatory rows, including USB policy and Data
Backup/Restore, have exact-image, assembled-board evidence and a human release
decision.
