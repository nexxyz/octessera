[CmdletBinding()]
param(
  [string]$ManifestPath = "",
  [ValidateSet("Passive", "Offline", "Live", "Full")]
  [string]$Phase = "Full",
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
$metadataModule = Join-Path $PSScriptRoot "orange-cross-metadata.psm1"
Import-Module $manifestModule -Force
Import-Module $resultsModule -Force
Import-Module $metadataModule -Force
$transport = Join-Path $PSScriptRoot "with-orange-ssh.ps1"
$target = "octessera@192.168.0.217"
$service = "octessera.service"
$defaultManifestPath = Join-Path $PSScriptRoot "..\performance\cross-board-baseline.json"
$defaultRunnerPath = Join-Path $PSScriptRoot "run-orange-capability-study.ps1"
if ([string]::IsNullOrWhiteSpace($ManifestPath)) { $ManifestPath = $defaultManifestPath }
if ([string]::IsNullOrWhiteSpace($RunnerPath)) { $RunnerPath = $defaultRunnerPath }
$manifestPathValues = @($ManifestPath, $RunnerPath)
foreach ($pathValue in $manifestPathValues) { Assert-PerformanceBaselinePath $pathValue "Orange baseline path" }
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$manifest = Read-PerformanceBaselineManifest $ManifestPath
$needsActiveRun = $CanaryOnly -or $Phase -in @("Offline", "Live", "Full")

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
  $commit = Get-CommitIdentity
  return Join-Path $repoRoot (Join-Path "target\performance-baselines\orange-pi-zero-2w" "$date-$commit")
}

function Invoke-FreshPowerShell {
  param(
    [Parameter(Mandatory)][string[]]$Arguments,
    [Parameter(Mandatory)][string]$StdoutPath,
    [Parameter(Mandatory)][string]$StderrPath
  )
  $processArguments = @($Arguments | ForEach-Object { if ($_ -match '\s') { '"' + $_ + '"' } else { $_ } })
  $process = Start-Process `
    -FilePath (Join-Path $PSHOME "powershell.exe") `
    -ArgumentList $processArguments `
    -RedirectStandardOutput $StdoutPath `
    -RedirectStandardError $StderrPath `
    -Wait `
    -PassThru
  return [pscustomobject]@{ ExitCode = $process.ExitCode }
}

function Get-IdentityCommand {
  return @'
set -eu
printf 'board_profile='; sed -n 's/^OCTESSERA_BOARD_PROFILE_ID=//p' /etc/octessera/build-metadata.env 2>/dev/null || true
printf 'hostname='; hostname
printf 'kernel='; uname -a
printf 'governor='; for path in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do [ -r "$path" ] && printf '%s:%s ' "$path" "$(cat "$path")"; done; printf '\n'
printf 'frequency='; for path in /sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq; do [ -r "$path" ] && printf '%s:%s ' "$path" "$(cat "$path")"; done; printf '\n'
printf 'thermal='; for path in /sys/class/thermal/thermal_zone*/temp; do [ -r "$path" ] && printf '%s:%s ' "$path" "$(cat "$path")"; done; printf '\n'
printf 'load='; cut -d' ' -f1-3 /proc/loadavg
printf 'memory='; awk '/^MemAvailable:/ { print $2; exit }' /proc/meminfo
printf 'service_active='; systemctl is-active octessera.service 2>/dev/null || true
printf 'service_enabled='; systemctl is-enabled octessera.service 2>/dev/null || true
'@
}

function Invoke-OrangeIdentity {
  param([Parameter(Mandatory)][string]$Directory)
  $stdout = Join-Path $Directory "passive.stdout.txt"
  $stderr = Join-Path $Directory "passive.stderr.txt"
  $command = Get-IdentityCommand
  & $transport "ssh" $target $command 1> $stdout 2> $stderr
  $exitCode = if ($null -eq $LASTEXITCODE) { 0 } else { [int]$LASTEXITCODE }
  if ($exitCode -ne 0) { throw "Orange passive identity collection failed: $exitCode" }
  $identityText = Get-Content -LiteralPath $stdout -Raw
  if ($identityText -notmatch "board_profile=orange-pi-zero-2w" -or $identityText -notmatch "service_active=active" -or $identityText -notmatch "service_enabled=enabled") { throw "Orange passive identity did not prove the fixed board and managed service state." }
  return [pscustomobject]@{ StdoutPath = $stdout; StderrPath = $stderr }
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

function Get-EvidenceDirectory {
  param([Parameter(Mandatory)][string]$StdoutPath)
  $lines = @(Get-Content -LiteralPath $StdoutPath)
  $matches = @($lines | Where-Object { $_ -like "Evidence directory: *" })
  if ($matches.Count -ne 1) { throw "Orange single-cell runner did not publish exactly one evidence directory." }
  return ([string]$matches[0]).Substring("Evidence directory: ".Length).Trim()
}

function Get-ProfileRow {
  param(
    [Parameter(Mandatory)][string]$EvidenceDirectory,
    [Parameter(Mandatory)][pscustomobject]$Cell
  )
  $path = Join-Path $EvidenceDirectory "profile.csv"
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Orange profile evidence is missing: $path" }
  $rows = @(Import-Csv -LiteralPath $path | Where-Object { $_.kind -eq "engine_source" -and $_.scenario -eq $Cell.scenario -and $_.metric -eq "raw_ratio" })
  if ($rows.Count -ne 1) { throw "Orange profile evidence did not contain exactly one row for $($Cell.scenario)." }
  return $rows[0]
}

function Read-StudyValues {
  param([Parameter(Mandatory)][string]$EvidenceDirectory)
  $values = @{}
  $path = Join-Path $EvidenceDirectory "study-result.txt"
  if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { throw "Orange study result evidence is missing: $path" }
  foreach ($line in Get-Content -LiteralPath $path) { $parts = $line -split "=", 2; if ($parts.Count -eq 2) { $values[$parts[0]] = $parts[1] } }
  return $values
}

function Get-OrangeSafetyFailureReason {
  param([Parameter(Mandatory)][string]$EvidenceDirectory)
  $abortPath = Join-Path $EvidenceDirectory "safety-abort.txt"
  if (Test-Path -LiteralPath $abortPath -PathType Leaf) {
    foreach ($line in Get-Content -LiteralPath $abortPath) {
      if ($line -match '^reason=(?<reason>.*)$') { return $Matches.reason }
    }
  }
  $study = Read-StudyValues $EvidenceDirectory
  if ($study.ContainsKey("reason") -and -not [string]::IsNullOrWhiteSpace([string]$study.reason)) {
    return [string]$study.reason
  }
  return "unspecified safety failure"
}

function Invoke-OrangeCell {
  param(
    [Parameter(Mandatory)][pscustomobject]$Cell,
    [Parameter(Mandatory)][string]$Kind,
    [Parameter(Mandatory)][int]$Repetition,
    [Parameter(Mandatory)][string]$Directory
  )
  $cellDirectory = Join-Path $Directory ("{0:D2}-{1}-rep{2}" -f $Repetition, $Cell.id, $Repetition)
  New-Item -ItemType Directory -Force -Path $cellDirectory | Out-Null
  $stdout = Join-Path $cellDirectory "runner.stdout.txt"
  $stderr = Join-Path $cellDirectory "runner.stderr.txt"
  $arguments = @(
    "-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", $RunnerPath,
    "-OutputDirectory", $cellDirectory, "-Artifact", $Artifact, "-Metadata", $Metadata, "-AllowServiceInterruption"
  )
  if ($Kind -eq "offline") {
    $arguments += @("-Mode", "ProfileBaseline", "-Scenario", $Cell.scenario, "-EngineBlockFrames", [string]$Cell.internal_frames, "-ProfileMeasureFrames", [string]$Cell.measure_frames, "-Workers", [string]$Cell.workers)
  } else {
    $arguments += @("-Mode", "LiveAudioBenchmark", "-Scenario", $Cell.scenario, "-OutputFrames", [string]$Cell.output_frames, "-EngineBlockFrames", [string]$Cell.internal_frames, "-Workers", [string]$Cell.workers, "-MeasureSeconds", [string]$Cell.measure_seconds)
  }
  if ($Kind -eq "live" -and $Cell.measure_seconds -eq 120) { $arguments += "-AllowLongRepeat" }
  $process = Invoke-FreshPowerShell $arguments $stdout $stderr
  try { $evidence = Get-EvidenceDirectory $stdout } catch { throw "Orange $Kind cell $($Cell.id) repetition $Repetition failed infrastructure checks: $($_.Exception.Message)" }
  $remoteHashPath = Join-Path $evidence "runtime-candidate-sha256.txt"
  if (-not (Test-Path -LiteralPath $remoteHashPath -PathType Leaf)) { throw "Orange $Kind cell $($Cell.id) did not retain the remote artifact hash." }
  $remoteArtifactHash = (Get-Content -LiteralPath $remoteHashPath -Raw).Trim()
  Assert-PerformanceBaselineArtifactMatch $localArtifactHash $remoteArtifactHash
  if ($Kind -eq "offline") {
    try {
      $study = Read-StudyValues $evidence
      if ($study.status_class -eq "safety_failure") {
        throw "Orange offline safety gate failed: $(Get-OrangeSafetyFailureReason $evidence)"
      }
      if ($study.status_class -cne "measured" -or $study.status -cne "0") { throw "Orange offline study did not complete its safety contract: status=$($study.status) class=$($study.status_class)" }
      foreach ($evidenceName in @("system-evidence.txt", "governor-before.txt", "governor-after.txt")) {
        if (-not (Test-Path -LiteralPath (Join-Path $evidence $evidenceName) -PathType Leaf)) { throw "Orange offline safety evidence is missing: $evidenceName" }
      }
      if ((Test-Path -LiteralPath (Join-Path $evidence "safety-abort.txt") -PathType Leaf) -or (Test-Path -LiteralPath (Join-Path $evidence "governor-abort.txt") -PathType Leaf)) { throw "Orange offline safety contract reported an abort." }
      if ($process.ExitCode -ne 0) { throw "single-cell runner exited with code $($process.ExitCode)" }
      $row = Get-ProfileRow $evidence $Cell
      $classified = ConvertTo-PerformanceBaselineOfflineResult -Row $row -Cell $Cell -SampleRate $manifest.sample_rate -Observations $manifest.offline_observations
      return [pscustomobject]@{ CellId = $Cell.id; Scenario = $Cell.scenario; Kind = $Kind; Repetition = $Repetition; StatusClass = $classified.StatusClass; ExitCode = $process.ExitCode; EvidenceDirectory = $evidence; StdoutPath = $stdout; StderrPath = $stderr; OverBudget = $classified.OverBudget; P99_9 = $classified.P99_9; Max = $classified.Max; WorkersEffective = $classified.WorkersEffective; WorkerFailure = $classified.WorkerFailure; WorkerFailureReason = $classified.WorkerFailureReason; RemoteArtifactSha256 = $remoteArtifactHash; Row = $classified.Row }
    } catch { throw "Orange offline cell $($Cell.id) repetition $Repetition failed identity/geometry validation: $($_.Exception.Message)" }
  }
  $hostEvidencePath = Join-Path $evidence "host-evidence.json"
  if (-not (Test-Path -LiteralPath $hostEvidencePath -PathType Leaf)) { throw "Orange live cell $($Cell.id) did not retain host evidence." }
  $hostEvidence = Get-Content -LiteralPath $hostEvidencePath -Raw | ConvertFrom-Json
  if ([string]$hostEvidence.Scenario -cne $Cell.scenario -or [int]$hostEvidence.OutputFrames -ne $Cell.output_frames -or [int]$hostEvidence.InternalFrames -ne $Cell.internal_frames -or [int]$hostEvidence.Workers -ne $Cell.workers) { throw "Orange live cell $($Cell.id) failed identity/geometry validation." }
  if (@("pass", "over_budget", "measured_failure") -notcontains [string]$hostEvidence.StatusClass) { throw "Orange live cell $($Cell.id) stopped on $($hostEvidence.StatusClass)." }
  if ([string]$hostEvidence.StatusClass -eq "pass" -and $process.ExitCode -ne 0) { throw "Orange live cell $($Cell.id) published pass evidence with a failing process." }
  return [pscustomobject]@{ CellId = $Cell.id; Scenario = $Cell.scenario; Kind = $Kind; Repetition = $Repetition; StatusClass = [string]$hostEvidence.StatusClass; ExitCode = $process.ExitCode; EvidenceDirectory = $evidence; StdoutPath = $stdout; StderrPath = $stderr; OverBudget = [int64]$hostEvidence.OverBudget; P99_9 = [double]$hostEvidence.RatioP999; Max = [double]$hostEvidence.RatioMax; RemoteArtifactSha256 = $remoteArtifactHash; HostEvidence = $hostEvidence }
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
  $remoteHash = $Result.PSObject.Properties["RemoteArtifactSha256"]
  if ($null -ne $remoteHash -and $null -ne $remoteHash.Value) {
    if ($null -eq $ManifestValue.artifact.remote_sha256) { $ManifestValue.artifact.remote_sha256 = [string]$remoteHash.Value } else { Assert-PerformanceBaselineArtifactMatch $ManifestValue.artifact.local_sha256 ([string]$remoteHash.Value) }
  }
  Write-CohortManifest $cohortManifestPath $ManifestValue
}

if ([string]::IsNullOrWhiteSpace($OutputDirectory)) { $OutputDirectory = Get-DefaultOutputDirectory }
Assert-PerformanceBaselinePath $OutputDirectory "Orange baseline output path"
if ($PrintOnly) {
  Write-Output "Orange performance baseline PrintOnly: no transport is invoked."
  Write-Output "Target: $target"
  Write-Output "Manifest: $ManifestPath"
  Write-Output "01: passive board identity"
  $index = 2
  if ($CanaryOnly) {
    $canary = Get-PerformanceBaselineCanaryCells $manifest
    foreach ($item in @(Get-PerformanceBaselineRoundRobinPlan @($canary.Offline) $manifest.repetitions)) { Write-Output ("{0:D2}: offline repetition={1}/{2} cell={3} scenario={4} internal={5} measure={6} workers={7}" -f $index, $item.Repetition, $manifest.repetitions, $item.Cell.id, $item.Cell.scenario, $item.Cell.internal_frames, $item.Cell.measure_frames, $item.Cell.workers); $index++ }
    foreach ($item in @(Get-PerformanceBaselineRoundRobinPlan @($canary.OrangeLive) $manifest.repetitions)) { Write-Output ("{0:D2}: live repetition={1}/{2} cell={3} scenario={4} output={5} internal={6} workers={7}" -f $index, $item.Repetition, $manifest.repetitions, $item.Cell.id, $item.Cell.scenario, $item.Cell.output_frames, $item.Cell.internal_frames, $item.Cell.workers); $index++ }
  } else {
    if ($Phase -in @("Offline", "Full")) { foreach ($item in @(Get-PerformanceBaselineRoundRobinPlan (Get-PerformanceBaselineOfflineCells $manifest "orange-pi-zero-2w") $manifest.repetitions)) { Write-Output ("{0:D2}: offline repetition={1}/{2} cell={3} scenario={4} internal={5} measure={6} workers={7}" -f $index, $item.Repetition, $manifest.repetitions, $item.Cell.id, $item.Cell.scenario, $item.Cell.internal_frames, $item.Cell.measure_frames, $item.Cell.workers); $index++ } }
    if ($Phase -in @("Live", "Full")) { foreach ($item in @(Get-PerformanceBaselineRoundRobinPlan (Get-PerformanceBaselineOrangeLiveCells $manifest) $manifest.repetitions)) { Write-Output ("{0:D2}: live repetition={1}/{2} cell={3} scenario={4} output={5} internal={6} workers={7}" -f $index, $item.Repetition, $manifest.repetitions, $item.Cell.id, $item.Cell.scenario, $item.Cell.output_frames, $item.Cell.internal_frames, $item.Cell.workers); $index++ }; Write-Output ("{0:D2}: live dynamic cell={1} measure=120 selection=p99.9_then_max" -f $index, $manifest.orange.long_repeat.id) }
  }
  exit 0
}

if ($needsActiveRun -and -not $AllowServiceInterruption) { throw "Active Orange baseline execution requires -AllowServiceInterruption." }
if ($needsActiveRun -and ([string]::IsNullOrWhiteSpace($Artifact) -or [string]::IsNullOrWhiteSpace($Metadata))) { throw "Active Orange baseline execution requires exact -Artifact and -Metadata paths." }
$repositoryIdentity = $null
$localArtifactHash = $null
if ($needsActiveRun) {
  foreach ($pathValue in @($Artifact, $Metadata)) { Assert-PerformanceBaselinePath $pathValue "Orange baseline artifact path" }
  $repositoryIdentity = Get-RepositoryIdentity
  Assert-PerformanceBaselineSourceContext $repositoryIdentity.Head $repositoryIdentity.Status
  if (-not (Test-Path -LiteralPath $Artifact -PathType Leaf)) { throw "Orange baseline artifact was not found: $Artifact" }
  $localArtifactHash = (Get-FileHash -LiteralPath $Artifact -Algorithm SHA256).Hash.ToLowerInvariant()
  $buildSpec = [pscustomobject]@{ Package = "octessera-pi"; Feature = "hardware-orange-pi-zero-2w"; ArtifactKind = "runtime-candidate" }
  Assert-OrangeBuildMetadata -MetadataPath $Metadata -BinaryPath $Artifact -SelectedBinary "octessera-pi" -SelectedTarget "aarch64-unknown-linux-gnu" -SelectedProfile "release" -BuildSpec $buildSpec -SourceCommit $repositoryIdentity.Head
}
if ($null -eq $repositoryIdentity) { $repositoryIdentity = Get-RepositoryIdentity }
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$cohortManifestPath = Join-Path $OutputDirectory "cohort-manifest.json"
$cohortManifest = [ordered]@{ schema_version = 1; study_id = $manifest.study_id; board_profile = "orange-pi-zero-2w"; target = $target; sample_rate = $manifest.sample_rate; warmup_seconds = $manifest.warmup_seconds; offline_observations = $manifest.offline_observations; repetitions = $manifest.repetitions; source = [ordered]@{ repository = $repoRoot; head = $repositoryIdentity.Head; status = $repositoryIdentity.Status }; artifact = [ordered]@{ local = $Artifact; binary = $Artifact; metadata = $Metadata; local_sha256 = $localArtifactHash; remote_sha256 = $null }; manifest = (Resolve-Path $ManifestPath).Path; phase = $Phase; canary_only = [bool]$CanaryOnly; identity = $null; results = @() }
Write-CohortManifest $cohortManifestPath $cohortManifest
$identity = Invoke-OrangeIdentity $OutputDirectory
$cohortManifest.identity = [ordered]@{ stdout = $identity.StdoutPath; stderr = $identity.StderrPath; values = Read-IdentityValues $identity.StdoutPath }
Write-CohortManifest $cohortManifestPath $cohortManifest
if ($Phase -eq "Passive" -and -not $CanaryOnly) { Write-Output "Orange performance baseline passive phase completed: $cohortManifestPath"; exit 0 }
$canary = Get-PerformanceBaselineCanaryCells $manifest
$offlineCells = if ($CanaryOnly) { @($canary.Offline) } else { Get-PerformanceBaselineOfflineCells $manifest "orange-pi-zero-2w" }
$liveCells = if ($CanaryOnly) { @($canary.OrangeLive) } else { Get-PerformanceBaselineOrangeLiveCells $manifest }
$offlineEnabled = $CanaryOnly -or $Phase -in @("Offline", "Full")
$liveEnabled = $CanaryOnly -or $Phase -in @("Live", "Full")
if ($offlineEnabled) { foreach ($item in @(Get-PerformanceBaselineRoundRobinPlan $offlineCells $manifest.repetitions)) { $result = Invoke-OrangeCell $item.Cell "offline" $item.Repetition $OutputDirectory; Add-Result $cohortManifest $result; if (-not (Test-PerformanceBaselineMeasuredOutcome $result.StatusClass)) { throw "Orange baseline stopped at $($item.Cell.id): $($result.StatusClass)" } } }
if ($liveEnabled) {
  foreach ($item in @(Get-PerformanceBaselineRoundRobinPlan $liveCells $manifest.repetitions)) { $result = Invoke-OrangeCell $item.Cell "live" $item.Repetition $OutputDirectory; Add-Result $cohortManifest $result; if (-not (Test-PerformanceBaselineMeasuredOutcome $result.StatusClass)) { throw "Orange baseline stopped at $($item.Cell.id): $($result.StatusClass)" } }
  if (-not $CanaryOnly) {
    $worst = Select-PerformanceBaselineWorstPassingDefault -Results @($cohortManifest.results) -DefaultCellIds @($manifest.orange.live_defaults.id) -Repetitions $manifest.repetitions
    $longCell = [pscustomobject]@{ id = $manifest.orange.long_repeat.id; scenario = $worst.Scenario; output_frames = 256; internal_frames = 64; workers = 2; measure_seconds = 120 }
    $long = Invoke-OrangeCell $longCell "live" 1 $OutputDirectory
    $long | Add-Member -NotePropertyName Selection -NotePropertyValue "p99.9_then_max:$($worst.CellId)"
    Add-Result $cohortManifest $long
    if (-not (Test-PerformanceBaselineMeasuredOutcome $long.StatusClass)) { throw "Orange baseline stopped at dynamic long repeat: $($long.StatusClass)" }
  }
}
Write-Output "Orange performance baseline completed: $cohortManifestPath"
