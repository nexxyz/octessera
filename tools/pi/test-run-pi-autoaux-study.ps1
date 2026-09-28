$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$payloadSource = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "pi-autoaux-study-payload.sh"))
$runner = Join-Path $PSScriptRoot "run-pi-autoaux-study.ps1"
$sandbox = Join-Path ([IO.Path]::GetTempPath()) ("octessera-pi-autoaux-test-" + [guid]::NewGuid().ToString("N"))
$tokens = $null
$parseErrors = $null
[System.Management.Automation.Language.Parser]::ParseFile($runner, [ref]$tokens, [ref]$parseErrors) | Out-Null
if ($parseErrors.Count -gt 0) { throw "Runner PowerShell syntax failed: $($parseErrors[0].Message)" }
New-Item -ItemType Directory -Path $sandbox | Out-Null

function To-WslPath {
  param([string]$Path)
  $result = & wsl --exec wslpath -a $Path
  if ($LASTEXITCODE -ne 0) { throw "WSL path conversion failed: $Path" }
  $result.Trim()
}

function Assert-Contains {
  param([string]$Text, [string]$Value)
  if (-not $Text.Contains($Value)) { throw "Expected text is missing: $Value" }
}

try {
  $payloadPath = Join-Path $sandbox "study.sh"
  $fixtureRoot = To-WslPath $sandbox
  $payload = $payloadSource.Replace("__RUN_ID__", "a" * 32).Replace("__LIVE_SECONDS__", "5").Replace("__ARTIFACT_HASH__", "b" * 64).Replace("__SOURCE_COMMIT__", "c" * 40).Replace("__REMOTE_ROOT__", "$fixtureRoot/remote")
  $payload = $payload.Replace("/usr/local/bin/octessera-pi", "$fixtureRoot/installed/octessera-pi").Replace('test "$configured_store" = /home/pi/presets', "test `"`$configured_store`" = $fixtureRoot/presets").Replace("/var/lib/octessera", "$fixtureRoot/octessera").Replace("/run/octessera", "$fixtureRoot/run")
  [IO.File]::WriteAllText($payloadPath, $payload, (New-Object System.Text.UTF8Encoding($false)))
  $linuxPayload = To-WslPath $payloadPath
  & wsl --exec bash -n $linuxPayload
  if ($LASTEXITCODE -ne 0) { throw "Remote payload shell syntax failed." }
  & wsl --exec shellcheck -s bash -S error $linuxPayload
  if ($LASTEXITCODE -ne 0) { throw "Remote payload ShellCheck failed." }

  $mockDirectory = Join-Path $sandbox "bin"
  $presetDirectory = Join-Path $sandbox "presets"
  $installedDirectory = Join-Path $sandbox "installed"
  $runDirectory = Join-Path $sandbox "run"
  $workingDirectory = Join-Path $sandbox "working"
  $procDirectory = Join-Path $sandbox "proc"
  $remoteDirectory = Join-Path $sandbox "remote"
  $octesseraDirectory = Join-Path $sandbox "octessera"
  $studyDirectory = Join-Path $octesseraDirectory "study-stores"
  New-Item -ItemType Directory -Path $mockDirectory, $presetDirectory, $installedDirectory, $runDirectory, $remoteDirectory, $workingDirectory, $octesseraDirectory, (Join-Path $procDirectory "1234"), (Join-Path $procDirectory "5678") | Out-Null
  if (Test-Path -LiteralPath $studyDirectory) { throw "Study parent fixture was unexpectedly pre-created: $studyDirectory" }
  $presetLinuxPath = To-WslPath $presetDirectory
  & wsl --exec test -d $presetLinuxPath
  if ($LASTEXITCODE -ne 0) { throw "The fake default store is not a Linux directory: $presetLinuxPath" }
  & wsl --exec test '!' -L $presetLinuxPath
  if ($LASTEXITCODE -ne 0) { throw "The fake default store resolves as a symlink: $presetLinuxPath" }
  $payload = $payload.Replace("/home/pi/octessera-dev", (To-WslPath $workingDirectory)).Replace("/proc", (To-WslPath $procDirectory))
  [IO.File]::WriteAllText($payloadPath, $payload, (New-Object System.Text.UTF8Encoding($false)))
  $linuxPayload = To-WslPath $payloadPath
  New-Item -ItemType Directory -Path (Join-Path $sandbox "remote\evidence") | Out-Null
  $originalDefaultJson = '{"runtimeConfig":{"dimTimerSeconds":60,"screenSleepSeconds":45,"autoSaveDefault":false,"midi":{"syncMode":"internal"}}}'
  $originalRecoveryJson = '{"untouched":true}'
  [IO.File]::WriteAllText((Join-Path $presetDirectory "default.json"), $originalDefaultJson)
  [IO.File]::WriteAllText((Join-Path $presetDirectory "recovery-save.json"), $originalRecoveryJson)
  [IO.File]::WriteAllText((Join-Path $installedDirectory "octessera-pi"), "original executable")
  $artifact = Join-Path $remoteDirectory "octessera-pi"
  [IO.File]::WriteAllText($artifact, "candidate executable")
  $artifactHash = (Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash.ToLowerInvariant()
  $payload = $payload.Replace("b" * 64, $artifactHash)
  $runnerTemplate = $payload
  [IO.File]::WriteAllText($payloadPath, $payload, (New-Object System.Text.UTF8Encoding($false)))
  $linuxPayload = To-WslPath $payloadPath
  $meta = [ordered]@{ schema_version = 1; board_profile = "raspberry-pi-zero-2w"; binary = "octessera-pi"; arch = "aarch64-unknown-linux-gnu"; cargo_feature = "hardware-raspberry-pi-zero-2w"; source_commit = "c" * 40; binary_sha256 = $artifactHash }
  [IO.File]::WriteAllText((Join-Path $remoteDirectory "octessera-pi.metadata.json"), ($meta | ConvertTo-Json -Compress))

  $sudo = @'
#!/bin/bash
set -e
[ "$1" = -n ] || exit 80
shift
printf '%s\n' "$*" >> "$MOCK_SUDO_LOG"
[ "${1:-}" = -v ] && exit 1
if [ "${1:-}" = true ] && [ "${MOCK_SUDO_TRUE_DENIED:-0}" = 1 ]; then exit 1; fi
if [ "${1:-}" = -u ]; then
  shift 2
  if [ "${1:-}" = test ] && [ "${2:-}" = -x ] && [ "${MOCK_EXECUTABLE:-1}" = 0 ]; then exit 1; fi
fi
if [ "${1:-}" = chown ]; then exit 0; fi
if [ "${1:-}" = install ]; then
  shift
  args=()
  while [ "$#" -gt 0 ]; do case "$1" in -o|-g) shift 2 ;; *) args+=("$1"); shift ;; esac; done
  exec install "${args[@]}"
fi
exec "$@"
'@
  $systemctl = @'
#!/bin/bash
set -eu
state="$MOCK_STATE"
service=octessera.service
case "$1" in
 is-active) unit="${2:-}"; if [ "$unit" = --quiet ]; then unit="$3"; quiet=1; fi; if [ "$unit" = "$service" ]; then value="$(cat "$state/service")"; else value="$(cat "$state/candidate")"; fi; [ "$value" = active ] && { [ "${quiet:-0}" = 0 ] && echo active; exit 0; }; [ "${quiet:-0}" = 0 ] && echo inactive; exit 3 ;;
 is-enabled) echo enabled ;;
 show) unit="$2"; property="${3#--property=}"; case "$property" in User) echo pi ;; ExecStart) echo "{ path=$MOCK_BINARY ; argv[]=$MOCK_BINARY ; }" ;; WorkingDirectory) echo "$MOCK_WORKING" ;; Environment) store="${MOCK_STORE_ENV:-/home/pi/presets}"; if [ "${MOCK_TRACE:-1}" = 1 ]; then echo "OCTESSERA_PI_SAMPLES_DIR=/home/pi/samples OCTESSERA_PI_STORE_DIR=$store OCTESSERA_WAKE_TRACE=1 OCTESSERA_EXPECTED_BOARD_PROFILE=raspberry-pi-zero-2w"; else echo "OCTESSERA_PI_SAMPLES_DIR=/home/pi/samples OCTESSERA_PI_STORE_DIR=$store OCTESSERA_EXPECTED_BOARD_PROFILE=raspberry-pi-zero-2w"; fi ;; MainPID) if [[ "$unit" = octessera-autoaux-* ]]; then if [ "$(cat "$state/candidate")" = active ]; then echo 2345; else echo 0; fi; elif [ "$(cat "$state/service")" = active ]; then cat "$state/service-pid"; else echo 0; fi ;; InvocationID) if [[ "$unit" = octessera-autoaux-* ]]; then echo candidate-invocation; elif [ "$(cat "$state/service")" = active ]; then cat "$state/service-invocation"; else echo; fi ;; LoadState) if [ -e "$state/candidate-created" ]; then echo loaded; else echo not-found; fi ;; esac ;;
   stop) if [ "$2" = "$service" ]; then echo inactive > "$state/service"; else if [ ! -e "$state/candidate-created" ] || [ "${MOCK_CANDIDATE_STOP_FAIL:-0}" = 1 ]; then exit 1; fi; if [ "${MOCK_CANDIDATE_STOP_FAIL_ONCE:-0}" = 1 ] && [ ! -e "$state/candidate-stop-failed" ]; then touch "$state/candidate-stop-failed"; if [ "${MOCK_COLLECT_AFTER_STOP_FAILURE:-0}" = 1 ]; then echo inactive > "$state/candidate"; rm -f "$state/candidate-created"; fi; exit 1; fi; echo inactive > "$state/candidate"; if [ "${MOCK_COLLECT_AFTER_STOP:-0}" = 1 ]; then rm -f "$state/candidate-created"; fi; fi ;;
  start) if [ "${MOCK_RESTORE_FAIL:-0}" = 1 ]; then exit 1; fi; echo active > "$state/service"; echo 5678 > "$state/service-pid"; echo restored-new-invocation > "$state/service-invocation"; printf '%s\n' start >> "$state/service-starts"; mkdir -p "$MOCK_RUN"; if [ "${MOCK_RESTORED_MARKER:-valid}" = valid ]; then printf '{"schema_version":1,"pid":5678,"systemd_invocation_id":"restored-new-invocation","kind":"octessera_candidate_readiness","status":"ready","board_profile":"raspberry-pi-zero-2w","package_version":"0.8.2","ready_at_unix_ms":1700000000000}\n' > "$MOCK_RUN/candidate-ready.json"; else printf '{"schema_version":1,"pid":1234,"systemd_invocation_id":"restored-invocation","kind":"octessera_candidate_readiness","status":"ready","board_profile":"orange-pi-zero-2w","package_version":"0.8.2","ready_at_unix_ms":1700000000000}\n' > "$MOCK_RUN/candidate-ready.json"; fi; if [ "${MOCK_START_MUTATE_DEFAULT:-0}" = 1 ]; then printf '{"runtimeConfig":{"startup_mutation":true}}' > "$MOCK_DEFAULT"; fi; if [ "${MOCK_START_CREATE_RECOVERY:-0}" = 1 ]; then printf '{"startup":"created"}' > "$MOCK_RECOVERY"; fi ;;
 *) exit 81 ;;
esac
'@
  $systemdRun = @'
#!/bin/bash
set -eu
if [ "${MOCK_LAUNCH_FAIL:-0}" = 1 ]; then exit 1; fi
touch "$MOCK_STATE/candidate-created"
echo active > "$MOCK_STATE/candidate"
if [ "${MOCK_STORE_DRIFT:-0}" = 1 ]; then printf '{"runtimeConfig":{"changed":true}}' > "$MOCK_DEFAULT"; fi
printf '%s\n' "$@" > "$MOCK_STATE/systemd-run-args"
cp "$MOCK_STORE/default.json" "$MOCK_STATE/clone.json"
case "${MOCK_CANDIDATE_MARKER:-valid}" in
 valid) printf '{"schema_version":1,"pid":2345,"systemd_invocation_id":"candidate-invocation","kind":"octessera_candidate_readiness","status":"ready","board_profile":"raspberry-pi-zero-2w","package_version":"0.8.2","ready_at_unix_ms":1700000000000}\n' > "$MOCK_READY" ;;
 wrong-board) printf '{"schema_version":1,"pid":2345,"systemd_invocation_id":"candidate-invocation","kind":"octessera_candidate_readiness","status":"ready","board_profile":"orange-pi-zero-2w","package_version":"0.8.2","ready_at_unix_ms":1700000000000}\n' > "$MOCK_READY" ;;
 stale-pid) printf '{"schema_version":1,"pid":9999,"systemd_invocation_id":"candidate-invocation","kind":"octessera_candidate_readiness","status":"ready","board_profile":"raspberry-pi-zero-2w","package_version":"0.8.2","ready_at_unix_ms":1700000000000}\n' > "$MOCK_READY" ;;
 stale-invocation) printf '{"schema_version":1,"pid":2345,"systemd_invocation_id":"stale-invocation","kind":"octessera_candidate_readiness","status":"ready","board_profile":"raspberry-pi-zero-2w","package_version":"0.8.2","ready_at_unix_ms":1700000000000}\n' > "$MOCK_READY" ;;
 malformed) printf '{broken' > "$MOCK_READY" ;;
esac
if [ "${MOCK_RECEIPT:-1}" = 1 ]; then echo 'raspberry-autoaux cutoff_start=1 oled_cutoff_a=1 oled_frame_a=2 oled_cutoff_b=3 oled_frame_b=4 synth_cutoff_commands=1 synth_cutoff_a=1 synth_cutoff_b=0 save_revision=22 save_request=91 save_elapsed_ms=8 aux_turns=2 rapid_turns=1 missed_turns=0' > "$MOCK_STATE/candidate-journal"; else : > "$MOCK_STATE/candidate-journal"; fi
if [ "${MOCK_FAILED:-0}" = 1 ]; then echo 'raspberry-autoaux-failed: child failure' >> "$MOCK_STATE/candidate-journal"; fi
if [ "${MOCK_EXIT2:-0}" = 1 ]; then echo inactive > "$MOCK_STATE/candidate"; fi
'@
  $journal = @'
#!/bin/bash
set -eu
if [ "${2:-}" = -k ] || [ "$1" = -k ]; then echo 'kernel audio journal inspected'; elif [[ " $* " = *'octessera-autoaux-'* ]]; then cat "$MOCK_STATE/candidate-journal"; else echo 'service journal inspected'; fi
'@
  $readlink = @'
#!/bin/bash
if [ "${MOCK_RESTORE_EXE_FAIL:-0}" = 1 ] && [[ " $* " = *'/5678/exe'* ]]; then echo /wrong-binary; exit 0; fi
printf '%s\n' "$MOCK_BINARY"
'@
  foreach ($entry in @(@("sudo", $sudo), @("systemctl", $systemctl), @("systemd-run", $systemdRun), @("journalctl", $journal), @("readlink", $readlink))) {
    $path = Join-Path $mockDirectory $entry[0]
    [IO.File]::WriteAllText($path, $entry[1], (New-Object System.Text.UTF8Encoding($false)))
    & wsl --exec chmod +x (To-WslPath $path)
    if ($LASTEXITCODE -ne 0) { throw "Could not make mock executable: $($entry[0])" }
  }

  function Invoke-FakeStudy {
    param([string]$Name, [hashtable]$Environment = @{}, [string]$ExpectedHash = $artifactHash)
    $hostLog = Join-Path $remoteDirectory "evidence\host.log"
    if (Test-Path -LiteralPath $hostLog) { Remove-Item -LiteralPath $hostLog -Force }
    $runId = [guid]::NewGuid().ToString("N")
    $casePayload = $runnerTemplate.Replace("a" * 32, $runId).Replace($artifactHash, $ExpectedHash)
    if ($Name -in @("restore-failure", "restored-marker-failure")) {
      $restoreDeadline = @'
    deadline=$(( $(date +%s) + 30 ))
    while [ "$(date +%s)" -lt "$deadline" ]; do
'@
      $casePayload = $casePayload.Replace($restoreDeadline, $restoreDeadline.Replace('+ 30', '+ 0'))
    }
    if ($Environment.ContainsKey("MOCK_CANDIDATE_MARKER")) {
      $candidateDeadline = @'
deadline=$(( $(date +%s) + 30 ))
candidate_pid=
'@
      $casePayload = $casePayload.Replace($candidateDeadline, $candidateDeadline.Replace('+ 30', '+ 0'))
    }
    $casePayloadPath = Join-Path $sandbox "$Name.sh"
    [IO.File]::WriteAllText($casePayloadPath, $casePayload, (New-Object System.Text.UTF8Encoding($false)))
    $state = Join-Path $sandbox "state-$Name"
    New-Item -ItemType Directory -Path $state | Out-Null
    [IO.File]::WriteAllText((Join-Path $state "service"), "active")
    [IO.File]::WriteAllText((Join-Path $state "candidate"), "inactive")
    [IO.File]::WriteAllText((Join-Path $state "service-pid"), "1234")
    [IO.File]::WriteAllText((Join-Path $state "service-invocation"), "initial-invocation")
    [IO.File]::WriteAllText((Join-Path $runDirectory "candidate-ready.json"), '{"schema_version":1,"pid":1234,"systemd_invocation_id":"initial-invocation","kind":"octessera_candidate_readiness","status":"ready","board_profile":"raspberry-pi-zero-2w","package_version":"0.8.2","ready_at_unix_ms":1700000000000}')
    $initialTrace = if ($Environment.ContainsKey("MOCK_TRACE") -and $Environment.MOCK_TRACE -eq "0") { "OTHER=1" } else { "OCTESSERA_WAKE_TRACE=1" }
    $restoreTrace = if ($Environment.ContainsKey("MOCK_RESTORE_TRACE") -and $Environment.MOCK_RESTORE_TRACE -eq "0") { "OTHER=1" } else { "OCTESSERA_WAKE_TRACE=1" }
    $effectiveStore = if ($Environment.ContainsKey("MOCK_STORE_ENV")) { $Environment.MOCK_STORE_ENV } else { To-WslPath $presetDirectory }
    $initialEnvironmentBytes = [Text.Encoding]::UTF8.GetBytes("OCTESSERA_PI_STORE_DIR=$effectiveStore`0OCTESSERA_PI_SAMPLES_DIR=/home/pi/samples`0$initialTrace`0")
    $restoredEnvironmentBytes = [Text.Encoding]::UTF8.GetBytes("OCTESSERA_PI_STORE_DIR=$effectiveStore`0OCTESSERA_PI_SAMPLES_DIR=/home/pi/samples`0$restoreTrace`0")
    [IO.File]::WriteAllBytes((Join-Path $procDirectory "1234\environ"), $initialEnvironmentBytes)
    [IO.File]::WriteAllBytes((Join-Path $procDirectory "5678\environ"), $restoredEnvironmentBytes)
    $stateLinux = To-WslPath $state
    $envArgs = @("MOCK_STATE=$stateLinux", "MOCK_RUN=$(To-WslPath $runDirectory)", "MOCK_READY=$(To-WslPath $runDirectory)/autoaux-ready-$runId.json", "MOCK_STORE=$(To-WslPath $studyDirectory)/octessera-study-$runId.service", "MOCK_STORE_ENV=$(To-WslPath $presetDirectory)", "MOCK_DEFAULT=$(To-WslPath (Join-Path $presetDirectory 'default.json'))", "MOCK_RECOVERY=$(To-WslPath (Join-Path $presetDirectory 'recovery-save.json'))", "MOCK_SUDO_LOG=$(To-WslPath (Join-Path $state 'sudo.log'))", "MOCK_BINARY=$(To-WslPath (Join-Path $installedDirectory 'octessera-pi'))", "MOCK_WORKING=$(To-WslPath $workingDirectory)", "PATH=$(To-WslPath $mockDirectory):/usr/bin:/bin")
    foreach ($key in $Environment.Keys) { $envArgs += "$key=$($Environment[$key])" }
    $commandArgs = @("--exec", "env") + $envArgs + @("bash", (To-WslPath $casePayloadPath))
    $output = & wsl @commandArgs 2>&1
    return [pscustomobject]@{ ExitCode = $LASTEXITCODE; Output = ($output -join "`n"); State = $state; RunId = $runId }
  }

  $missingReceipt = Invoke-FakeStudy "missing" @{ MOCK_RECEIPT = "0" }
  if ($missingReceipt.ExitCode -eq 0) { throw "Missing receipt was accepted." }
  if (-not (Test-Path -LiteralPath (Join-Path $studyDirectory "octessera-study-$($missingReceipt.RunId).service"))) { throw "Failed study deleted its clone before successful restoration: $($missingReceipt.Output)" }
  $successful = Invoke-FakeStudy "success"
  if ($successful.ExitCode -ne 0) { throw "Successful receipt study failed: $($successful.Output)" }
  Assert-Contains $successful.Output "service_active=active"
  Assert-Contains $successful.Output "wake_trace=OCTESSERA_WAKE_TRACE=1"
  $candidateArgs = Get-Content -LiteralPath (Join-Path $successful.State "systemd-run-args") -Raw
  foreach ($contract in @("OCTESSERA_PI_TIMING_AUTOAUX=1", "OCTESSERA_PI_UI_PROFILE=1", "OCTESSERA_PI_TIMING_KEEP_AWAKE=1", "OCTESSERA_PI_SAMPLES_DIR=/home/pi/samples", "OCTESSERA_EXPECTED_BOARD_PROFILE=raspberry-pi-zero-2w")) { Assert-Contains $candidateArgs $contract }
  if (Test-Path -LiteralPath (Join-Path $studyDirectory "octessera-study-$($successful.RunId).service")) { throw "Successful restored study retained its isolated clone." }
  $sudoLog = Get-Content -LiteralPath (Join-Path $successful.State "sudo.log") -Raw
  Assert-Contains $sudoLog "install -d -o root -g root -m 0755"
  $editedClone = Get-Content -LiteralPath (Join-Path $successful.State "clone.json") -Raw | ConvertFrom-Json
  if ($editedClone.runtimeConfig.dimTimerSeconds -ne 0 -or $editedClone.runtimeConfig.screenSleepSeconds -ne 0 -or -not $editedClone.runtimeConfig.autoSaveDefault -or $editedClone.runtimeConfig.midi.syncMode -cne "internal") { throw "Study clone did not receive only the expected settings." }
  if ((Get-Content -LiteralPath (Join-Path $presetDirectory "default.json") -Raw) -cne $originalDefaultJson) { throw "Original store was modified." }
  $exit2 = Invoke-FakeStudy "exit2" @{ MOCK_EXIT2 = "1" }
  if ($exit2.ExitCode -eq 0) { throw "Early candidate exit was accepted." }
  $hashMismatch = Invoke-FakeStudy "hash-mismatch" @{} ("d" * 64)
  if ($hashMismatch.ExitCode -eq 0 -or [IO.File]::ReadAllText((Join-Path $hashMismatch.State "service")) -ne "active") { throw "Candidate hash mismatch did not fail before service interruption." }
  $traceMissing = Invoke-FakeStudy "trace-missing" @{ MOCK_TRACE = "0" }
  if ($traceMissing.ExitCode -eq 0 -or [IO.File]::ReadAllText((Join-Path $traceMissing.State "service")) -ne "active") { throw "Missing wake-trace preflight did not fail closed." }
  $wrongStore = Invoke-FakeStudy "wrong-store" @{ MOCK_STORE_ENV = "/home/pi/other-presets" }
  if ($wrongStore.ExitCode -eq 0 -or [IO.File]::ReadAllText((Join-Path $wrongStore.State "service")) -ne "active") { throw "Wrong installed store path did not fail before interruption." }
  $permissionFailure = Invoke-FakeStudy "permission-failure" @{ MOCK_EXECUTABLE = "0" }
  if ($permissionFailure.ExitCode -eq 0 -or [IO.File]::ReadAllText((Join-Path $permissionFailure.State "service")) -ne "active" -or (Test-Path -LiteralPath (Join-Path $permissionFailure.State "service-starts"))) { throw "Runtime-user execute failure did not fail before service interruption." }
  $sudoDenied = Invoke-FakeStudy "sudo-command-denied" @{ MOCK_SUDO_TRUE_DENIED = "1" }
  $sudoDeniedLog = [IO.File]::ReadAllText((Join-Path $sudoDenied.State "sudo.log"))
  if ($sudoDenied.ExitCode -eq 0 -or [IO.File]::ReadAllText((Join-Path $sudoDenied.State "service")).Trim() -ne "active" -or $sudoDeniedLog.Contains("systemctl stop octessera.service") -or (Test-Path -LiteralPath (Join-Path $sudoDenied.State "service-starts"))) { throw "Denied non-interactive sudo command reached service interruption." }
  $recoveryPath = Join-Path $presetDirectory "recovery-save.json"
  $recoveryBytes = [IO.File]::ReadAllBytes($recoveryPath)
  Remove-Item -LiteralPath $recoveryPath -Force
  $launchFailure = Invoke-FakeStudy "launch-failure" @{ MOCK_LAUNCH_FAIL = "1" }
  $launchServiceState = [IO.File]::ReadAllText((Join-Path $launchFailure.State "service")).Trim()
  if ($launchFailure.ExitCode -eq 0 -or $launchServiceState -cne "active" -or -not (Test-Path -LiteralPath (Join-Path $launchFailure.State "service-starts")) -or (Test-Path -LiteralPath $recoveryPath)) { throw "Never-created candidate unit blocked safe restoration or changed recovery-file absence: $($launchFailure.Output)" }
  [IO.File]::WriteAllBytes($recoveryPath, $recoveryBytes)
  foreach ($marker in @("wrong-board", "stale-pid", "stale-invocation", "malformed")) {
    $invalidReadiness = Invoke-FakeStudy "candidate-marker-$marker" @{ MOCK_CANDIDATE_MARKER = $marker }
    if ($invalidReadiness.ExitCode -eq 0 -or [IO.File]::ReadAllText((Join-Path $invalidReadiness.State "service")).Trim() -ne "active") { throw "Invalid candidate readiness marker was accepted: $marker ($($invalidReadiness.Output))" }
  }
  $stopFailure = Invoke-FakeStudy "candidate-stop-failure" @{ MOCK_CANDIDATE_STOP_FAIL_ONCE = "1"; MOCK_COLLECT_AFTER_STOP_FAILURE = "1" }
  if ($stopFailure.ExitCode -eq 0 -or [IO.File]::ReadAllText((Join-Path $stopFailure.State "service")).Trim() -ne "inactive" -or [IO.File]::ReadAllText((Join-Path $stopFailure.State "candidate")).Trim() -ne "inactive" -or (Test-Path -LiteralPath (Join-Path $stopFailure.State "service-starts"))) { throw "Installed service started after a candidate stop failed, even though cleanup later stopped it: $($stopFailure.Output)" }
  $collectedUnit = Invoke-FakeStudy "collected-unit" @{ MOCK_COLLECT_AFTER_STOP = "1" }
  if ($collectedUnit.ExitCode -ne 0 -or [IO.File]::ReadAllText((Join-Path $collectedUnit.State "service")).Trim() -ne "active" -or (Test-Path -LiteralPath (Join-Path $collectedUnit.State "candidate-created")) -or (Test-Path -LiteralPath (Join-Path $studyDirectory "octessera-study-$($collectedUnit.RunId).service"))) { throw "Collected candidate unit blocked installed-service restoration or clone cleanup: $($collectedUnit.Output)" }
  $restoredMarkerFailure = Invoke-FakeStudy "restored-marker-failure" @{ MOCK_RESTORED_MARKER = "wrong" }
  if ($restoredMarkerFailure.ExitCode -eq 0 -or -not (Test-Path -LiteralPath (Join-Path $studyDirectory "octessera-study-$($restoredMarkerFailure.RunId).service"))) { throw "Invalid restoration marker passed or deleted the clone." }
  $restoredProcessFailure = Invoke-FakeStudy "restored-process-failure" @{ MOCK_RESTORE_EXE_FAIL = "1"; MOCK_RESTORE_TRACE = "0" }
  if ($restoredProcessFailure.ExitCode -eq 0 -or -not (Test-Path -LiteralPath (Join-Path $studyDirectory "octessera-study-$($restoredProcessFailure.RunId).service"))) { throw "Restored process executable/environment mismatch passed or deleted the clone." }
  $restoreDefaultMutation = Invoke-FakeStudy "restore-default-mutation" @{ MOCK_START_MUTATE_DEFAULT = "1" }
  $restoreDefaultLog = Get-Content -LiteralPath (Join-Path $remoteDirectory "evidence\host.log") -Raw
  if ($restoreDefaultMutation.ExitCode -eq 0 -or [IO.File]::ReadAllText((Join-Path $restoreDefaultMutation.State "service")).Trim() -ne "active" -or -not (Test-Path -LiteralPath (Join-Path $studyDirectory "octessera-study-$($restoreDefaultMutation.RunId).service")) -or -not $restoreDefaultLog.Contains("restore_status=1")) { throw "Post-start default mutation was not reported with clone/evidence retained: $($restoreDefaultMutation.Output)" }
  if ((Get-Content -LiteralPath (Join-Path $presetDirectory "default.json") -Raw) -cne '{"runtimeConfig":{"startup_mutation":true}}') { throw "Post-start default mutation was overwritten." }
  [IO.File]::WriteAllText((Join-Path $presetDirectory "default.json"), $originalDefaultJson)
  [IO.File]::WriteAllText((Join-Path $presetDirectory "recovery-save.json"), $originalRecoveryJson)
  Remove-Item -LiteralPath (Join-Path $presetDirectory "recovery-save.json") -Force
  $restoreRecoveryCreation = Invoke-FakeStudy "restore-recovery-creation" @{ MOCK_START_CREATE_RECOVERY = "1" }
  $restoreRecoveryLog = Get-Content -LiteralPath (Join-Path $remoteDirectory "evidence\host.log") -Raw
  if ($restoreRecoveryCreation.ExitCode -eq 0 -or [IO.File]::ReadAllText((Join-Path $restoreRecoveryCreation.State "service")).Trim() -ne "active" -or -not (Test-Path -LiteralPath (Join-Path $studyDirectory "octessera-study-$($restoreRecoveryCreation.RunId).service")) -or -not $restoreRecoveryLog.Contains("restore_status=1") -or -not (Test-Path -LiteralPath (Join-Path $presetDirectory "recovery-save.json"))) { throw "Post-start recovery creation was not reported with clone/evidence retained: $($restoreRecoveryCreation.Output)" }
  Remove-Item -LiteralPath (Join-Path $presetDirectory "recovery-save.json") -Force
  [IO.File]::WriteAllText((Join-Path $presetDirectory "recovery-save.json"), $originalRecoveryJson)
  [IO.File]::WriteAllText((Join-Path $presetDirectory "default.json"), $originalDefaultJson)
  $storeDrift = Invoke-FakeStudy "store-drift" @{ MOCK_STORE_DRIFT = "1" }
  if ($storeDrift.ExitCode -eq 0 -or [IO.File]::ReadAllText((Join-Path $storeDrift.State "service")).Trim() -ne "inactive" -or (Test-Path -LiteralPath (Join-Path $storeDrift.State "service-starts"))) { throw "Original-store hash drift was not fail-closed before service restart: $($storeDrift.Output)" }
  if ((Get-Content -LiteralPath (Join-Path $presetDirectory "default.json") -Raw) -cne '{"runtimeConfig":{"changed":true}}') { throw "Study attempted to overwrite original-store drift." }
  [IO.File]::WriteAllText((Join-Path $presetDirectory "default.json"), $originalDefaultJson)
  $restoreFailure = Invoke-FakeStudy "restore-failure" @{ MOCK_RESTORE_FAIL = "1" }
  $restoreFailureLog = Get-Content -LiteralPath (Join-Path $restoreFailure.State "sudo.log") -Raw
  if ($restoreFailure.ExitCode -eq 0 -or -not (Test-Path -LiteralPath (Join-Path $studyDirectory "octessera-study-$($restoreFailure.RunId).service")) -or -not $restoreFailureLog.Contains("systemctl start octessera.service")) { throw "Restore-failure case did not attempt service start or deleted the isolated clone." }
  $target = "pi@127.0.0.1"
  $runnerOutput = (& $runner -Target $target -Artifact (Join-Path $sandbox "missing") -PrintOnly) -join "`n"
  Assert-Contains $runnerOutput "Normal-runtime AutoAux candidate"
  Assert-Contains $runnerOutput "timing-probe mode is not used"
  if ($runnerOutput.Contains("--timing-probe")) { throw "Wrapper selected timing-probe mode." }
  . (Join-Path $PSScriptRoot "board-profile.ps1")
  $checkpoint = (& git -C (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path rev-parse HEAD).Trim()
  $localArtifact = Join-Path $sandbox "candidate"
  $localMetadata = "$localArtifact.metadata.json"
  [IO.File]::WriteAllText($localArtifact, "metadata candidate")
  $validMetadata = Get-RaspberryBoardMetadataJson -SourceCommit $checkpoint -BinaryPath $localArtifact
  [IO.File]::WriteAllText($localMetadata, $validMetadata)
  & $runner -Target $target -Artifact $localArtifact -Metadata $localMetadata -PrintOnly | Out-Null
  [IO.File]::WriteAllText($localMetadata, $validMetadata.Replace((Get-FileHash $localArtifact -Algorithm SHA256).Hash.ToLowerInvariant(), ("0" * 64)))
  $metadataRejected = $false
  try { & $runner -Target $target -Artifact $localArtifact -Metadata $localMetadata -PrintOnly | Out-Null } catch { $metadataRejected = $true }
  if (-not $metadataRejected) { throw "Mismatched artifact metadata was accepted." }
  Write-Output "Raspberry AutoAux fake preflight, clone isolation, receipt, early-exit, restoration, hash, trace and shell checks passed"
} finally {
  Remove-Item -LiteralPath $sandbox -Recurse -Force -ErrorAction SilentlyContinue
}
