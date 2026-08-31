$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$driver = Join-Path $PSScriptRoot "run-orange-performance-baseline.ps1"
$runner = Join-Path $PSScriptRoot "run-orange-capability-study.ps1"
$driverSource = [IO.File]::ReadAllText($driver)
$runnerSource = [IO.File]::ReadAllText($runner)
$payloadSource = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "orange-capability-study-payloads.psm1"))
$manifestModule = Join-Path $PSScriptRoot "..\performance\performance-baseline-plan.psm1"
$resultsModule = Join-Path $PSScriptRoot "..\performance\performance-baseline-results.psm1"
Import-Module $manifestModule -Force
Import-Module $resultsModule -Force
$manifestPath = Join-Path $PSScriptRoot "..\performance\cross-board-baseline.json"
$manifest = Read-PerformanceBaselineManifest $manifestPath
if ($driverSource -notmatch 'Assert-OrangeBuildMetadata[^\r\n]+-SourceCommit \$repositoryIdentity\.Head') { throw "Orange baseline driver does not bind metadata to the clean repository HEAD." }

function Assert-Throws {
  param([Parameter(Mandatory)][scriptblock]$Action, [Parameter(Mandatory)][string]$Label)
  $threw = $false
  try { & $Action } catch { $threw = $true }
  if (-not $threw) { throw "Expected failure did not occur: $Label" }
}

function Invoke-PrintOnly {
  param([Parameter(Mandatory)][string]$Path, [hashtable]$Parameters = @{})
  $arguments = @("-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", $Path)
  foreach ($key in $Parameters.Keys) {
    if ($Parameters[$key] -is [bool]) { if ($Parameters[$key]) { $arguments += "-$key" } } else { $arguments += @("-$key", [string]$Parameters[$key]) }
  }
  $output = @(& (Join-Path $PSHOME "powershell.exe") @arguments 2>&1)
  if ($null -ne $LASTEXITCODE -and $LASTEXITCODE -ne 0) { throw "PrintOnly failed for $Path" }
  return ($output | ForEach-Object { [string]$_ }) -join "`n"
}

function Assert-Contains {
  param([Parameter(Mandatory)][string]$Text, [Parameter(Mandatory)][string]$Value)
  if ($Text.IndexOf($Value, [StringComparison]::Ordinal) -lt 0) { throw "Missing expected text: $Value" }
}

Assert-Contains $driverSource "/etc/octessera/build-metadata.env"
if ($driverSource -match "/etc/octessera/board-profile\.env") { throw "Orange baseline passive identity regressed to the Raspberry board-profile path." }

function Write-ManifestCopy {
  param([Parameter(Mandatory)][object]$Value, [Parameter(Mandatory)][string]$Path)
  [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 12), (New-Object System.Text.UTF8Encoding($false)))
}

function New-OfflineRow {
  param([Parameter(Mandatory)][pscustomobject]$Cell, [int]$Dispatch = 0, [int]$LightSkips = 0)
  $effectiveWorkers = if ($Cell.workers -gt 0) { $Cell.workers } else { 0 }
  return [pscustomobject][ordered]@{
    kind = "engine_source"; scenario = $Cell.scenario; metric = "raw_ratio"; value = "1"; block_frames = [string]$Cell.measure_frames; sample_rate = "44100"; blocks = "4096"; avg = "1"; p95 = "1"; p99 = "1"; max = "1"; notes = ""; internal_block_frames = [string]$Cell.internal_frames; schema_version = "2"; p99_9 = "1"; over_audio_duration_budget_count = "0"; requested_measure_frames = [string]$Cell.measure_frames; requested_internal_block_frames = [string]$Cell.internal_frames; workers_requested_count = [string]$Cell.workers; workers_effective_count = [string]$effectiveWorkers; peak_synth_voices = "0"; peak_sample_voices = "0"; peak_preview_sample_voices = "0"; peak_momentary_fx = "0"; peak_bus_fx_slots = "0"; peak_global_fx_slots = "0"; peak_voice_steals = "0"; voice_steal_delta = "0"; synth_parallel_dispatch_delta = [string]$Dispatch; synth_parallel_light_skip_delta = [string]$LightSkips; synth_parallel_backoff_skip_delta = "0"; synth_parallel_timing_backoff_delta = "0"; synth_parallel_failure_delta = "0"; synth_parallel_unhealthy = "false"
  }
}

if ((Get-PerformanceBaselineOfflineCells $manifest "orange-pi-zero-2w").Count -ne 40 -or (Get-PerformanceBaselineOrangeLiveCells $manifest).Count -ne 14) { throw "Manifest plan counts changed." }
$offlinePlan = @(Get-PerformanceBaselineRoundRobinPlan (Get-PerformanceBaselineOfflineCells $manifest "orange-pi-zero-2w") $manifest.repetitions)
if ($offlinePlan.Count -ne 120 -or $offlinePlan[0].Repetition -ne 1 -or $offlinePlan[39].Repetition -ne 1 -or $offlinePlan[40].Repetition -ne 2 -or $offlinePlan[40].Cell.id -ne "common_baseline_idle" -or $offlinePlan[119].Repetition -ne 3) { throw "Offline plan is not round-robin by repetition." }
if (-not (Test-PerformanceBaselineMeasuredOutcome "over_budget") -or (Test-PerformanceBaselineMeasuredOutcome "safety_failure") -or (Test-PerformanceBaselineMeasuredOutcome "thermal_failure") -or (Test-PerformanceBaselineMeasuredOutcome "restoration_failure") -or (Test-PerformanceBaselineMeasuredOutcome "infrastructure_failure")) { throw "Measured/fatal outcome policy is incorrect." }
Assert-PerformanceBaselinePath (Join-Path ([IO.Path]::GetTempPath()) "octessera baseline spaced\runner.ps1") "spaced test path"
Assert-Throws { Assert-PerformanceBaselinePath "bad`"path" "quoted test path" } "quoted path"
Assert-Throws { Assert-PerformanceBaselineSourceContext ((& git rev-parse HEAD 2>$null | Out-String).Trim()) " M synthetic-dirty-source" } "dirty source"
$fullHead = ((& git rev-parse HEAD 2>$null | Out-String).Trim())
if ($fullHead.Length -ne 40) { throw "Repository identity is not a full commit." }
$sampleResult = ConvertTo-PerformanceBaselineOfflineResult -Row (New-OfflineRow $manifest.cohorts.common_reference.cells[3]) -Cell $manifest.cohorts.common_reference.cells[3] -SampleRate 44100 -Observations 4096
if ($sampleResult.StatusClass -cne "pass") { throw "Simple sample-only offline evidence without dispatch was rejected." }
$synthResult = ConvertTo-PerformanceBaselineOfflineResult -Row (New-OfflineRow $manifest.cohorts.common_reference.cells[2]) -Cell $manifest.cohorts.common_reference.cells[2] -SampleRate 44100 -Observations 4096
if ($synthResult.StatusClass -cne "measured_failure") { throw "Missing required synth dispatch was not classified as a measured failure." }
$maxFxCell = $manifest.cohorts.common_reference.cells[9]
$maxFxNoDispatch = ConvertTo-PerformanceBaselineOfflineResult -Row (New-OfflineRow $maxFxCell) -Cell $maxFxCell -SampleRate 44100 -Observations 4096
if ($maxFxNoDispatch.StatusClass -cne "measured_failure") { throw "Max-FX fixed-slot evidence without dispatch was not classified as a measured failure." }
$maxFxWithDispatch = ConvertTo-PerformanceBaselineOfflineResult -Row (New-OfflineRow $maxFxCell 2) -Cell $maxFxCell -SampleRate 44100 -Observations 4096
if ($maxFxWithDispatch.StatusClass -cne "pass") { throw "Max-FX fixed-slot evidence with required dispatch was not classified as pass." }
$backoffRow = New-OfflineRow $manifest.cohorts.common_reference.cells[2] 2
$backoffRow.synth_parallel_backoff_skip_delta = "1"
$backoffResult = ConvertTo-PerformanceBaselineOfflineResult -Row $backoffRow -Cell $manifest.cohorts.common_reference.cells[2] -SampleRate 44100 -Observations 4096
if ($backoffResult.StatusClass -cne "measured_failure") { throw "Worker backoff was not classified as a measured failure." }
$rankResults = @()
foreach ($repetition in 1..3) { $rankResults += [pscustomobject]@{ CellId = "orange_live_default_synth_cross_slot_16"; Scenario = "synth_cross_slot_16"; Repetition = $repetition; StatusClass = "pass"; P99_9 = 2 + $repetition / 10; Max = 3 }; $rankResults += [pscustomobject]@{ CellId = "orange_live_default_sample_cross_slot_64"; Scenario = "sample_cross_slot_64"; Repetition = $repetition; StatusClass = "pass"; P99_9 = 1; Max = 1 } }
$selected = Select-PerformanceBaselineWorstPassingDefault -Results $rankResults -DefaultCellIds @($manifest.orange.live_defaults.id) -Repetitions 3
if ($selected.CellId -cne "orange_live_default_synth_cross_slot_16") { throw "Dynamic worst-passing-default selection did not use p99.9 then max." }

$temporaryManifest = Join-Path ([IO.Path]::GetTempPath()) ("octessera-orange-baseline-manifest-" + [guid]::NewGuid().ToString("N") + ".json")
try {
  $mutated = ConvertFrom-Json -InputObject ([IO.File]::ReadAllText($manifestPath))
  $mutated.cohorts.common_reference.cells[0].scenario = "unknown_scenario"
  Write-ManifestCopy $mutated $temporaryManifest
  Assert-Throws { Read-PerformanceBaselineManifest $temporaryManifest | Out-Null } "unknown scenario ID"
  $mutated = ConvertFrom-Json -InputObject ([IO.File]::ReadAllText($manifestPath))
  $mutated.cohorts.common_reference.cells[0].internal_frames = 512
  Write-ManifestCopy $mutated $temporaryManifest
  Assert-Throws { Read-PerformanceBaselineManifest $temporaryManifest | Out-Null } "unknown offline geometry"
  $mutated = ConvertFrom-Json -InputObject ([IO.File]::ReadAllText($manifestPath))
  $mutated.orange.live_neighbors[0].output_frames = 256
  Write-ManifestCopy $mutated $temporaryManifest
  Assert-Throws { Read-PerformanceBaselineManifest $temporaryManifest | Out-Null } "unknown Orange geometry"
  [IO.File]::WriteAllText($temporaryManifest, '{"schema_version":1,"schema_version":1}', (New-Object System.Text.UTF8Encoding($false)))
  Assert-Throws { Read-PerformanceBaselineManifest $temporaryManifest | Out-Null } "duplicate JSON field"
  [IO.File]::WriteAllText($temporaryManifest, '{', (New-Object System.Text.UTF8Encoding($false)))
  Assert-Throws { Read-PerformanceBaselineManifest $temporaryManifest | Out-Null } "malformed JSON"
  $bomEncoding = New-Object System.Text.UTF8Encoding($true)
  [IO.File]::WriteAllBytes($temporaryManifest, $bomEncoding.GetPreamble() + $bomEncoding.GetBytes('{}'))
  Assert-Throws { Read-PerformanceBaselineManifest $temporaryManifest | Out-Null } "UTF-8 BOM"
} finally {
  Remove-Item -LiteralPath $temporaryManifest -Force -ErrorAction SilentlyContinue
}

$canary = Invoke-PrintOnly $driver @{ PrintOnly = $true; CanaryOnly = $true }
Assert-Contains $canary "Orange performance baseline PrintOnly: no transport is invoked."
Assert-Contains $canary "Target: octessera@192.168.0.217"
Assert-Contains $canary "01: passive board identity"
Assert-Contains $canary "02: offline repetition=1/3 cell=common_baseline_idle"
Assert-Contains $canary "05: live repetition=1/3 cell=orange_live_default_synth_cross_slot_16"
if ($canary -match "08:|with-orange-ssh.ps1|with-pi-ssh.ps1") { throw "Orange canary PrintOnly emitted transport or an extra cell." }
$spacedDirectory = Join-Path ([IO.Path]::GetTempPath()) ("octessera orange spaced " + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $spacedDirectory | Out-Null
$spacedRunner = Join-Path $spacedDirectory "runner.ps1"
Copy-Item -LiteralPath $runner -Destination $spacedRunner
$spaced = Invoke-PrintOnly $driver @{ PrintOnly = $true; RunnerPath = $spacedRunner }
Assert-Contains $spaced "Orange performance baseline PrintOnly: no transport is invoked."
Remove-Item -LiteralPath $spacedDirectory -Recurse -Force
$full = Invoke-PrintOnly $driver @{ PrintOnly = $true; Phase = "Full" }
Assert-Contains $full "offline repetition=2/3"
Assert-Contains $full "live repetition=3/3"
Assert-Contains $full "selection=p99.9_then_max"
Assert-Throws { & $driver -Phase Offline -Artifact "missing" -Metadata "missing.json" } "active consent"
Assert-Throws { & $driver -PrintOnly -RunnerPath "bad`"runner.ps1" } "quoted runner path"

$baselineArtifact = Join-Path ([IO.Path]::GetTempPath()) "octessera-orange-profile-baseline-missing"
$baselinePrint = Invoke-PrintOnly $runner @{ Mode = "ProfileBaseline"; Scenario = "synth_cross_slot_16"; EngineBlockFrames = 256; ProfileMeasureFrames = 256; Workers = 2; Artifact = $baselineArtifact; Metadata = "$baselineArtifact.metadata.json"; PrintOnly = $true }
Assert-Contains $baselinePrint "Profile baseline selection: scenario=synth_cross_slot_16 internal=256 measure=256 workers=2 warmup=2 observations=4096"
Assert-Contains $baselinePrint "OCTESSERA_PI_PROFILE_MEASURE_FRAMES=256"
Assert-Contains $baselinePrint "OCTESSERA_PI_PROFILE_SAMPLE_RATE=44100"
Assert-Contains $baselinePrint "OCTESSERA_SYNTH_SLOT_WORKERS=2"
Assert-Contains $baselinePrint "OCTESSERA_PI_PROFILE_SCENARIO='synth_cross_slot_16'"
Assert-Contains $baselinePrint "--profile-dsp"
Assert-Contains $baselinePrint "safety-abort.txt"
Assert-Contains $baselinePrint "sleep 1"
Assert-Contains $baselinePrint "governor-before.txt"
if ($payloadSource -match "function New-ProfileBaselineBody") { throw "Orange ProfileBaseline payload remains in the general payload module." }
Assert-Contains $baselinePrint "runtime-candidate-sha256.txt"
Assert-Throws { & $runner -Mode ProfileBaseline -Scenario unknown_scenario -EngineBlockFrames 256 -ProfileMeasureFrames 256 -Workers 2 -PrintOnly } "unknown profile ID"
Assert-Throws { & $runner -Mode ProfileBaseline -Scenario synth_cross_slot_16 -EngineBlockFrames 128 -ProfileMeasureFrames 256 -Workers 2 -PrintOnly } "profile geometry"
if ((& $runner -Mode LiveAudioBenchmark -Scenario mixed_16_synth_32_sample -OutputFrames 128 -EngineBlockFrames 32 -Workers 2 -MeasureSeconds 30 -Artifact $baselineArtifact -Metadata "$baselineArtifact.metadata.json" -AllowServiceInterruption -PrintOnly | Out-String) -notmatch "output=128 period=32 engine=32") { throw "Orange 128/32/2 baseline-live geometry was not retained." }
if ($runnerSource -match "with-pi-ssh|run-pi-performance-baseline") { throw "Orange runner references Raspberry tooling." }
if ($driverSource -match "with-pi-ssh|run-pi-timing-probes") { throw "Orange driver references Raspberry tooling." }

$studyStart = $baselinePrint.IndexOf("Study payload:`n", [StringComparison]::Ordinal) + "Study payload:`n".Length
$studyEnd = $baselinePrint.IndexOf("Study payload transport:", $studyStart, [StringComparison]::Ordinal)
if ($studyStart -lt "Study payload:`n".Length -or $studyEnd -le $studyStart) { throw "Orange baseline payload was not published." }
$payloadPath = Join-Path ([IO.Path]::GetTempPath()) ("octessera-orange-baseline-payload-" + [guid]::NewGuid().ToString("N") + ".sh")
try {
  [IO.File]::WriteAllText($payloadPath, $baselinePrint.Substring($studyStart, $studyEnd - $studyStart), (New-Object System.Text.UTF8Encoding($false)))
  $bash = Get-Command bash -ErrorAction SilentlyContinue
  if ($null -ne $bash -and [string]$bash.Source -notmatch "WindowsApps") {
    & bash -n $payloadPath
    if ($LASTEXITCODE -ne 0) { throw "Generated Orange baseline payload failed bash -n." }
  } elseif ($null -ne (Get-Command wsl.exe -ErrorAction SilentlyContinue)) {
    $drive = $payloadPath.Substring(0, 1).ToLowerInvariant()
    $wslPath = "/mnt/$drive" + ($payloadPath.Substring(2) -replace "\\", "/")
    & wsl.exe bash -n $wslPath
    if ($LASTEXITCODE -ne 0) { throw "Generated Orange baseline payload failed WSL bash -n." }
  }
} finally {
  Remove-Item -LiteralPath $payloadPath -Force -ErrorAction SilentlyContinue
}

Write-Output "Orange performance baseline manifest, plan, consent, isolation, continuation, and payload tests passed"
