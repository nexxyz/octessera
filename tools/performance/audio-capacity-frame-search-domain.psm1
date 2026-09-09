Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot "audio-capacity-frame-search-plan.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "audio-capacity-frame-search-evidence.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "audio-capacity-frame-search-analysis.psm1") -Force

function Get-FrameSearchGradeOrder {
  param([Parameter(Mandatory)][string]$Grade)
  return Get-FrameSearchAnalysisGradeOrder $Grade
}

function Get-FrameSearchWorstIncident {
  param([Parameter(Mandatory)][uint64]$RepeatIncidents, [Parameter(Mandatory)][uint64]$SilentIncidents, [Parameter(Mandatory)][uint64]$AlsaRecoveryLogIncidents, [Parameter(Mandatory)][uint64]$CpalStreamErrors, [Parameter(Mandatory)][uint64]$CpalDeviceErrors)
  return Get-FrameSearchEvidenceWorstIncident $RepeatIncidents $SilentIncidents $AlsaRecoveryLogIncidents $CpalStreamErrors $CpalDeviceErrors
}

function Get-FrameSearchGrade {
  param([Parameter(Mandatory)][uint64]$Worst, [Parameter(Mandatory)][int]$MeasureSeconds)
  return Get-FrameSearchEvidenceGrade $Worst $MeasureSeconds
}

function Get-FrameSearchCellGroup {
  param([Parameter(Mandatory)][object]$Row)
  return "$($Row.Board)/$($Row.Mode)"
}

function Get-FrameSearchStableKey {
  param([Parameter(Mandatory)][string]$Value)
  $hash = [Security.Cryptography.SHA256]::Create()
  try { $bytes = $hash.ComputeHash([Text.Encoding]::UTF8.GetBytes($Value)) } finally { $hash.Dispose() }
  return [Convert]::ToUInt64(([BitConverter]::ToString($bytes[0..7])).Replace("-", ""), 16)
}

function Get-FrameSearchInterlacedOrder {
  param([Parameter(Mandatory)][object[]]$Rows, [Parameter(Mandatory)][int]$Salt)
  $remaining = New-Object 'System.Collections.Generic.List[object]'
  foreach ($row in $Rows) { $remaining.Add($row) | Out-Null }
  $ordered = @(); $lastGroup = ""
  while ($remaining.Count -gt 0) {
    $choices = @($remaining | Where-Object { (Get-FrameSearchCellGroup $_) -cne $lastGroup })
    if ($choices.Count -eq 0) { $choices = @($remaining | ForEach-Object { $_ }) }
    $selected = $choices | Sort-Object @{ Expression = { Get-FrameSearchStableKey "$Salt|$($_.Profile)|$($_.U)|$($_.Cell)" }; Ascending = $true }, @{ Expression = { [string]$_.Cell }; Ascending = $true } | Select-Object -First 1
    $ordered += ,$selected; $remaining.Remove($selected) | Out-Null; $lastGroup = Get-FrameSearchCellGroup $selected
  }
  return $ordered
}

function Test-FrameSearchEpochOrder {
  param([Parameter(Mandatory)][object[]]$Rep1, [Parameter(Mandatory)][object[]]$Rep2)
  if ($Rep1.Count -ne $Rep2.Count -or $Rep1.Count -lt 2) { return $false }
  for ($index = 0; $index -lt $Rep2.Count; $index++) { if ($Rep1[$index].Cell -ceq $Rep2[$index].Cell) { return $false } }
  for ($index = 1; $index -lt $Rep1.Count; $index++) { if ($Rep1[$index - 1].Cell -ceq $Rep1[$index].Cell -or $Rep2[$index - 1].Cell -ceq $Rep2[$index].Cell) { return $false } }
  if ($Rep1.Count -ge 3 -and $Rep1[$Rep1.Count - 1].Cell -ceq $Rep2[0].Cell) { return $false }
  if ($Rep1.Count -lt 6) { return $true }
  $positions = @{}
  for ($index = 0; $index -lt $Rep1.Count; $index++) { $positions[$Rep1[$index].Cell] = $index }
  foreach ($index in 0..($Rep2.Count - 1)) { if ($Rep2.Count + $index - $positions[$Rep2[$index].Cell] -lt 5) { return $false } }
  $firstIds = @($Rep1 | ForEach-Object { $_.Cell }); $secondIds = @($Rep2 | ForEach-Object { $_.Cell }); $reverse = @()
  for ($index = $secondIds.Count - 1; $index -ge 0; $index--) { $reverse += $secondIds[$index] }
  if (($firstIds -join ",") -ceq ($reverse -join ",")) { return $false }
  for ($offset = 0; $offset -lt $firstIds.Count; $offset++) { $rotation = @(); for ($index = 0; $index -lt $firstIds.Count; $index++) { $rotation += $secondIds[($index + $offset) % $secondIds.Count] }; if (($firstIds -join ",") -ceq ($rotation -join ",")) { return $false } }
  return $true
}

function Add-FrameSearchSmallDerangementCandidate {
  param([Parameter(Mandatory)][object[]]$Rows, [Parameter(Mandatory)][object[]]$Rep1, [Parameter(Mandatory)][int]$Index, [Parameter(Mandatory)][AllowEmptyCollection()][object[]]$Current, [Parameter(Mandatory)][AllowEmptyCollection()][System.Collections.Generic.HashSet[string]]$Used, [Parameter(Mandatory)][ref]$Results)
  if ($Index -eq $Rows.Count) {
    $positions = @{}; for ($position = 0; $position -lt $Rep1.Count; $position++) { $positions[$Rep1[$position].Cell] = $position }
    $distances = @(); for ($position = 0; $position -lt $Current.Count; $position++) { $distances += $Rows.Count + $position - $positions[$Current[$position].Cell] }
    $adjacent = if ($Rows.Count -ge 3 -and $Rep1[$Rep1.Count - 1].Cell -ceq $Current[0].Cell) { 1 } else { 0 }
    $Results.Value += ,([pscustomobject][ordered]@{ Order = $Current; MinimumDistance = [int]($distances | Measure-Object -Minimum).Minimum; AdjacentViolations = $adjacent; OrderKey = (($Current | ForEach-Object { $_.Cell }) -join ",") })
    return
  }
  foreach ($row in @($Rows | Sort-Object Cell)) {
    if ($Used.Contains([string]$row.Cell) -or $Rep1[$Index].Cell -ceq $row.Cell) { continue }
    $nextUsed = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
    foreach ($usedCell in $Used) { $nextUsed.Add($usedCell) | Out-Null }
    $nextUsed.Add([string]$row.Cell) | Out-Null
    Add-FrameSearchSmallDerangementCandidate $Rows $Rep1 ($Index + 1) @($Current + $row) $nextUsed $Results
  }
}

function Get-FrameSearchSmallDerangement {
  param([Parameter(Mandatory)][object[]]$Rows, [Parameter(Mandatory)][object[]]$Rep1)
  $results = @(); $used = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
  Add-FrameSearchSmallDerangementCandidate $Rows $Rep1 0 @() $used ([ref]$results)
  $preferred = @($results | Where-Object { $_.AdjacentViolations -eq 0 }); if ($preferred.Count -eq 0) { $preferred = @($results) }
  return ($preferred | Sort-Object @{ Expression = { $_.MinimumDistance }; Descending = $true }, @{ Expression = { $_.OrderKey }; Ascending = $true } | Select-Object -First 1).Order
}

function New-FrameSearchDerangedEpoch {
  param([Parameter(Mandatory)][object[]]$Rows, [Parameter(Mandatory)][object[]]$Rep1)
  if ($Rows.Count -lt 2) { return @() }
  if ($Rows.Count -le 5) { return @(Get-FrameSearchSmallDerangement $Rows $Rep1) }
  for ($salt = 1703; $salt -lt 1903; $salt++) { $candidate = @(Get-FrameSearchInterlacedOrder $Rows $salt); if (Test-FrameSearchEpochOrder $Rep1 $candidate) { return $candidate } }
  throw "Unable to satisfy frame-search epoch interlacing constraints."
}

function New-FrameSearchPhysicalSchedule {
  param([Parameter(Mandatory)][object[]]$PairedRows, [Parameter(Mandatory)][string]$Wave)
  if ($PairedRows.Count -lt 2) { return @() }
  $rep1 = @(Get-FrameSearchInterlacedOrder $PairedRows 1701); $rep2 = @(New-FrameSearchDerangedEpoch $PairedRows $rep1); $quality = if ($PairedRows.Count -ge 6) { "strict" } else { "degraded: fewer than six paired cells" }
  $runs = @(); $order = 0
  foreach ($epoch in @(@($rep1, 1), @($rep2, 2))) {
    foreach ($row in $epoch[0]) {
      $order++
      $runs += ,([pscustomobject][ordered]@{ Run = "$($row.Cell)-rep$($epoch[1])"; Cell = $row.Cell; Rep = [int]$epoch[1]; Epoch = [int]$epoch[1]; Wave = $Wave; Phase = "Preliminary"; Profile = $row.Profile; Board = $row.Board; Mode = $row.Mode; Output = [int]$row.Output; Period = [int]$row.Period; Internal = [int]$row.Internal; Lookahead = [int]$row.Lookahead; Effective = [int]$row.Effective; U = [int]$row.U; Sec = 180; Status = "pending"; ScheduleIndex = $order; SkipReason = ""; OrderQuality = $quality })
    }
  }
  if (@($runs | Where-Object { $_.Run -eq "" }).Count -gt 0) { throw "Frame-search schedule contains an empty run ID." }
  if ($PairedRows.Count -ge 3) { for ($index = 1; $index -lt $runs.Count; $index++) { if ($runs[$index - 1].Cell -ceq $runs[$index].Cell) { throw "Frame-search schedule repeats a cell consecutively." } } }
  return $runs
}

Export-ModuleMember -Function Get-FrameSearchGradeOrder, Get-FrameSearchWorstIncident, Get-FrameSearchGrade, Get-FrameSearchInterlacedOrder, Test-FrameSearchEpochOrder, New-FrameSearchPhysicalSchedule, Get-FrameSearchPairObservation, Get-FrameSearchPairObservations, Get-FrameSearchProfileAnalysis, Get-FrameSearchProfileAnalyses, Get-FrameSearchCampaignReadiness, Get-FrameSearchNondominatedProfiles, Get-FrameSearchSeedSkipReason, Get-FrameSearchReversalIntervals, Get-FrameSearchAdaptivePlan, Get-FrameSearchWinnerSet, Get-FrameSearchGainResults, Assert-FrameSearchHostEvidence
