Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot "performance-baseline-json.psm1") -Force

$script:Phase1Ids = @(
  "baseline_idle",
  "synth_shipped_policy_8",
  "synth_cross_slot_16",
  "sample_8",
  "sample_cross_slot_64",
  "mixed_16_synth_32_sample",
  "fixed_8_synth_8_sample_0_bus_2_global_0_momentary",
  "fixed_8_synth_8_sample_6_bus_2_global_2_momentary",
  "fixed_8_synth_8_sample_12_bus_2_global_0_momentary",
  "fixed_8_synth_8_sample_12_bus_2_global_2_momentary",
  "synth_cross_slot_32_no_steal",
  "synth_cross_slot_64_no_steal"
)
$script:ShippedIds = $script:Phase1Ids | Where-Object { $_ -notlike "*_no_steal" }
$script:BlockIds = @("synth_cross_slot_16", "mixed_16_synth_32_sample", "fixed_8_synth_8_sample_12_bus_2_global_2_momentary")
$script:WorkerIds = @("synth_cross_slot_16", "mixed_16_synth_32_sample", "synth_cross_slot_32_no_steal")
$script:OrangeLiveIds = @("synth_cross_slot_16", "sample_cross_slot_64", "mixed_16_synth_32_sample", "fixed_8_synth_8_sample_12_bus_2_global_2_momentary", "synth_cross_slot_32_no_steal")
$script:CommonCellIds = @("common_baseline_idle", "common_synth_shipped_policy_8", "common_synth_cross_slot_16", "common_sample_8", "common_sample_cross_slot_64", "common_mixed_16_synth_32_sample", "common_fixed_8_0_bus_2_global_0_momentary", "common_fixed_8_6_bus_2_global_2_momentary", "common_fixed_8_12_bus_2_global_0_momentary", "common_fixed_8_12_bus_2_global_2_momentary", "common_synth_cross_slot_32_no_steal", "common_synth_cross_slot_64_no_steal")
$script:OrangeDefaultCellIds = @("orange_default_baseline_idle", "orange_default_synth_shipped_policy_8", "orange_default_synth_cross_slot_16", "orange_default_sample_8", "orange_default_sample_cross_slot_64", "orange_default_mixed_16_synth_32_sample", "orange_default_fixed_8_0_bus_2_global_0_momentary", "orange_default_fixed_8_6_bus_2_global_2_momentary", "orange_default_fixed_8_12_bus_2_global_0_momentary", "orange_default_fixed_8_12_bus_2_global_2_momentary")
$script:BlockCellIds = @("block_synth_cross_slot_16_64", "block_synth_cross_slot_16_128", "block_synth_cross_slot_16_256", "block_mixed_16_synth_32_sample_64", "block_mixed_16_synth_32_sample_128", "block_mixed_16_synth_32_sample_256", "block_max_fx_64", "block_max_fx_128", "block_max_fx_256")
$script:WorkerCellIds = @("workers_synth_cross_slot_16_0", "workers_synth_cross_slot_16_2", "workers_synth_cross_slot_16_3", "workers_mixed_16_synth_32_sample_0", "workers_mixed_16_synth_32_sample_2", "workers_mixed_16_synth_32_sample_3", "workers_synth_cross_slot_32_no_steal_0", "workers_synth_cross_slot_32_no_steal_2", "workers_synth_cross_slot_32_no_steal_3")
$script:OrangeDefaultLiveCellIds = @("orange_live_default_synth_cross_slot_16", "orange_live_default_sample_cross_slot_64", "orange_live_default_mixed_16_synth_32_sample", "orange_live_default_max_fx")
$script:OrangeNeighborCellIds = @("orange_live_neighbor_mixed_128", "orange_live_neighbor_mixed_512", "orange_live_neighbor_max_fx_128", "orange_live_neighbor_max_fx_512")
$script:OrangeWorkerCellIds = @("orange_live_worker_synth32_0", "orange_live_worker_synth32_2", "orange_live_worker_synth32_3", "orange_live_worker_mixed_0", "orange_live_worker_mixed_2", "orange_live_worker_mixed_3")
$script:RaspberryLiveCellIds = @("raspberry_live_output_128", "raspberry_live_output_256", "raspberry_live_output_512")

function Assert-PlanFields {
  param([object]$Value, [string[]]$Fields, [string]$Context)
  if ($null -eq $Value -or $Value -is [array] -or $Value -is [string] -or $Value -is [ValueType]) { throw "$Context must be a JSON object." }
  $actual = @($Value.PSObject.Properties.Name)
  if ($actual.Count -ne $Fields.Count) { throw "$Context must contain exactly: $($Fields -join ', ')." }
  foreach ($field in $Fields) {
    if (-not ($actual -ccontains $field)) { throw "$Context is missing '$field'." }
  }
  foreach ($field in $actual) {
    if (-not ($Fields -ccontains $field)) { throw "$Context contains unexpected field '$field'." }
  }
}

function Assert-PlanInteger {
  param([object]$Value, [string]$Context)
  if ($Value -isnot [int] -and $Value -isnot [long]) { throw "$Context must be an integer." }
}

function Assert-PlanString {
  param([object]$Value, [string]$Context)
  if ($Value -isnot [string] -or [string]::IsNullOrWhiteSpace($Value)) { throw "$Context must be a non-empty string." }
}

function Assert-PerformanceBaselinePath {
  param([Parameter(Mandatory)][string]$Value, [Parameter(Mandatory)][string]$Context)
  if ([string]::IsNullOrWhiteSpace($Value) -or $Value -match '[\x00-\x1f\x7f]' -or $Value.IndexOfAny([char[]]@("'", '"')) -ge 0) { throw "$Context contains a quote or control character." }
}

function Assert-PerformanceBaselineSourceContext {
  param([Parameter(Mandatory)][string]$Head, [AllowEmptyString()][string]$Status = "")
  if ($Head -notmatch '^[0-9a-f]{40}$') { throw "Repository HEAD is not a full reproducible commit identity." }
  if (-not [string]::IsNullOrWhiteSpace($Status)) { throw "Active performance baseline execution requires a clean worktree." }
}

function Assert-PerformanceBaselineArtifactMatch {
  param([Parameter(Mandatory)][string]$LocalHash, [Parameter(Mandatory)][string]$RemoteHash)
  if ($LocalHash -notmatch '^[0-9a-f]{64}$' -or $RemoteHash -notmatch '^[0-9a-f]{64}$' -or $LocalHash -cne $RemoteHash) { throw "Local artifact SHA-256 does not match the remote binary SHA-256." }
}

function Assert-OfflineCell {
  param([object]$Cell, [string]$Context)
  Assert-PlanFields $Cell @("id", "scenario", "internal_frames", "measure_frames", "workers") $Context
  Assert-PlanString $Cell.id "$Context.id"
  if ($Cell.id -notmatch '^[a-z0-9][a-z0-9_]*$') { throw "$Context.id is not a canonical cell ID." }
  if ($script:Phase1Ids -notcontains $Cell.scenario) { throw "$Context has an unknown scenario ID: $($Cell.scenario)." }
  Assert-PlanInteger $Cell.internal_frames "$Context.internal_frames"
  Assert-PlanInteger $Cell.measure_frames "$Context.measure_frames"
  Assert-PlanInteger $Cell.workers "$Context.workers"
  if (@(64, 128, 256) -notcontains $Cell.internal_frames -or $Cell.internal_frames -ne $Cell.measure_frames) { throw "$Context has invalid internal/measure geometry." }
  if (@(0, 2, 3) -notcontains $Cell.workers) { throw "$Context has invalid workers." }
}

function Assert-OrangeLiveCell {
  param([object]$Cell, [string]$Context)
  Assert-PlanFields $Cell @("id", "scenario", "output_frames", "internal_frames", "workers", "measure_seconds") $Context
  Assert-PlanString $Cell.id "$Context.id"
  if ($Cell.id -notmatch '^[a-z0-9][a-z0-9_]*$') { throw "$Context.id is not a canonical cell ID." }
  if ($script:OrangeLiveIds -notcontains $Cell.scenario) { throw "$Context has an unknown Orange live scenario ID: $($Cell.scenario)." }
  foreach ($field in @("output_frames", "internal_frames", "workers", "measure_seconds")) { Assert-PlanInteger $Cell.$field "$Context.$field" }
  $tuple = "$($Cell.output_frames)/$($Cell.internal_frames)/$($Cell.workers)"
  if (@("128/32/2", "256/64/2", "512/128/2", "1024/256/0", "1024/256/2", "1024/256/3") -notcontains $tuple) { throw "$Context has invalid Orange live geometry: $tuple." }
  if ($Cell.measure_seconds -ne 30) { throw "$Context must use a 30-second measurement." }
  if ($Cell.output_frames -eq 1024 -and @("synth_cross_slot_32_no_steal", "mixed_16_synth_32_sample") -notcontains $Cell.scenario) { throw "$Context has an invalid 1024-frame scenario." }
  if ($Cell.output_frames -ne 1024 -and $Cell.scenario -eq "synth_cross_slot_32_no_steal") { throw "$Context has an invalid synth32 live geometry." }
}

function Assert-UniquePlanIds {
  param([object[]]$Cells, [string]$Context)
  $ids = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
  foreach ($cell in $Cells) {
    if (-not $ids.Add([string]$cell.id)) { throw "$Context contains duplicate cell ID: $($cell.id)." }
  }
}

function Assert-PlanScenarioOrder {
  param([object[]]$Cells, [string[]]$Scenarios, [string]$Context)
  $actual = @($Cells | ForEach-Object { [string]$_.scenario })
  if (($actual -join ",") -cne ($Scenarios -join ",")) { throw "$Context scenario order is not explicit and approved." }
}

function Assert-PlanIdOrder {
  param([object[]]$Cells, [string[]]$Ids, [string]$Context)
  $actual = @($Cells | ForEach-Object { [string]$_.id })
  if (($actual -join ",") -cne ($Ids -join ",")) { throw "$Context cell ID order is not explicit and approved." }
}

function Assert-PlanTuple {
  param([object]$Cell, [int]$OutputFrames, [int]$InternalFrames, [int]$MeasureFrames, [int]$Workers, [string]$Context)
  $output = $Cell.PSObject.Properties["output_frames"]
  $measure = $Cell.PSObject.Properties["measure_frames"]
  if (($null -ne $output -and $Cell.output_frames -ne $OutputFrames) -or $Cell.internal_frames -ne $InternalFrames -or ($null -ne $measure -and $Cell.measure_frames -ne $MeasureFrames) -or $Cell.workers -ne $Workers) { throw "$Context has an unexpected geometry tuple." }
}

function Read-PerformanceBaselineManifest {
  param([Parameter(Mandatory)][string]$Path)
  Assert-PerformanceBaselinePath $Path "Performance baseline manifest path"
  $text = Read-StrictUtf8Text $Path "Performance baseline manifest"
  $manifest = ConvertFrom-StrictJsonText $text "Performance baseline manifest"
  Assert-PlanFields $manifest @("schema_version", "study_id", "sample_rate", "warmup_seconds", "offline_observations", "repetitions", "cohorts", "orange", "raspberry") "Performance baseline manifest"
  Assert-PlanInteger $manifest.schema_version "schema_version"
  Assert-PlanInteger $manifest.sample_rate "sample_rate"
  Assert-PlanInteger $manifest.warmup_seconds "warmup_seconds"
  Assert-PlanInteger $manifest.offline_observations "offline_observations"
  Assert-PlanInteger $manifest.repetitions "repetitions"
  Assert-PlanString $manifest.study_id "study_id"
  if ($manifest.schema_version -ne 1 -or $manifest.study_id -cne "cross-board-baseline" -or $manifest.sample_rate -ne 44100 -or $manifest.warmup_seconds -ne 2 -or $manifest.offline_observations -ne 4096 -or $manifest.repetitions -ne 3) { throw "Performance baseline manifest has an unsupported study contract." }

  Assert-PlanFields $manifest.cohorts @("common_reference", "orange_effective_default", "block_sweep", "worker_sweep") "manifest.cohorts"
  $cohortCells = [ordered]@{}
  foreach ($cohortName in @("common_reference", "orange_effective_default", "block_sweep", "worker_sweep")) {
    $cohort = $manifest.cohorts.$cohortName
    $cohortFields = if (@("block_sweep", "worker_sweep") -contains $cohortName) { @("kind", "scenarios", "cells") } else { @("kind", "cells") }
    Assert-PlanFields $cohort $cohortFields "manifest.cohorts.$cohortName"
    if ($cohort.kind -cne "native_profile" -or $cohort.cells -isnot [array]) { throw "manifest.cohorts.$cohortName is not a native profile cohort." }
    $cells = @($cohort.cells)
    foreach ($cell in $cells) { Assert-OfflineCell $cell "manifest.cohorts.$cohortName.$($cell.id)" }
    if (@("block_sweep", "worker_sweep") -contains $cohortName) {
      if ($cohort.scenarios -isnot [array]) { throw "manifest.cohorts.$cohortName.scenarios must be an array." }
      $expectedScenarios = if ($cohortName -eq "block_sweep") { $script:BlockIds } else { $script:WorkerIds }
      if (($cohort.scenarios -join ",") -cne ($expectedScenarios -join ",")) { throw "manifest.cohorts.$cohortName scenario order is invalid." }
    }
    $cohortCells[$cohortName] = $cells
  }
  if ($cohortCells.common_reference.Count -ne 12 -or $cohortCells.orange_effective_default.Count -ne 10 -or $cohortCells.block_sweep.Count -ne 9 -or $cohortCells.worker_sweep.Count -ne 9) { throw "Performance baseline cohort counts do not match the explicit study." }
  Assert-PlanScenarioOrder $cohortCells.common_reference $script:Phase1Ids "common_reference"
  Assert-PlanScenarioOrder $cohortCells.orange_effective_default $script:ShippedIds "orange_effective_default"
  Assert-PlanScenarioOrder $cohortCells.block_sweep (@("synth_cross_slot_16", "synth_cross_slot_16", "synth_cross_slot_16", "mixed_16_synth_32_sample", "mixed_16_synth_32_sample", "mixed_16_synth_32_sample", "fixed_8_synth_8_sample_12_bus_2_global_2_momentary", "fixed_8_synth_8_sample_12_bus_2_global_2_momentary", "fixed_8_synth_8_sample_12_bus_2_global_2_momentary")) "block_sweep"
  Assert-PlanScenarioOrder $cohortCells.worker_sweep (@("synth_cross_slot_16", "synth_cross_slot_16", "synth_cross_slot_16", "mixed_16_synth_32_sample", "mixed_16_synth_32_sample", "mixed_16_synth_32_sample", "synth_cross_slot_32_no_steal", "synth_cross_slot_32_no_steal", "synth_cross_slot_32_no_steal")) "worker_sweep"
  Assert-PlanIdOrder $cohortCells.common_reference $script:CommonCellIds "common_reference"
  Assert-PlanIdOrder $cohortCells.orange_effective_default $script:OrangeDefaultCellIds "orange_effective_default"
  Assert-PlanIdOrder $cohortCells.block_sweep $script:BlockCellIds "block_sweep"
  Assert-PlanIdOrder $cohortCells.worker_sweep $script:WorkerCellIds "worker_sweep"
  foreach ($cell in $cohortCells.common_reference) { Assert-PlanTuple $cell 0 256 256 2 "common_reference.$($cell.id)" }
  foreach ($cell in $cohortCells.orange_effective_default) { Assert-PlanTuple $cell 0 64 64 2 "orange_effective_default.$($cell.id)" }
  for ($index = 0; $index -lt $cohortCells.block_sweep.Count; $index++) { $blockFrames = @(64, 128, 256)[$index % 3]; Assert-PlanTuple $cohortCells.block_sweep[$index] 0 $blockFrames $blockFrames 0 "block_sweep.$($cohortCells.block_sweep[$index].id)" }
  for ($index = 0; $index -lt $cohortCells.worker_sweep.Count; $index++) { Assert-PlanTuple $cohortCells.worker_sweep[$index] 0 256 256 (@(0, 2, 3)[$index % 3]) "worker_sweep.$($cohortCells.worker_sweep[$index].id)" }
  Assert-UniquePlanIds @($cohortCells.Values | ForEach-Object { $_ }) "native cohorts"

  Assert-PlanFields $manifest.orange @("board_profile", "live_defaults", "live_neighbors", "live_workers", "long_repeat") "manifest.orange"
  if ($manifest.orange.board_profile -cne "orange-pi-zero-2w") { throw "manifest.orange has the wrong board profile." }
  foreach ($groupName in @("live_defaults", "live_neighbors", "live_workers")) {
    if ($manifest.orange.$groupName -isnot [array]) { throw "manifest.orange.$groupName must be an array." }
    foreach ($cell in @($manifest.orange.$groupName)) { Assert-OrangeLiveCell $cell "manifest.orange.$groupName.$($cell.id)" }
  }
  if (@($manifest.orange.live_defaults).Count -ne 4 -or @($manifest.orange.live_neighbors).Count -ne 4 -or @($manifest.orange.live_workers).Count -ne 6) { throw "Orange live cohort counts do not match the explicit study." }
  Assert-PlanScenarioOrder @($manifest.orange.live_defaults) @("synth_cross_slot_16", "sample_cross_slot_64", "mixed_16_synth_32_sample", "fixed_8_synth_8_sample_12_bus_2_global_2_momentary") "orange.live_defaults"
  Assert-PlanScenarioOrder @($manifest.orange.live_neighbors) @("mixed_16_synth_32_sample", "mixed_16_synth_32_sample", "fixed_8_synth_8_sample_12_bus_2_global_2_momentary", "fixed_8_synth_8_sample_12_bus_2_global_2_momentary") "orange.live_neighbors"
  Assert-PlanScenarioOrder @($manifest.orange.live_workers) @("synth_cross_slot_32_no_steal", "synth_cross_slot_32_no_steal", "synth_cross_slot_32_no_steal", "mixed_16_synth_32_sample", "mixed_16_synth_32_sample", "mixed_16_synth_32_sample") "orange.live_workers"
  Assert-PlanIdOrder @($manifest.orange.live_defaults) $script:OrangeDefaultLiveCellIds "orange.live_defaults"
  Assert-PlanIdOrder @($manifest.orange.live_neighbors) $script:OrangeNeighborCellIds "orange.live_neighbors"
  Assert-PlanIdOrder @($manifest.orange.live_workers) $script:OrangeWorkerCellIds "orange.live_workers"
  foreach ($cell in @($manifest.orange.live_defaults)) { Assert-PlanTuple $cell 256 64 0 2 "orange.live_defaults.$($cell.id)" }
  foreach ($cell in @($manifest.orange.live_neighbors)) { $neighborIndex = [array]::IndexOf(@($manifest.orange.live_neighbors), $cell); $tuple = if ($neighborIndex % 2 -eq 0) { @(128, 32) } else { @(512, 128) }; Assert-PlanTuple $cell $tuple[0] $tuple[1] 0 2 "orange.live_neighbors.$($cell.id)" }
  foreach ($cell in @($manifest.orange.live_workers)) { Assert-PlanTuple $cell 1024 256 0 (@(0, 2, 3)[[array]::IndexOf(@($manifest.orange.live_workers), $cell) % 3]) "orange.live_workers.$($cell.id)" }
  Assert-UniquePlanIds @($manifest.orange.live_defaults) + @($manifest.orange.live_neighbors) + @($manifest.orange.live_workers) "Orange live cohorts"
  Assert-PlanFields $manifest.orange.long_repeat @("id", "selection", "measure_seconds", "warmup_seconds") "manifest.orange.long_repeat"
  if ($manifest.orange.long_repeat.id -cne "orange_live_worst_passing_default_120" -or $manifest.orange.long_repeat.selection -cne "p99.9_then_max" -or $manifest.orange.long_repeat.measure_seconds -ne 120 -or $manifest.orange.long_repeat.warmup_seconds -ne 5) { throw "Orange long-repeat contract changed." }

  Assert-PlanFields $manifest.raspberry @("board_profile", "live_cells") "manifest.raspberry"
  if ($manifest.raspberry.board_profile -cne "raspberry-pi-zero-2w" -or $manifest.raspberry.live_cells -isnot [array] -or @($manifest.raspberry.live_cells).Count -ne 3) { throw "Raspberry live cohort is incomplete." }
  foreach ($cell in @($manifest.raspberry.live_cells)) {
    Assert-PlanFields $cell @("id", "scenario", "output_frames", "internal_frames", "workers", "duration_seconds", "probe_modes", "callback_fields") "manifest.raspberry.$($cell.id)"
    Assert-PlanString $cell.id "manifest.raspberry.$($cell.id).id"
    if ($cell.id -notmatch '^[a-z0-9][a-z0-9_]*$') { throw "manifest.raspberry.$($cell.id).id is not a canonical cell ID." }
    Assert-PlanString $cell.scenario "manifest.raspberry.$($cell.id).scenario"
    foreach ($field in @("output_frames", "internal_frames", "workers", "duration_seconds")) { Assert-PlanInteger $cell.$field "manifest.raspberry.$($cell.id).$field" }
    if (@(128, 256, 512) -notcontains $cell.output_frames -or $cell.scenario -cne "pulses-stress" -or $cell.internal_frames -ne 256 -or $cell.workers -ne 2 -or $cell.duration_seconds -ne 30 -or $cell.probe_modes -isnot [array] -or ($cell.probe_modes -join ",") -cne "Live,AudioDrain" -or $null -ne $cell.callback_fields) { throw "manifest.raspberry.$($cell.id) does not preserve the fresh nullable-callback probe contract." }
  }
  Assert-PlanIdOrder @($manifest.raspberry.live_cells) $script:RaspberryLiveCellIds "raspberry.live_cells"
  Assert-UniquePlanIds @($manifest.raspberry.live_cells) "Raspberry live cells"
  return $manifest
}

function Get-PerformanceBaselineOfflineCells {
  param([Parameter(Mandatory)][object]$Manifest, [Parameter(Mandatory)][ValidateSet("orange-pi-zero-2w", "raspberry-pi-zero-2w")][string]$BoardProfile)
  $cells = @()
  $cells += @($Manifest.cohorts.common_reference.cells)
  if ($BoardProfile -eq "orange-pi-zero-2w") { $cells += @($Manifest.cohorts.orange_effective_default.cells) }
  $cells += @($Manifest.cohorts.block_sweep.cells)
  $cells += @($Manifest.cohorts.worker_sweep.cells)
  return $cells
}

function Get-PerformanceBaselineOrangeLiveCells {
  param([Parameter(Mandatory)][object]$Manifest)
  $cells = @()
  $cells += @($Manifest.orange.live_defaults)
  $cells += @($Manifest.orange.live_neighbors)
  $cells += @($Manifest.orange.live_workers)
  return $cells
}

function Get-PerformanceBaselineRoundRobinPlan {
  param([Parameter(Mandatory)][object[]]$Cells, [Parameter(Mandatory)][int]$Repetitions)
  if ($Repetitions -lt 1) { throw "Repetitions must be positive." }
  $plan = @()
  for ($repetition = 1; $repetition -le $Repetitions; $repetition++) {
    foreach ($cell in $Cells) {
      $plan += [pscustomobject]@{ Repetition = $repetition; Cell = $cell }
    }
  }
  return $plan
}

function Select-PerformanceBaselineWorstPassingDefault {
  param([Parameter(Mandatory)][object[]]$Results, [Parameter(Mandatory)][string[]]$DefaultCellIds, [Parameter(Mandatory)][int]$Repetitions)
  $eligible = @()
  for ($order = 0; $order -lt $DefaultCellIds.Count; $order++) {
    $cellId = $DefaultCellIds[$order]
    $cellResults = @($Results | Where-Object { $_.CellId -ceq $cellId })
    if ($cellResults.Count -ne $Repetitions) { continue }
    $repetitionIds = @($cellResults | ForEach-Object { [int]$_.Repetition } | Sort-Object)
    $expectedIds = @(1..$Repetitions)
    if (($repetitionIds -join ",") -cne ($expectedIds -join ",")) { continue }
    if (@($cellResults | Where-Object { $_.StatusClass -cne "pass" }).Count -ne 0) { continue }
    $worstP999 = ($cellResults | ForEach-Object { [double]$_.P99_9 } | Measure-Object -Maximum).Maximum
    $worstMax = ($cellResults | ForEach-Object { [double]$_.Max } | Measure-Object -Maximum).Maximum
    $eligible += [pscustomobject]@{ CellId = $cellId; Scenario = [string]$cellResults[0].Scenario; WorstP99_9 = [double]$worstP999; WorstMax = [double]$worstMax; ManifestOrder = $order; Results = $cellResults }
  }
  if ($eligible.Count -eq 0) { throw "No default live cell has exactly $Repetitions passing repetitions." }
  return $eligible | Sort-Object -Property @(@{ Expression = { $_.WorstP99_9 }; Descending = $true }, @{ Expression = { $_.WorstMax }; Descending = $true }, @{ Expression = { $_.ManifestOrder }; Descending = $false }) | Select-Object -First 1
}

function Get-PerformanceBaselineCanaryCells {
  param([Parameter(Mandatory)][object]$Manifest)
  return [pscustomobject]@{
    Offline = @($Manifest.cohorts.common_reference.cells)[0]
    OrangeLive = @($Manifest.orange.live_defaults)[0]
  }
}

function Test-PerformanceBaselineMeasuredOutcome {
  param([Parameter(Mandatory)][string]$StatusClass)
  return @("pass", "over_budget", "measured_failure") -contains $StatusClass
}

Export-ModuleMember -Function @(
  "Get-PerformanceBaselineCanaryCells",
  "Get-PerformanceBaselineOfflineCells",
  "Get-PerformanceBaselineOrangeLiveCells",
  "Get-PerformanceBaselineRoundRobinPlan",
  "Read-PerformanceBaselineManifest",
  "Select-PerformanceBaselineWorstPassingDefault",
  "Assert-PerformanceBaselineArtifactMatch",
  "Assert-PerformanceBaselinePath",
  "Assert-PerformanceBaselineSourceContext",
  "Test-PerformanceBaselineMeasuredOutcome"
)
