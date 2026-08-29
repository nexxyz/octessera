# Fixed-board FAT diagnostic harness

The native FAT harness supplies bounded, sanitized evidence for one fixed board
profile. It is not a replacement for the [two-board FAT
orchestrator](fat-quick-run.md), the [Raspberry and Orange end-to-end
paths](fat-board-end-to-end.md), image validators, or the operator-led physical
sweep.

This is the safe diagnostic owner for both fixed boards. Use
`--fat-diagnostic --board-profile <profile>` for the explicit form. The older
Raspberry `--diagnostic` invocation is only a compatibility alias when no
profile options are supplied; it delegates to this same read-only collector,
creates a fresh temporary evidence directory, and is deprecated. A profile-aware
`--diagnostic --board-profile <profile>` invocation uses the explicit form
directly.

The existing profile-aware harness is already the fixed-board owner for this
evidence lane. No new Rust harness is required for v0.8.1. Physical controls,
OLED appearance, audio, wiring, and other bench checks remain operator-led.

`OCTESSERA_PI_DIAGNOSTIC=1` is the matching deprecated compatibility alias for
older service scripts. It selects the same Raspberry-safe collector even when
no command-line diagnostic flag is present. The environment alias is rejected
with `--board-profile`/`--profile`, `--hardware-test`, or
`--hardware-noise-test`; those combinations fail before hardware
initialization. Use an explicit profile command for Orange.

## Requirements and safety

Run exactly one matching diagnostic block per board, only after setup has
provided normal-WLAN network access and SSH. The installed
`/usr/local/bin/octessera-pi` must be the matching canonical hardware build:

| Board | Required build features | Required runtime profile |
|---|---|---|
| Raspberry Pi Zero 2 W | `--no-default-features --features hardware-raspberry-pi-zero-2w` | `raspberry-pi-zero-2w` |
| Orange Pi Zero 2W | `--no-default-features --features hardware-orange-pi-zero-2w` | `orange-pi-zero-2w` |

Default, stub, and deprecated alias builds are rejected. The binary and
`--board-profile` must agree. Do not use a diagnostic image, a Raspberry asset
on Orange, or an Orange asset on Raspberry.

For Orange production, the identity check reads the bounded, regular adjacent
`octessera-runtime.json` for the resolved release executable and verifies its
profile, release version, and executable SHA-256. It does not rerun the
candidate metadata command or fall back to a candidate sidecar.

The harness is non-destructive with respect to persistent board state. It never
flashes, reboots, restores, binds a USB gadget, plays a tone, or writes an
OLED/LED qualification result. It does not open, reset, configure, or scan
NeoTrellis, NeoKey, or encoder hardware. Never stop the production service to
make an automated input check possible; physical input checks remain operator-led.

The default timeout is 30 seconds per command. Use `--timeout-seconds` only
within 1–600 seconds. Replace each `<fresh-run-id>` with a value that does not
already exist on that board.

## Evidence-safe invocation

Use the `$Evidence` folder created by the [orchestrator](fat-quick-run.md).
Each local diagnostic destination must be new and unused. The harness creates
the remote evidence directory and refuses to reuse an existing directory. Do
not use `-Force`, reuse an earlier run, or copy credentials, transfer codes, raw
service logs, or unsanitized metadata into shared evidence.

```powershell
$RaspberryDiagnostic = Join-Path $Evidence "raspberry\00-fat-diagnostic"
if (Test-Path -LiteralPath $RaspberryDiagnostic) { throw "Raspberry diagnostic destination already exists" }
New-Item -ItemType Directory -Path $RaspberryDiagnostic | Out-Null
$raspberryDestination = Get-Item -LiteralPath $RaspberryDiagnostic
if (($raspberryDestination.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw "Raspberry diagnostic destination is a reparse point" }
$RaspberryRemoteDiagnostic = "/tmp/octessera-fat-diagnostic-rpi-<fresh-run-id>"
.\tools\pi\with-pi-ssh.ps1 ssh pi@192.168.0.218 `
  "/usr/local/bin/octessera-pi --fat-diagnostic --board-profile raspberry-pi-zero-2w --evidence-dir $RaspberryRemoteDiagnostic --timeout-seconds 30" `
  | Tee-Object -FilePath (Join-Path $RaspberryDiagnostic "run.txt")
$raspberryDiagnosticExitCode = $LASTEXITCODE

.\tools\pi\with-pi-ssh.ps1 scp `
  "pi@192.168.0.218:$RaspberryRemoteDiagnostic/*" $RaspberryDiagnostic
if ($LASTEXITCODE -ne 0) { throw "Could not copy Raspberry diagnostic evidence" }
$RaspberryReportPath = Join-Path $RaspberryDiagnostic "fat-diagnostic.json"
if (-not (Test-Path -LiteralPath $RaspberryReportPath -PathType Leaf)) { throw "Copied Raspberry diagnostic report is missing" }
$RaspberryReport = Get-Content -LiteralPath $RaspberryReportPath -Raw | ConvertFrom-Json
if ($null -eq $RaspberryReport) { throw "Copied Raspberry diagnostic report is not valid JSON" }
if ($null -eq $RaspberryReport.overall_status) { throw "Copied Raspberry diagnostic report is missing overall_status" }
if ($raspberryDiagnosticExitCode -ne 0) { throw "Raspberry FAT diagnostic failed with exit code $raspberryDiagnosticExitCode" }

$OrangeDiagnostic = Join-Path $Evidence "orange\00-fat-diagnostic"
if (Test-Path -LiteralPath $OrangeDiagnostic) { throw "Orange diagnostic destination already exists" }
New-Item -ItemType Directory -Path $OrangeDiagnostic | Out-Null
$orangeDestination = Get-Item -LiteralPath $OrangeDiagnostic
if (($orangeDestination.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw "Orange diagnostic destination is a reparse point" }
$OrangeTarget = "octessera@<normal-wlan-ip>"
$OrangeSshOptions = @("-o", "StrictHostKeyChecking=yes")
$OrangeRunId = [guid]::NewGuid().ToString("N")
$OrangeRemoteDiagnostic = "/tmp/octessera-fat-diagnostic-orange-$OrangeRunId"
$OrangeDiagnosticCommand = "sudo -H -u octessera-runtime -- /bin/sh -c 'umask 022; exec /usr/local/bin/octessera-pi --fat-diagnostic --board-profile orange-pi-zero-2w --evidence-dir $OrangeRemoteDiagnostic --timeout-seconds 30'"
& ssh @OrangeSshOptions -t $OrangeTarget $OrangeDiagnosticCommand `
  | Tee-Object -FilePath (Join-Path $OrangeDiagnostic "run.txt")
$orangeDiagnosticExitCode = $LASTEXITCODE

& scp @OrangeSshOptions "${OrangeTarget}:$OrangeRemoteDiagnostic/*" $OrangeDiagnostic
if ($LASTEXITCODE -ne 0) { throw "Could not copy Orange diagnostic evidence" }
$OrangeReportPath = Join-Path $OrangeDiagnostic "fat-diagnostic.json"
if (-not (Test-Path -LiteralPath $OrangeReportPath -PathType Leaf)) { throw "Copied Orange diagnostic report is missing" }
$OrangeReport = Get-Content -LiteralPath $OrangeReportPath -Raw | ConvertFrom-Json
if ($null -eq $OrangeReport) { throw "Copied Orange diagnostic report is not valid JSON" }
if ($null -eq $OrangeReport.overall_status) { throw "Copied Orange diagnostic report is missing overall_status" }
if ($orangeDiagnosticExitCode -ne 0) { throw "Orange FAT diagnostic failed with exit code $orangeDiagnosticExitCode" }
```

Before this attended production run, verify the exact Orange host key in the
operator's `known_hosts`; strict checking is intentional and the host key must
already be verified. The normal-WLAN SSH login must work, and attended
password-based `sudo` as `octessera-runtime` must work. Key-only setup is not
enough unless an approved sudo credential or policy already exists. A locked
`nologin` shell for `octessera-runtime` is okay because `sudo` runs the explicit
command. The command uses `umask 022` so the sanitized files can be copied; do
not loosen that to copy raw logs. Keep `octessera.service` active throughout.
Do not add sudoers rules or stop the service.

The harness writes `fat-diagnostic.json`, a tab-separated
`fat-diagnostic.log`, and one sanitized artifact per check. The report's
`overall_status` is one of `pass`, `not_run`, `operator_required`, or `fail`.
`AUTOMATED_PASS=true` does not replace operator-required observations.

## What the output means

| Harness check | Credits in this run | Still requires an operator or existing validator |
|---|---|---|
| Identity/profile, service, readiness, OLED handoff markers | Live model/profile contract, compiled binary metadata, service state, current readiness correlation, and native OLED ownership markers | Exact flashed image bytes, image constructor proof, and readable/stable OLED/normal-menu observation |
| Store/backup paths | Safe path shape and default store readability | Setup-status hygiene unless a real `05-setup-status.txt` observation is present and passes; standalone Backup & Restore workflow and data continuity |
| Audio route status | Expected ALSA card listing and selected route label | Audible sound, safe volume, and physical DAC qualification |
| Physical input observation | No automated credit; the harness records this as operator-required | Presses, turns, clicks, grid orientation, LED color, debounce, and physical wiring |
| USB gadget state | Passive UDC/configfs/service state and fixed Orange UDC identity | Authorized port role, cable/VBUS/CC/no-backfeed gate, host enumeration, Audio/MIDI |
| Artifact/log collection | Sanitized machine-readable report and allowlisted structured service status | Review of failed evidence; never copy credentials, transfer codes, or raw service logs |

Missing or failed USB state, structured service status, or required artifact
collection is `not_run` or `fail`, never an automated pass. Operator-required
input, audio, OLED, image/constructor, setup-status, and physical observations
remain separate from that result. Operator notes never change an automated
check or turn `operator_required` into `pass`.

The harness cannot prove that the recorded image bytes were the bytes flashed to
the board, nor that the board was made by the current image constructor. Those
are separate FAT items and need exact-card and constructor evidence. Absent
setup-status files produce `not_run`; only a collected, contract-shaped
setup-status artifact can receive automated credit.

The interactive owner is separate: Raspberry's `--hardware-test` performs
operator-controlled LED, input, and audio actuation, while
`--hardware-noise-test` performs the explicit no-touch input check. Do not
combine either interactive mode with a diagnostic mode. On Orange, those
Raspberry-only commands are rejected; use the profile diagnostic for passive
evidence instead. Image construction and identity validators cover image
contracts. The pristine diagnostic-image/initial bring-up utility is not a
production-FAT command. For production FAT, the profile-aware
`--fat-diagnostic` harness covers profile identity, service account/state,
required fixed device paths, route/readiness, and passive UDC state. Physical
pinmux, wiring, and other board-specific checks remain operator evidence, and a
harness `automated_pass` never becomes sound, visual, or physical qualification.
