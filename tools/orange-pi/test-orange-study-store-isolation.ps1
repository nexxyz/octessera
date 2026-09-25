$ErrorActionPreference = "Stop"
Import-Module (Join-Path $PSScriptRoot "orange-study-store-isolation.psm1") -Force
$runner = Join-Path $PSScriptRoot "run-orange-capability-study.ps1"
$tempParent = [IO.Path]::GetTempPath()
if (-not (Test-Path -LiteralPath $tempParent -PathType Container)) { throw "Sandbox parent is missing." }
$sandbox = Join-Path $tempParent ("octessera-awake-study-" + [guid]::NewGuid().ToString("N"))

function To-WslPath {
  param([string]$Path)
  $converted = & wsl --exec wslpath -a $Path
  if ($LASTEXITCODE -ne 0) { throw "WSL path conversion failed: $Path" }
  return $converted.Trim()
}

function Assert-PlanContains {
  param([string]$Plan, [string]$Needle)
  if ($Plan.IndexOf($Needle, [StringComparison]::Ordinal) -lt 0) { throw "AWAKE plan is missing: $Needle" }
}

function Invoke-StorePython {
  param([string]$Python, [string]$Mode, [string]$Source, [string]$Clone, [string]$Record)
  $result = & wsl --exec python3 $Python $Mode $Source $Clone $Record
  if ($LASTEXITCODE -ne 0) { throw "Sandbox $Mode failed." }
  return $result
}

New-Item -ItemType Directory -Path $sandbox | Out-Null
try {
  $source = Join-Path $sandbox "presets"
  $parent = Join-Path $sandbox "study-stores"
  $samples = Join-Path $sandbox "samples"
  New-Item -ItemType Directory -Path $source, $parent, $samples | Out-Null
  $manifest = Join-Path $samples "MANIFEST.tsv"
  [IO.File]::WriteAllText($manifest, "library/kick.wav`tsha256`n")
  $unit = "octessera-study-$([guid]::NewGuid().ToString('N')).service"
  $clone = Join-Path $parent $unit
  $record = Join-Path $sandbox "awake-store-evidence.json"
  $default = Join-Path $source "default.json"
  $recovery = Join-Path $source "recovery-save.json"
  $fixture = '{"runtimeConfig":{"dimTimerSeconds":60,"screenSleepSeconds":60,"midi":{"syncMode":"internal"},"bpm":120,"instruments":[{"sample":{"slots":[{"path":"library/kick.wav"}]}}]},"deviceConfig":{"usb":"gadget"}}'
  [IO.File]::WriteAllText($default, $fixture)
  [IO.File]::WriteAllText($recovery, '{"recovery":"unchanged"}')
  $original = [IO.File]::ReadAllBytes($default)
  $recoveryOriginal = [IO.File]::ReadAllBytes($recovery)
  $pythonFile = Join-Path $sandbox "store.py"
  [IO.File]::WriteAllText($pythonFile, (Get-OrangeStudyStorePython), (New-Object System.Text.UTF8Encoding($false)))
  $wslPython = To-WslPath $pythonFile
  $wslSource = To-WslPath $source
  $wslClone = To-WslPath $clone
  $wslRecord = To-WslPath $record
  $evidence = Invoke-StorePython $wslPython "prepare" $wslSource $wslClone $wslRecord
  [IO.File]::WriteAllText($record, "$evidence`n")
  $recorded = $evidence | ConvertFrom-Json
  if ($recorded.scenario -cne "AWAKE" -or $recorded.clone_path -cne $wslClone -or $recorded.original_timers.dimTimerSeconds -ne 60 -or $recorded.original_timers.screenSleepSeconds -ne 60) { throw "Original timer or clone-path evidence is wrong." }
  if ($recorded.original_sha256.'default.json' -cne (Get-FileHash -LiteralPath $default -Algorithm SHA256).Hash.ToLowerInvariant()) { throw "Original hash evidence is wrong." }
  if ([Convert]::ToBase64String($original) -cne [Convert]::ToBase64String([IO.File]::ReadAllBytes($default))) { throw "Original default changed." }
  if ([Convert]::ToBase64String($recoveryOriginal) -cne [Convert]::ToBase64String([IO.File]::ReadAllBytes($recovery))) { throw "Original recovery changed." }
  $expected = $fixture | ConvertFrom-Json
  $expected.runtimeConfig.dimTimerSeconds = 0
  $expected.runtimeConfig.screenSleepSeconds = 0
  $cloned = [IO.File]::ReadAllText((Join-Path $clone "default.json")) | ConvertFrom-Json
  if (($expected | ConvertTo-Json -Depth 20 -Compress) -cne ($cloned | ConvertTo-Json -Depth 20 -Compress)) { throw "AWAKE clone changed fields beyond the two timers." }
  if ([Convert]::ToBase64String($recoveryOriginal) -cne [Convert]::ToBase64String([IO.File]::ReadAllBytes((Join-Path $clone "recovery-save.json")))) { throw "Recovery copy is not byte-identical." }
  if ([IO.File]::ReadAllText($manifest) -cne "library/kick.wav`tsha256`n") { throw "Sample manifest changed." }
  Invoke-StorePython $wslPython "verify" $wslSource $wslClone $wslRecord | Out-Null
  [IO.File]::WriteAllText($default, "{}")
  $ErrorActionPreference = "Continue"
  $failure = & wsl --exec python3 $wslPython verify $wslSource $wslClone $wslRecord 2>&1
  $failed = $LASTEXITCODE -ne 0
  $ErrorActionPreference = "Stop"
  if (-not $failed -or -not (Test-Path -LiteralPath $clone -PathType Container)) { throw "Origin mismatch did not fail closed and preserve clone: $failure" }
  $rejected = Join-Path $parent "octessera-study-$([guid]::NewGuid().ToString('N')).service"
  $ErrorActionPreference = "Continue"
  & wsl --exec python3 $wslPython prepare $wslSource (To-WslPath $rejected) $wslRecord 2>&1 | Out-Null
  $failed = $LASTEXITCODE -ne 0
  $ErrorActionPreference = "Stop"
  if (-not $failed -or (Test-Path -LiteralPath $rejected)) { throw "Invalid default created an AWAKE clone." }
  [IO.File]::WriteAllBytes($default, $original)
  $recoverySaved = Join-Path $source "recovery-original.json"
  Move-Item -LiteralPath $recovery -Destination $recoverySaved
  & wsl --exec ln -s (To-WslPath $recoverySaved) (To-WslPath $recovery)
  if ($LASTEXITCODE -ne 0) { throw "Sandbox recovery symlink setup failed." }
  $rejected = Join-Path $parent "octessera-study-$([guid]::NewGuid().ToString('N')).service"
  $ErrorActionPreference = "Continue"
  & wsl --exec python3 $wslPython prepare $wslSource (To-WslPath $rejected) $wslRecord 2>&1 | Out-Null
  $failed = $LASTEXITCODE -ne 0
  $ErrorActionPreference = "Stop"
  if (-not $failed -or (Test-Path -LiteralPath $rejected)) { throw "Non-regular recovery source created an AWAKE clone." }
  Remove-Item -LiteralPath $recovery -Force
  Move-Item -LiteralPath $recoverySaved -Destination $recovery
  Remove-Item -LiteralPath $recovery -Force
  $optionalClone = Join-Path $parent "octessera-study-$([guid]::NewGuid().ToString('N')).service"
  $optionalEvidence = Invoke-StorePython $wslPython "prepare" $wslSource (To-WslPath $optionalClone) $wslRecord | ConvertFrom-Json
  if ($null -ne $optionalEvidence.original_sha256.'recovery-save.json' -or (Test-Path -LiteralPath (Join-Path $optionalClone "recovery-save.json"))) { throw "Missing recovery-save was not kept optional." }
  [IO.File]::WriteAllBytes($recovery, $recoveryOriginal)

  $plan = (& $runner -Target "octessera@orange.test.invalid" -Mode LiveCandidate -UiProfile -AutoPlay -KeepAwake -AllowServiceInterruption -Artifact (Join-Path $sandbox "missing") -PrintOnly) -join "`n"
  Assert-PlanContains $plan 'Display scenario: AWAKE'
  Assert-PlanContains $plan 'scenario=AWAKE'
  Assert-PlanContains $plan '"store_dir=$study_store"'
  Assert-PlanContains $plan '--setenv=OCTESSERA_PI_STORE_DIR="$study_store" --setenv=OCTESSERA_PI_TIMING_KEEP_AWAKE=1'
  Assert-PlanContains $plan 'OCTESSERA_PI_SAMPLES_DIR=/var/lib/octessera/samples'
  $studyStart = $plan.IndexOf("Study payload:`n", [StringComparison]::Ordinal) + "Study payload:`n".Length
  $study = $plan.Substring($studyStart, $plan.IndexOf("Study payload transport:", $studyStart, [StringComparison]::Ordinal) - $studyStart)
  $cleanupStart = $plan.IndexOf("Cleanup payload:`n", [StringComparison]::Ordinal) + "Cleanup payload:`n".Length
  $cleanupTail = $plan.Substring($cleanupStart)
  $cleanup = $cleanupTail.Substring(0, $cleanupTail.LastIndexOf("`n& '", [StringComparison]::Ordinal))
  Assert-PlanContains $study ('prepare_study_store' + "`n" + 'sudo -n systemctl stop "$service"')
  if ($study.IndexOf('if ! verify_study_store; then', [StringComparison]::Ordinal) -ge $study.IndexOf('if ! timeout --signal=TERM --kill-after=2 15s sudo -n systemctl start "$service"', [StringComparison]::Ordinal)) { throw "Origin hash verification did not precede service restoration." }
  if ($cleanup.IndexOf('verify_study_store || exit 72', [StringComparison]::Ordinal) -ge $cleanup.IndexOf('timeout --signal=TERM --kill-after=2 15s sudo -n systemctl start "$service"', [StringComparison]::Ordinal)) { throw "Host recovery bypasses origin verification." }
  if ($cleanup.Contains('cleanup_study_store || cleanup_status=1')) { throw "Host recovery can delete failed-study evidence." }
  $studyFile = Join-Path $sandbox "study.sh"
  $cleanupFile = Join-Path $sandbox "cleanup.sh"
  $encoding = New-Object System.Text.UTF8Encoding($false)
  [IO.File]::WriteAllText($studyFile, $study, $encoding)
  [IO.File]::WriteAllText($cleanupFile, $cleanup, $encoding)
  & wsl --exec bash -n (To-WslPath $studyFile)
  if ($LASTEXITCODE -ne 0) { throw "AWAKE study shell syntax failed." }
  & wsl --exec bash -n (To-WslPath $cleanupFile)
  if ($LASTEXITCODE -ne 0) { throw "AWAKE cleanup shell syntax failed." }
  $owned = $study.Substring($study.IndexOf("study_parent=/var/lib/octessera/study-stores", [StringComparison]::Ordinal))
  $owned = $owned.Substring(0, $owned.IndexOf("candidate_status=0", [StringComparison]::Ordinal))
  $ownedFile = Join-Path $sandbox "store-functions.sh"
  [IO.File]::WriteAllText($ownedFile, "unit=octessera-study-$('0' * 32).service`nroot=/tmp/awake-test`n$owned", $encoding)
  & wsl --exec shellcheck -s bash -S warning (To-WslPath $ownedFile)
  if ($LASTEXITCODE -ne 0) { throw "AWAKE store functions ShellCheck failed." }
  $guardStart = $study.IndexOf('  if [ "$exit_status" -eq 0 ] && [ "$restore_status" -eq 0 ]', [StringComparison]::Ordinal)
  $guardEnd = $study.IndexOf('  if [ "$restore_status" -ne 0 ]; then', $guardStart, [StringComparison]::Ordinal)
  if ($guardStart -lt 0 -or $guardEnd -lt 0) { throw "AWAKE clone cleanup guard is missing." }
  $guard = $study.Substring($guardStart, $guardEnd - $guardStart)
  foreach ($case in @(@(0, 1, 0), @(1, 0, 0), @(0, 0, 1), @(0, 0, 0))) {
    $marker = Join-Path $sandbox ("cleanup-" + [guid]::NewGuid().ToString("N"))
    $wslMarker = To-WslPath $marker
    $expected = if ($case[0] -eq 0 -and $case[1] -eq 0 -and $case[2] -eq 0) { "-e" } else { "! -e" }
    $mock = "set -eu`nunit=octessera-study-test.service`nexit_status=$($case[0])`nrestore_status=$($case[1])`ncleanup_status=$($case[2])`nsudo() { return 1; }`ncleanup_study_store() { touch '$wslMarker'; }`n$guard`n[ $expected '$wslMarker' ]`n"
    $mockFile = Join-Path $sandbox "mock-restore.sh"
    [IO.File]::WriteAllText($mockFile, $mock, $encoding)
    & wsl --exec bash (To-WslPath $mockFile)
    if ($LASTEXITCODE -ne 0) { throw "AWAKE cleanup ran before successful study and service restoration." }
  }
  $verifyStart = $study.IndexOf('  if ! verify_study_store; then', [StringComparison]::Ordinal)
  $verifyEnd = $study.IndexOf('  if ! timeout --signal=TERM --kill-after=2 15s sudo -n systemctl start "$service"', $verifyStart, [StringComparison]::Ordinal)
  if ($verifyStart -lt 0 -or $verifyEnd -lt 0) { throw "AWAKE origin failure guard is missing." }
  $restoreGuard = $study.Substring($verifyStart, $verifyEnd - $verifyStart)
  $startMarker = To-WslPath (Join-Path $sandbox "service-started")
  $startMock = "set -eu`nrestore_status=0`nverify_study_store() { return 1; }`nrestore_service() {`n$restoreGuard`n  touch '$startMarker'`n}`nrestore_service`n[ `"`$restore_status`" -eq 1 ] && [ ! -e '$startMarker' ]`n"
  $startMockFile = Join-Path $sandbox "mock-start.sh"
  [IO.File]::WriteAllText($startMockFile, $startMock, $encoding)
  & wsl --exec bash (To-WslPath $startMockFile)
  if ($LASTEXITCODE -ne 0) { throw "Origin hash failure did not block installed-service start." }
  Write-Output "Orange AWAKE sandbox clone, origin hashes, sample parity, PrintOnly and shell syntax tests passed"
} finally {
  Remove-Item -LiteralPath $sandbox -Recurse -Force -ErrorAction SilentlyContinue
}
