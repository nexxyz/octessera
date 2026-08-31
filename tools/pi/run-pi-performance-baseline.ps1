[CmdletBinding()]
param(
  [string]$ManifestPath = "",
  [ValidateSet("Passive", "Offline", "Live", "Full")]
  [string]$Phase = "Full",
  [string]$Target = "pi@192.168.0.218",
  [string]$Key = "$env:USERPROFILE\.ssh\octessera_pi_dev",
  [string]$Binary = "/usr/local/bin/octessera-pi",
  [string]$Artifact = "",
  [string]$Metadata = "",
  [string]$RunnerPath = "",
  [string]$OutputDirectory = "",
  [switch]$AllowServiceInterruption,
  [switch]$CanaryOnly,
  [switch]$PrintOnly
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$manifestModule = Join-Path $PSScriptRoot "..\performance\performance-baseline-plan.psm1"
$resultsModule = Join-Path $PSScriptRoot "..\performance\performance-baseline-results.psm1"
$boardProfile = Join-Path $PSScriptRoot "board-profile.ps1"
Import-Module $manifestModule -Force
Import-Module $resultsModule -Force
. $boardProfile
$transport = Join-Path $PSScriptRoot "with-pi-ssh.ps1"
$defaultManifestPath = Join-Path $PSScriptRoot "..\performance\cross-board-baseline.json"
$defaultRunnerPath = Join-Path $PSScriptRoot "run-pi-timing-probes.ps1"
if ([string]::IsNullOrWhiteSpace($ManifestPath)) { $ManifestPath = $defaultManifestPath }
if ([string]::IsNullOrWhiteSpace($RunnerPath)) { $RunnerPath = $defaultRunnerPath }
$pathValues = @($ManifestPath, $RunnerPath, $Binary, $Key, $Artifact, $Metadata) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
foreach ($pathValue in $pathValues) { Assert-PerformanceBaselinePath $pathValue "Raspberry baseline path" }
$manifest = Read-PerformanceBaselineManifest $ManifestPath
$needsActiveRun = $CanaryOnly -or $Phase -in @("Offline", "Live", "Full")
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path

function Get-CommitIdentity {
  $commit = (& git -C $repoRoot rev-parse --short=12 HEAD 2>$null | Out-String).Trim()
  if ([string]::IsNullOrWhiteSpace($commit)) { return "unknown" }
  return $commit
}

function Get-RepositoryIdentity {
  $head = (& git -C $repoRoot rev-parse HEAD 2>$null | Out-String).Trim().ToLowerInvariant()
  $status = (& git -C $repoRoot status --porcelain --untracked-files=all 2>$null | Out-String).Trim()
  if ([string]::IsNullOrWhiteSpace($head)) { throw "Repository HEAD could not be resolved." }
  return [pscustomobject]@{ Head = $head; Status = $status }
}

function Get-DefaultOutputDirectory {
  $date = (Get-Date).ToUniversalTime().ToString("yyyy-MM-dd")
  return Join-Path $repoRoot (Join-Path "target\performance-baselines\raspberry-pi-zero-2w" "$date-$(Get-CommitIdentity)")
}

function Quote-ShValue {
  param([Parameter(Mandatory)][string]$Value)
  return "'" + $Value.Replace("'", "'\''") + "'"
}

function Invoke-FreshPowerShell {
  param([Parameter(Mandatory)][string[]]$Arguments, [Parameter(Mandatory)][string]$StdoutPath, [Parameter(Mandatory)][string]$StderrPath)
  $processArguments = @($Arguments | ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } })
  $process = Start-Process -FilePath (Join-Path $PSHOME "powershell.exe") -ArgumentList $processArguments -RedirectStandardOutput $StdoutPath -RedirectStandardError $StderrPath -Wait -PassThru
  return [pscustomobject]@{ ExitCode = $process.ExitCode }
}

function Get-IdentityCommand {
  param([Parameter(Mandatory)][string]$RemoteBinary)
  return @"
set -eu
printf 'binary_sha256='; sha256sum -- $(Quote-ShValue $RemoteBinary) | awk 'NR == 1 { print $1 }'
printf 'board_profile='; $(Quote-ShValue $RemoteBinary) --print-build-metadata
printf 'hostname='; hostname
printf 'kernel='; uname -a
printf 'governor='; for path in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do [ -r "`$path" ] && printf '%s:%s ' "`$path" "`$(cat "`$path")"; done; printf '\n'
printf 'frequency='; for path in /sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq; do [ -r "`$path" ] && printf '%s:%s ' "`$path" "`$(cat "`$path")"; done; printf '\n'
printf 'thermal='; for path in /sys/class/thermal/thermal_zone*/temp; do [ -r "`$path" ] && printf '%s:%s ' "`$path" "`$(cat "`$path")"; done; printf '\n'
printf 'load='; cut -d' ' -f1-3 /proc/loadavg
printf 'memory='; awk '/^MemAvailable:/ { print `$2; exit }' /proc/meminfo
printf 'service_active='; systemctl is-active octessera.service 2>/dev/null || true
printf 'service_enabled='; systemctl is-enabled octessera.service 2>/dev/null || true
"@
}

function Invoke-PiIdentity {
  param([Parameter(Mandatory)][string]$Directory)
  $stdout = Join-Path $Directory "passive.stdout.txt"
  $stderr = Join-Path $Directory "passive.stderr.txt"
  & $transport "ssh" -Target $Target -Key $Key $Target (Get-IdentityCommand $Binary) 1> $stdout 2> $stderr
  $exitCode = if ($null -eq $LASTEXITCODE) { 0 } else { [int]$LASTEXITCODE }
  if ($exitCode -ne 0) { throw "Raspberry passive identity collection failed: $exitCode" }
  $identityText = Get-Content -LiteralPath $stdout -Raw
  if ($identityText -notmatch '"board_profile":"raspberry-pi-zero-2w"' -or $identityText -notmatch "service_active=active" -or $identityText -notmatch "service_enabled=enabled") { throw "Raspberry passive identity did not prove the fixed board and managed service state." }
  $binaryHash = [regex]::Match($identityText, '(?m)^binary_sha256=(?<hash>[0-9a-f]{64})$')
  if (-not $binaryHash.Success) { throw "Raspberry passive identity did not prove the exact binary hash." }
  return [pscustomobject]@{ StdoutPath = $stdout; StderrPath = $stderr; BinarySha256 = $binaryHash.Groups["hash"].Value }
}

function Read-IdentityValues {
  param([Parameter(Mandatory)][string]$Path)
  $values = [ordered]@{}
  foreach ($line in Get-Content -LiteralPath $Path) {
    $parts = $line -split "=", 2
    if ($parts.Count -eq 2) { $values[$parts[0]] = $parts[1] }
  }
  return $values
}

function Get-ProfileRow {
  param([Parameter(Mandatory)][string]$StdoutPath, [Parameter(Mandatory)][pscustomobject]$Cell)
  $lines = @(Get-Content -LiteralPath $StdoutPath | Where-Object { $_ -match '^(kind,scenario,metric,|engine_source,|system,)' })
  $rows = @($lines | ConvertFrom-Csv | Where-Object { $_.kind -eq "engine_source" -and $_.scenario -eq $Cell.scenario -and $_.metric -eq "raw_ratio" })
  if ($rows.Count -ne 1) { throw "Raspberry profile evidence did not contain exactly one row for $($Cell.scenario)." }
  return $rows[0]
}

function Get-RestoreStatus {
  param([Parameter(Mandatory)][string]$StdoutPath)
  $values = @{}
  foreach ($line in Get-Content -LiteralPath $StdoutPath) { $parts = $line -split "=", 2; if ($parts.Count -eq 2) { $values[$parts[0]] = $parts[1] } }
  if ($values.restore_status -ne "0" -or $values.final_active -ne "active" -or $values.final_enabled -ne "enabled") { throw "Raspberry profile service restoration failed." }
}

function Invoke-PiProfileCell {
  param([Parameter(Mandatory)][pscustomobject]$Cell, [Parameter(Mandatory)][int]$Repetition, [Parameter(Mandatory)][string]$Directory)
  $cellDirectory = Join-Path $Directory ("{0:D2}-{1}-rep{2}" -f $Repetition, $Cell.id, $Repetition)
  New-Item -ItemType Directory -Force -Path $cellDirectory | Out-Null
  $stdout = Join-Path $cellDirectory "runner.stdout.txt"
  $stderr = Join-Path $cellDirectory "runner.stderr.txt"
  $arguments = @("-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", $RunnerPath, "-Target", $Target, "-Key", $Key, "-Binary", $Binary, "-Metadata", $Metadata, "-Mode", "ProfileBaseline", "-Scenario", $Cell.scenario, "-AudioBlockFrames", [string]$Cell.internal_frames, "-ProfileMeasureFrames", [string]$Cell.measure_frames, "-SynthSlotWorkers", [string]$Cell.workers, "-AllowServiceInterruption")
  $process = Invoke-FreshPowerShell $arguments $stdout $stderr
  $systemEvidence = Assert-RaspberrySystemEvidence (Get-Content -LiteralPath $stdout -Raw) "Raspberry profile $($Cell.id) repetition $Repetition"
  Get-RestoreStatus $stdout
  if ($process.ExitCode -ne 0) { throw "Raspberry profile runner exited with code $($process.ExitCode)." }
  $row = Get-ProfileRow $stdout $Cell
  $classified = ConvertTo-PerformanceBaselineOfflineResult -Row $row -Cell $Cell -SampleRate $manifest.sample_rate -Observations $manifest.offline_observations
  return [pscustomobject]@{ CellId = $Cell.id; Scenario = $Cell.scenario; Kind = "offline"; Repetition = $Repetition; StatusClass = $classified.StatusClass; ExitCode = $process.ExitCode; EvidenceDirectory = $cellDirectory; StdoutPath = $stdout; StderrPath = $stderr; OverBudget = $classified.OverBudget; P99_9 = $classified.P99_9; Max = $classified.Max; WorkersEffective = $classified.WorkersEffective; WorkerFailure = $classified.WorkerFailure; WorkerFailureReason = $classified.WorkerFailureReason; CallbackFields = $null; SystemEvidence = $systemEvidence; Row = $classified.Row }
}

function Get-JsonPayload {
  param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][ValidateSet("array", "object")][string]$Shape)
  $text = Get-Content -LiteralPath $Path -Raw
  $startToken = if ($Shape -eq "array") { "[" } else { "{" }
  $endToken = if ($Shape -eq "array") { "]" } else { "}" }
  $start = $text.IndexOf($startToken, [StringComparison]::Ordinal)
  $end = $text.LastIndexOf($endToken, [StringComparison]::Ordinal)
  if ($start -lt 0 -or $end -lt $start) { throw "Raspberry probe JSON evidence is missing from $Path." }
  return $text.Substring($start, $end - $start + 1) | ConvertFrom-Json
}

function Invoke-PiLiveProbe {
  param([Parameter(Mandatory)][pscustomobject]$Cell, [Parameter(Mandatory)][int]$Repetition, [Parameter(Mandatory)][string]$Mode, [Parameter(Mandatory)][string]$Directory)
  $probeDirectory = Join-Path $Directory ("{0:D2}-{1}-rep{2}\{3}" -f $Repetition, $Cell.id, $Repetition, $Mode.ToLowerInvariant())
  New-Item -ItemType Directory -Force -Path $probeDirectory | Out-Null
  $stdout = Join-Path $probeDirectory "stdout.txt"
  $stderr = Join-Path $probeDirectory "stderr.txt"
  $arguments = @("-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", $RunnerPath, "-Target", $Target, "-Key", $Key, "-Binary", $Binary, "-Metadata", $Metadata, "-Mode", $Mode, "-Durations", "$($Cell.duration_seconds)s", "-AudioOutputBufferFrames", [string]$Cell.output_frames, "-AudioBlockFrames", [string]$Cell.internal_frames, "-SynthSlotWorkers", [string]$Cell.workers, "-AllowServiceInterruption")
  if ($Mode -eq "Live") { $arguments += @("-Scenarios", $Cell.scenario) }
  $process = Invoke-FreshPowerShell $arguments $stdout $stderr
  $systemEvidence = Assert-RaspberrySystemEvidence (Get-Content -LiteralPath $stdout -Raw) "Raspberry $Mode $($Cell.id) repetition $Repetition"
  Get-RestoreStatus $stdout
  if ($process.ExitCode -ne 0) { throw "Raspberry $Mode probe exited with code $($process.ExitCode)." }
  $summary = if ($Mode -eq "Live") { @(Get-JsonPayload $stdout "array") } else { Get-JsonPayload $stdout "object" }
  if ($Mode -eq "Live") {
    if ($summary.Count -ne 1 -or [string]$summary[0].scenario -cne ($Cell.scenario -replace "-", "_") -or [int]$summary[0].duration_ms -ne $Cell.duration_seconds * 1000) { throw "Raspberry Live summary did not match the requested cell." }
  } elseif ([int]$summary.duration_ms -ne $Cell.duration_seconds * 1000 -or [int]$summary.marks -le 0) {
    throw "Raspberry AudioDrain summary did not match the requested cell."
  }
  $result = [ordered]@{ CellId = $Cell.id; Scenario = $Cell.scenario; Kind = $Mode; Repetition = $Repetition; StatusClass = "pass"; ExitCode = $process.ExitCode; EvidenceDirectory = $probeDirectory; StdoutPath = $stdout; StderrPath = $stderr; OutputFrames = $Cell.output_frames; InternalFrames = $Cell.internal_frames; Workers = $Cell.workers; CallbackFields = $null; SystemEvidence = $systemEvidence; Summary = $summary }
  return [pscustomobject]$result
}

function Write-CohortManifest {
  param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][object]$Value)
  $temporary = "$Path.tmp-$PID"
  [IO.File]::WriteAllText($temporary, ($Value | ConvertTo-Json -Depth 12), (New-Object System.Text.UTF8Encoding($false)))
  Move-Item -LiteralPath $temporary -Destination $Path -Force
}

function Add-Result {
  param([Parameter(Mandatory)][object]$ManifestValue, [Parameter(Mandatory)][object]$Result)
  $ManifestValue.results += $Result
  Write-CohortManifest $cohortManifestPath $ManifestValue
}

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) { $OutputDirectory = Get-DefaultOutputDirectory }
Assert-PerformanceBaselinePath $OutputDirectory "Raspberry baseline output path"
if ($needsActiveRun -and -not $PrintOnly -and -not $AllowServiceInterruption) { throw "Active Raspberry baseline execution requires -AllowServiceInterruption." }
if ($needsActiveRun -and -not $PrintOnly -and [string]::IsNullOrWhiteSpace($Metadata)) { throw "Active Raspberry baseline execution requires exact -Metadata." }
if ($needsActiveRun -and -not $PrintOnly -and [string]::IsNullOrWhiteSpace($Artifact)) { throw "Active Raspberry baseline execution requires exact -Artifact." }
if ($needsActiveRun -and -not $PrintOnly) {
  foreach ($pathValue in @($Artifact, $Metadata)) { Assert-PerformanceBaselinePath $pathValue "Raspberry baseline artifact path" }
  $repositoryIdentity = Get-RepositoryIdentity
  Assert-PerformanceBaselineSourceContext $repositoryIdentity.Head $repositoryIdentity.Status
  if (-not (Test-Path -LiteralPath $Artifact -PathType Leaf)) { throw "Raspberry baseline artifact was not found: $Artifact" }
  $localArtifactHash = (Get-FileHash -LiteralPath $Artifact -Algorithm SHA256).Hash.ToLowerInvariant()
  Assert-RaspberryBuildMetadata -Metadata (Read-RaspberryBoardMetadata $Metadata) -SourceCommit $repositoryIdentity.Head -BinaryPath $Artifact | Out-Null
} else {
  $repositoryIdentity = Get-RepositoryIdentity
  $localArtifactHash = $null
}
if ($PrintOnly) {
  Write-Output "Raspberry performance baseline PrintOnly: no transport is invoked."
  Write-Output "Target: $Target"
  Write-Output "Manifest: $ManifestPath"
  Write-Output "01: passive board identity"
  $index = 2
  if ($CanaryOnly) {
    $canary = Get-PerformanceBaselineCanaryCells $manifest
    foreach ($item in @(Get-PerformanceBaselineRoundRobinPlan @($canary.Offline) $manifest.repetitions)) { Write-Output ("{0:D2}: offline repetition={1}/{2} cell={3} scenario={4} internal={5} measure={6} workers={7}" -f $index, $item.Repetition, $manifest.repetitions, $item.Cell.id, $item.Cell.scenario, $item.Cell.internal_frames, $item.Cell.measure_frames, $item.Cell.workers); $index++ }
    foreach ($item in @(Get-PerformanceBaselineRoundRobinPlan @($manifest.raspberry.live_cells | Where-Object { $_.output_frames -eq 256 }) $manifest.repetitions)) { foreach ($mode in @("Live", "AudioDrain")) { Write-Output ("{0:D2}: live mode={1} repetition={2}/{3} cell={4} scenario={5} output={6} internal={7} workers={8} callback=null" -f $index, $mode, $item.Repetition, $manifest.repetitions, $item.Cell.id, $item.Cell.scenario, $item.Cell.output_frames, $item.Cell.internal_frames, $item.Cell.workers); $index++ } }
  } else {
    if ($Phase -in @("Offline", "Full")) { foreach ($item in @(Get-PerformanceBaselineRoundRobinPlan (Get-PerformanceBaselineOfflineCells $manifest "raspberry-pi-zero-2w") $manifest.repetitions)) { Write-Output ("{0:D2}: offline repetition={1}/{2} cell={3} scenario={4} internal={5} measure={6} workers={7}" -f $index, $item.Repetition, $manifest.repetitions, $item.Cell.id, $item.Cell.scenario, $item.Cell.internal_frames, $item.Cell.measure_frames, $item.Cell.workers); $index++ } }
    if ($Phase -in @("Live", "Full")) { foreach ($item in @(Get-PerformanceBaselineRoundRobinPlan $manifest.raspberry.live_cells $manifest.repetitions)) { foreach ($mode in @("Live", "AudioDrain")) { Write-Output ("{0:D2}: live mode={1} repetition={2}/{3} cell={4} scenario={5} output={6} internal={7} workers={8} callback=null" -f $index, $mode, $item.Repetition, $manifest.repetitions, $item.Cell.id, $item.Cell.scenario, $item.Cell.output_frames, $item.Cell.internal_frames, $item.Cell.workers); $index++ } } }
  }
  exit 0
}

New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$cohortManifestPath = Join-Path $OutputDirectory "cohort-manifest.json"
$cohortManifest = [ordered]@{ schema_version = 1; study_id = $manifest.study_id; board_profile = "raspberry-pi-zero-2w"; target = $Target; sample_rate = $manifest.sample_rate; warmup_seconds = $manifest.warmup_seconds; offline_observations = $manifest.offline_observations; repetitions = $manifest.repetitions; source = [ordered]@{ repository = $repoRoot; head = $repositoryIdentity.Head; status = $repositoryIdentity.Status }; artifact = [ordered]@{ local = $Artifact; binary = $Binary; metadata = $Metadata; local_sha256 = $localArtifactHash; remote_sha256 = $null }; manifest = (Resolve-Path $ManifestPath).Path; phase = $Phase; canary_only = [bool]$CanaryOnly; identity = $null; results = @() }
Write-CohortManifest $cohortManifestPath $cohortManifest
$identity = Invoke-PiIdentity $OutputDirectory
$cohortManifest.artifact.remote_sha256 = $identity.BinarySha256
$cohortManifest.artifact.local_sha256 = $localArtifactHash
$cohortManifest.identity = [ordered]@{ stdout = $identity.StdoutPath; stderr = $identity.StderrPath; values = Read-IdentityValues $identity.StdoutPath }
Write-CohortManifest $cohortManifestPath $cohortManifest
if ($needsActiveRun) { Assert-PerformanceBaselineArtifactMatch $localArtifactHash $identity.BinarySha256 }
if ($Phase -eq "Passive" -and -not $CanaryOnly) { Write-Output "Raspberry performance baseline passive phase completed: $cohortManifestPath"; exit 0 }
$canary = Get-PerformanceBaselineCanaryCells $manifest
$offlineCells = if ($CanaryOnly) { @($canary.Offline) } else { Get-PerformanceBaselineOfflineCells $manifest "raspberry-pi-zero-2w" }
$offlineEnabled = $CanaryOnly -or $Phase -in @("Offline", "Full")
$liveEnabled = $CanaryOnly -or $Phase -in @("Live", "Full")
if ($offlineEnabled) { foreach ($item in @(Get-PerformanceBaselineRoundRobinPlan $offlineCells $manifest.repetitions)) { $result = Invoke-PiProfileCell $item.Cell $item.Repetition $OutputDirectory; Add-Result $cohortManifest $result; if (-not (Test-PerformanceBaselineMeasuredOutcome $result.StatusClass)) { throw "Raspberry baseline stopped at $($item.Cell.id): $($result.StatusClass)" } } }
if ($liveEnabled) {
  $liveCells = if ($CanaryOnly) { @($manifest.raspberry.live_cells | Where-Object { $_.output_frames -eq 256 }) } else { @($manifest.raspberry.live_cells) }
  foreach ($item in @(Get-PerformanceBaselineRoundRobinPlan $liveCells $manifest.repetitions)) { foreach ($mode in @("Live", "AudioDrain")) { $result = Invoke-PiLiveProbe $item.Cell $item.Repetition $mode $OutputDirectory; Add-Result $cohortManifest $result } }
}
Write-Output "Raspberry performance baseline completed: $cohortManifestPath"
