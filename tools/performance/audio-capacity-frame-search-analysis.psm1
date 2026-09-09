Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot "audio-capacity-frame-search-plan.psm1") -Force

$script:FrameSearchGradeOrder = [ordered]@{ Stable = 0; Stretched = 1; Compromised = 2 }

function Get-FrameSearchAnalysisGradeOrder {
  param([Parameter(Mandatory)][string]$Grade)
  if (-not $script:FrameSearchGradeOrder.Contains($Grade)) { throw "Unknown frame-search grade: $Grade" }
  return [int]$script:FrameSearchGradeOrder[$Grade]
}

function Get-FrameSearchPairObservation {
  param([Parameter(Mandatory)][string]$Cell, [Parameter(Mandatory)][AllowEmptyCollection()][object[]]$Runs)
  $matches = @($Runs | Where-Object { $_.Cell -ceq $Cell -and $_.Status -ceq "completed" -and ($null -eq $_.PSObject.Properties["Phase"] -or $_.Phase -ceq "Preliminary") })
  if ($matches.Count -gt 2) { throw "Cell $Cell has more than two completed repetitions." }
  if ($matches.Count -ne 2) { return [pscustomobject][ordered]@{ Cell = $Cell; StructurallyComplete = $false; Grade = "Unresolved"; GradeOrder = $null; Worst = $null; RepeatIncidents = $null; SilentIncidents = $null; AlsaRecoveryLogIncidents = $null; CpalStreamErrors = $null; CpalDeviceErrors = $null; CallbackOverBudget = $null; P999 = $null; NativeStatus = "incomplete"; NativeStatusRank = 2; Profile = $null; Board = $null; Mode = $null; U = $null; Effective = $null; Adaptive = $false } }
  $grade = $matches | ForEach-Object { $_.Grade } | Sort-Object { Get-FrameSearchAnalysisGradeOrder $_ } -Descending | Select-Object -First 1
  $nativeStatuses = @($matches | ForEach-Object { [string]$_.NativeStatus } | Select-Object -Unique)
  $nativeStatusRank = if ($nativeStatuses.Count -eq 1 -and $nativeStatuses[0] -ceq "pass") { 0 } elseif (@($nativeStatuses | Where-Object { $_ -ceq "fail" }).Count -gt 0) { 2 } else { 1 }
  return [pscustomobject][ordered]@{
    Cell = $Cell; StructurallyComplete = $true; Grade = $grade; GradeOrder = Get-FrameSearchAnalysisGradeOrder $grade
    Worst = [uint64](@($matches | ForEach-Object { [uint64]$_.Worst } | Measure-Object -Maximum).Maximum)
    RepeatIncidents = [uint64](@($matches | ForEach-Object { [uint64]$_.RepeatIncidents } | Measure-Object -Maximum).Maximum)
    SilentIncidents = [uint64](@($matches | ForEach-Object { [uint64]$_.SilentIncidents } | Measure-Object -Maximum).Maximum)
    AlsaRecoveryLogIncidents = [uint64](@($matches | ForEach-Object { [uint64]$_.AlsaRecoveryLogIncidents } | Measure-Object -Maximum).Maximum)
    CpalStreamErrors = [uint64](@($matches | ForEach-Object { [uint64]$_.CpalStreamErrors } | Measure-Object -Maximum).Maximum)
    CpalDeviceErrors = [uint64](@($matches | ForEach-Object { [uint64]$_.CpalDeviceErrors } | Measure-Object -Maximum).Maximum)
    CallbackOverBudget = [uint64](@($matches | ForEach-Object { [uint64]$_.CallbackOverBudget } | Measure-Object -Maximum).Maximum)
    P999 = [double](@($matches | ForEach-Object { [double]$_.P999 } | Measure-Object -Maximum).Maximum)
    NativeStatus = (($nativeStatuses | Sort-Object) -join ","); NativeStatusRank = $nativeStatusRank
    Profile = [string]$matches[0].Profile; Board = [string]$matches[0].Board; Mode = [string]$matches[0].Mode; U = [int]$matches[0].U; Effective = [int]$matches[0].Effective
    Adaptive = @($matches | Where-Object { $_.Wave -like "A*" }).Count -gt 0
  }
}

function Get-FrameSearchPairObservations {
  param([Parameter(Mandatory)][AllowEmptyCollection()][object[]]$Runs)
  $cells = @($Runs | Where-Object { $null -eq $_.PSObject.Properties["Phase"] -or $_.Phase -ceq "Preliminary" } | Select-Object -ExpandProperty Cell -Unique)
  return @($cells | ForEach-Object { Get-FrameSearchPairObservation $_ $Runs })
}

function Get-FrameSearchProfileAnalysis {
  param([Parameter(Mandatory)][object]$Profile, [Parameter(Mandatory)][AllowEmptyCollection()][object[]]$PairObservations, [AllowEmptyCollection()][object[]]$BlockedCells = @(), [switch]$BudgetBlocked)
  $observations = @($PairObservations | Where-Object { $_.Profile -ceq $Profile.Profile -and $_.StructurallyComplete } | Sort-Object U)
  $observedU = New-Object 'System.Collections.Generic.HashSet[int]'
  foreach ($observation in $observations) { $observedU.Add([int]$observation.U) | Out-Null }
  $proposedU = New-Object 'System.Collections.Generic.HashSet[int]'; $probeReasons = @{}
  $addProbe = { param([int]$U, [string]$Reason); if ($U -lt 1 -or $U -gt 42 -or $observedU.Contains($U) -or $proposedU.Contains($U)) { return }; $proposedU.Add($U) | Out-Null; $probeReasons[$U] = $Reason }
  $borders = @(); $unresolvedReasons = @()
  for ($index = 1; $index -lt $observations.Count; $index++) {
    $low = $observations[$index - 1]; $high = $observations[$index]; $gap = [int]$high.U - [int]$low.U
    $kind = if ($low.Grade -ceq "Stable" -and $high.Grade -ceq "Stretched") { "StableToStretched" } elseif ($low.Grade -ceq "Stretched" -and $high.Grade -ceq "Compromised") { "StretchedToCompromised" } elseif ($low.Grade -ceq "Stable" -and $high.Grade -ceq "Compromised") { "DirectStableToCompromised" } elseif ($high.GradeOrder -lt $low.GradeOrder) { "Reversal" } else { "" }
    if ([string]::IsNullOrWhiteSpace($kind)) { continue }
    $resolved = $gap -le 1
    $probeU = $null
    if (-not $resolved) { $probeU = [int][math]::Floor(([int]$low.U + [int]$high.U) / 2); & $addProbe $probeU "resolve $kind between U$($low.U) and U$($high.U)"; $unresolvedReasons += "$kind gap U$($low.U)-U$($high.U) requires midpoint U$probeU" }
    $borders += ,([pscustomobject][ordered]@{ Kind = $kind; LowU = [int]$low.U; HighU = [int]$high.U; FromGrade = $low.Grade; ToGrade = $high.Grade; Gap = $gap; Resolved = $resolved; ProbeU = $probeU })
  }
  if ($observations.Count -gt 0) {
    $top = $observations[$observations.Count - 1]
    if ($top.GradeOrder -lt 2 -and $top.U -lt 42) {
      $unresolvedReasons += "non-Compromised top U$($top.U) is below U42"
      if ($top.U -lt 32) { & $addProbe 32 "upward capacity probe after U$($top.U)" } elseif ($top.U -eq 32) { & $addProbe 36 "U32 non-Compromised upward probe"; & $addProbe 42 "U32 non-Compromised upward probe" } else { & $addProbe 42 "upward capacity probe after U$($top.U)" }
    }
    $low = $observations[0]
    if ($low.GradeOrder -gt 0 -and $low.U -gt 1) { $downwardU = [int][math]::Floor(($low.U + 1) / 2); $unresolvedReasons += "non-Stable low U$($low.U) is above U1"; & $addProbe $downwardU "low-end downward probe" }
  }
  if ($observations.Count -lt 2) { $unresolvedReasons += "fewer than two structurally complete observations" }
  $blocked = New-Object 'System.Collections.Generic.HashSet[string]'
  foreach ($cell in @($BlockedCells)) { if ($cell.PSObject.Properties["Cell"]) { $blocked.Add([string]$cell.Cell) | Out-Null } else { $blocked.Add([string]$cell) | Out-Null } }
  $blockedProbeCount = 0
  foreach ($u in @($proposedU)) { if ($blocked.Contains("$($Profile.Profile)-U$u")) { $blockedProbeCount++; $unresolvedReasons += "required probe $($Profile.Profile)-U$u is deferred or skipped" } }
  if ($BudgetBlocked -and $proposedU.Count -gt 0) { $unresolvedReasons += "required probes were suppressed by the adaptive budget" }
  $resolution = "inconclusive"
  if ($observations.Count -ge 2 -and $unresolvedReasons.Count -eq 0) { $resolution = if ($observations[$observations.Count - 1].GradeOrder -lt 2 -and $observations[$observations.Count - 1].U -eq 42) { "bounded_at_U42" } else { "resolved" } }
  $reversals = @($borders | Where-Object { $_.Kind -ceq "Reversal" })
  return [pscustomobject][ordered]@{ Profile = $Profile.Profile; Board = $Profile.Board; Mode = $Profile.Mode; Observations = $observations; Borders = $borders; Reversals = $reversals; RequiredProbes = @($proposedU | Sort-Object | ForEach-Object { [pscustomobject][ordered]@{ U = [int]$_; Cell = "$($Profile.Profile)-U$_"; Reason = $probeReasons[$_] } }); UnresolvedReasons = $unresolvedReasons; BlockedProbeCount = $blockedProbeCount; BudgetBlocked = [bool]$BudgetBlocked; Resolution = $resolution; TopU = if ($observations.Count -gt 0) { [int]$observations[$observations.Count - 1].U } else { $null }; TopGrade = if ($observations.Count -gt 0) { $observations[$observations.Count - 1].Grade } else { "Unresolved" } }
}

function Get-FrameSearchProfileAnalyses {
  param([Parameter(Mandatory)][object[]]$CandidateProfiles, [Parameter(Mandatory)][AllowEmptyCollection()][object[]]$PairObservations, [AllowEmptyCollection()][object[]]$BlockedCells = @(), [AllowEmptyCollection()][string[]]$BudgetBlockedProfiles = @())
  return @($CandidateProfiles | ForEach-Object { Get-FrameSearchProfileAnalysis $_ $PairObservations $BlockedCells -BudgetBlocked:($BudgetBlockedProfiles -ccontains $_.Profile) })
}

function Get-FrameSearchCampaignReadiness {
  param([Parameter(Mandatory)][AllowEmptyCollection()][object[]]$ProfileAnalyses, [Parameter(Mandatory)][AllowEmptyCollection()][object[]]$DeferredRows)
  $unresolved = @($ProfileAnalyses | Where-Object { $_.Resolution -ceq "inconclusive" })
  $deferred = @($DeferredRows)
  return [pscustomobject][ordered]@{ Ready = $deferred.Count -eq 0 -and $unresolved.Count -eq 0; DeferredCount = $deferred.Count; UnresolvedProfiles = @($unresolved | ForEach-Object { $_.Profile }); Reason = if ($deferred.Count -gt 0) { "singleton or required work remains deferred" } elseif ($unresolved.Count -gt 0) { "required profile analysis work remains unresolved" } else { "" } }
}

function Test-FrameSearchProfileDominates {
  param([Parameter(Mandatory)][object]$Left, [Parameter(Mandatory)][object]$Right)
  $commonU = @($Left.SharedObservations | ForEach-Object { $_.U } | Where-Object { @($Right.SharedObservations | ForEach-Object { $_.U }) -contains $_ })
  if ($commonU.Count -eq 0) { return $false }
  $notWorse = $Left.Effective -le $Right.Effective
  $strict = $Left.Effective -lt $Right.Effective
  foreach ($u in $commonU) {
    $leftObservation = @($Left.SharedObservations | Where-Object { $_.U -eq $u })[0]
    $rightObservation = @($Right.SharedObservations | Where-Object { $_.U -eq $u })[0]
    if ($leftObservation.GradeOrder -gt $rightObservation.GradeOrder -or $leftObservation.Worst -gt $rightObservation.Worst) { return $false }
    if ($leftObservation.GradeOrder -lt $rightObservation.GradeOrder -or $leftObservation.Worst -lt $rightObservation.Worst) { $strict = $true }
  }
  return $notWorse -and $strict
}

function Get-FrameSearchNondominatedProfiles {
  param([Parameter(Mandatory)][object[]]$CandidateProfiles, [Parameter(Mandatory)][AllowEmptyCollection()][object[]]$PairObservations, [Parameter(Mandatory)][string]$Board, [Parameter(Mandatory)][string]$Mode)
  $summaries = @()
  foreach ($profile in @($CandidateProfiles | Where-Object { $_.Board -ceq $Board -and $_.Mode -ceq $Mode })) {
    $observations = @($PairObservations | Where-Object { $_.Profile -ceq $profile.Profile -and $_.StructurallyComplete })
    $accepted = @($observations | Where-Object { $_.GradeOrder -le 1 })
    $acceptedU = if ($accepted.Count -gt 0) { [int]($accepted | Measure-Object -Property U -Maximum).Maximum } else { 0 }
    $worst = if ($observations.Count -gt 0) { [uint64]($observations | Measure-Object -Property Worst -Minimum).Minimum } else { [uint64]::MaxValue }
    $summaries += ,([pscustomobject][ordered]@{ Profile = $profile.Profile; HasObservation = $observations.Count -gt 0; AcceptedU = $acceptedU; Effective = [int]$profile.Effective; Worst = $worst; Definition = $profile; SharedObservations = $observations })
  }
  $dominated = @()
  foreach ($summary in $summaries) { if (@($summaries | Where-Object { $_.Profile -cne $summary.Profile -and (Test-FrameSearchProfileDominates $_ $summary) }).Count -gt 0) { $dominated += ,$summary } }
  $undominated = @()
  foreach ($summary in $summaries) { if (@($dominated | Where-Object { $_.Profile -ceq $summary.Profile }).Count -eq 0) { $undominated += $summary } }
  $selected = @($undominated | Sort-Object @{ Expression = { $_.HasObservation }; Descending = $true }, @{ Expression = { $_.AcceptedU }; Descending = $true }, @{ Expression = { $_.Effective }; Ascending = $true } | Select-Object -First 2)
  return [pscustomobject][ordered]@{ Selected = $selected; Dominated = $dominated; Summaries = $summaries }
}

function Get-FrameSearchSeedSkipReason {
  param([Parameter(Mandatory)][object]$Row, [Parameter(Mandatory)][AllowEmptyCollection()][object[]]$Runs, [Parameter(Mandatory)][AllowEmptyCollection()][object[]]$PairObservations)
  if ($Row.Wave -ceq "W3") {
    if (@($Runs | Where-Object { $_.Profile -ceq $Row.Profile -and $_.Status -ceq "failed" }).Count -gt 0) { return "skip if profile dominated/unsafe: prior fatal or unsafe result" }
    return ""
  }
  if ($Row.Wave -ne "W4") { return "" }
  $profile = Get-FrameSearchProfileDefinition $Row.Profile
  $lower = @((Get-FrameSearchProfileDefinitions | Where-Object { $_.Board -ceq $profile.Board -and $_.Mode -ceq $profile.Mode -and $_.Effective -lt $profile.Effective }))
  foreach ($candidate in $lower) {
    $highSummary = [pscustomobject]@{ Effective = $profile.Effective; SharedObservations = @($PairObservations | Where-Object { $_.Profile -ceq $profile.Profile -and $_.StructurallyComplete }) }
    $lowSummary = [pscustomobject]@{ Effective = $candidate.Effective; SharedObservations = @($PairObservations | Where-Object { $_.Profile -ceq $candidate.Profile -and $_.StructurallyComplete }) }
    if (Test-FrameSearchProfileDominates $lowSummary $highSummary) { return "skip if profile dominated/unsafe: higher-latency profile is dominated across shared U observations" }
  }
  return ""
}

function Get-FrameSearchReversalIntervals {
  param([Parameter(Mandatory)][AllowEmptyCollection()][object[]]$Observations)
  $reversals = @()
  foreach ($profile in @($Observations | Where-Object { $_.StructurallyComplete } | Select-Object -ExpandProperty Profile -Unique)) {
    $ordered = @($Observations | Where-Object { $_.Profile -ceq $profile -and $_.StructurallyComplete } | Sort-Object U)
    for ($index = 1; $index -lt $ordered.Count; $index++) {
      $low = $ordered[$index - 1]; $high = $ordered[$index]
      if ($high.GradeOrder -lt $low.GradeOrder) { $reversals += ,([pscustomobject][ordered]@{ Profile = $profile; LowU = [int]$low.U; HighU = [int]$high.U; Gap = [int]$high.U - [int]$low.U; FromGrade = $low.Grade; ToGrade = $high.Grade; Reason = "higher-U grade improved from $($low.Grade) to $($high.Grade)" }) }
    }
  }
  return $reversals
}

function Get-FrameSearchAdaptivePlan {
  param([Parameter(Mandatory)][object[]]$CandidateProfiles, [Parameter(Mandatory)][AllowEmptyCollection()][object[]]$PairObservations, [int]$InitialAdaptivePerProfile = 4, [int]$MaxAdaptivePerProfile = 8, [int]$MaxAdaptiveTotal = 32, [AllowEmptyCollection()][object[]]$BlockedCells = @())
  $rows = @(); $decisions = @(); $analyses = @(); $reversalIntervals = @(Get-FrameSearchReversalIntervals $PairObservations); $globalAdaptiveCount = @($PairObservations | Where-Object { $_.Adaptive }).Count; $entries = @(); $blockedCellIds = @($BlockedCells | ForEach-Object { if ($_.PSObject.Properties["Cell"]) { [string]$_.Cell } else { [string]$_ } })
  foreach ($group in @(@("Raspberry", "Inline"), @("Raspberry", "Multicore"), @("Orange", "Inline"), @("Orange", "Multicore"))) {
    $selection = Get-FrameSearchNondominatedProfiles $CandidateProfiles $PairObservations $group[0] $group[1]
    foreach ($skipped in $selection.Dominated) { $decisions += ,([pscustomobject][ordered]@{ Kind = "skip"; Profile = $skipped.Profile; Reason = "profile dominated after seed waves" }) }
    foreach ($profileSummary in $selection.Selected) {
      $profile = $profileSummary.Definition
      $analysis = Get-FrameSearchProfileAnalysis $profile $PairObservations $BlockedCells
      $adaptiveCount = @($PairObservations | Where-Object { $_.Profile -ceq $profile.Profile -and $_.Adaptive }).Count
      $entries += ,([pscustomobject]@{ Profile = $profile; Analysis = $analysis; AdaptiveCount = $adaptiveCount; Required = @($analysis.RequiredProbes | Where-Object { $blockedCellIds -notcontains $_.Cell }); Proposed = @() })
    }
  }
  foreach ($pass in @(1, 2)) {
    do {
      $allocated = $false
      foreach ($entry in $entries) {
        $target = if ($pass -eq 1) { $InitialAdaptivePerProfile } else { $MaxAdaptivePerProfile }
        if ($globalAdaptiveCount -ge $MaxAdaptiveTotal -or $entry.AdaptiveCount + $entry.Proposed.Count -ge $target) { continue }
        $available = @($entry.Required | Where-Object { @($entry.Proposed | ForEach-Object { $_.Cell }) -notcontains $_.Cell })
        if ($available.Count -eq 0) { continue }
        $probe = $available[0]
        $entry.Proposed += ,$probe; $globalAdaptiveCount++; $allocated = $true
      }
    } while ($allocated -and $globalAdaptiveCount -lt $MaxAdaptiveTotal)
  }
  foreach ($entry in $entries) {
      $profile = $entry.Profile; $analysis = $entry.Analysis; $proposed = @($entry.Proposed); $requiredCount = $entry.Required.Count
      if ($proposed.Count -lt $requiredCount) { $analysis.BudgetBlocked = $true; $analysis.Resolution = "inconclusive"; $analysis.UnresolvedReasons += "required probes were suppressed by the adaptive budget" }
      $analyses += ,$analysis
      foreach ($border in @($analysis.Borders)) { $decisions += ,([pscustomobject][ordered]@{ Kind = "border"; Profile = $profile.Profile; Transition = "$($border.FromGrade)->$($border.ToGrade)"; Reason = "$($border.Kind) U$($border.LowU)-U$($border.HighU)" }) }
      foreach ($reversal in @($analysis.Reversals)) { $decisions += ,([pscustomobject][ordered]@{ Kind = "reversal"; Profile = $profile.Profile; Reason = "Reversal U$($reversal.LowU)-U$($reversal.HighU)" }) }
      foreach ($probe in $proposed) { $rows += ,([pscustomobject][ordered]@{ Cell = $probe.Cell; Wave = "A1"; Profile = $profile.Profile; Board = $profile.Board; Mode = $profile.Mode; Output = $profile.Output; Period = $profile.Period; Internal = $profile.Internal; Lookahead = $profile.Lookahead; Effective = $profile.Effective; U = [int]$probe.U; Sec = 180; Reps = 2; State = "pending"; Condition = "adaptive"; Reason = $probe.Reason }) }
  }
  return [pscustomobject][ordered]@{ Rows = $rows; Decisions = $decisions; Reversals = $reversalIntervals; Analyses = $analyses; GlobalAdaptiveCount = $globalAdaptiveCount }
}

function Get-FrameSearchWinnerSet {
  param([Parameter(Mandatory)][object[]]$CandidateProfiles, [Parameter(Mandatory)][AllowEmptyCollection()][object[]]$PairObservations, [AllowEmptyCollection()][object[]]$ProfileAnalyses = @())
  if ($ProfileAnalyses.Count -eq 0) { $ProfileAnalyses = @(Get-FrameSearchProfileAnalyses $CandidateProfiles $PairObservations) }
  $winners = @()
  foreach ($group in @(@("Raspberry", "Inline"), @("Raspberry", "Multicore"), @("Orange", "Inline"), @("Orange", "Multicore"))) {
    $candidates = @()
    foreach ($profile in @($CandidateProfiles | Where-Object { $_.Board -ceq $group[0] -and $_.Mode -ceq $group[1] })) {
      $analysisMatches = @($ProfileAnalyses | Where-Object { $_.Profile -ceq $profile.Profile })
      if ($analysisMatches.Count -ne 1) { continue }
      $analysis = $analysisMatches[0]
      if (@("resolved", "bounded_at_U42") -cnotcontains $analysis.Resolution) { continue }
      $observations = @($PairObservations | Where-Object { $_.Profile -ceq $profile.Profile -and $_.StructurallyComplete -and $_.GradeOrder -le 1 } | Sort-Object U -Descending)
      foreach ($observation in $observations) {
        $compromisedBelow = @($PairObservations | Where-Object { $_.Profile -ceq $profile.Profile -and $_.StructurallyComplete -and $_.U -lt $observation.U -and $_.GradeOrder -eq 2 }).Count -gt 0
        $reversalBlocked = @($analysis.Borders | Where-Object { $_.Kind -ceq "Reversal" -and -not $_.Resolved -and $_.HighU -ge $observation.U }).Count -gt 0
        if ($compromisedBelow -or $reversalBlocked) { continue }
        $nativeRank = if ($observation.PSObject.Properties["NativeStatusRank"]) { [int]$observation.NativeStatusRank } elseif ([string]$observation.NativeStatus -ceq "pass") { 0 } else { 2 }
        $candidates += ,([pscustomobject][ordered]@{ Board = $profile.Board; Mode = $profile.Mode; Profile = $profile.Profile; U = [int]$observation.U; Grade = $observation.Grade; Worst = [uint64]$observation.Worst; Effective = [int]$profile.Effective; CallbackOverBudget = [uint64]$observation.CallbackOverBudget; P999 = [double]$observation.P999; NativeStatus = [string]$observation.NativeStatus; NativeStatusRank = $nativeRank; HasUnresolvedReversal = $false; Geometry = "output=$($profile.Output),period=$($profile.Period),internal=$($profile.Internal),lookahead=$($profile.Lookahead),effective=$($profile.Effective)" }); break
      }
    }
    if ($candidates.Count -eq 0) { continue }
    $winners += ,($candidates | Sort-Object @{ Expression = { $_.U }; Descending = $true }, @{ Expression = { $_.Effective }; Ascending = $true }, @{ Expression = { $_.Worst }; Ascending = $true }, @{ Expression = { $_.CallbackOverBudget }; Ascending = $true }, @{ Expression = { $_.P999 }; Ascending = $true }, @{ Expression = { $_.NativeStatusRank }; Ascending = $true } | Select-Object -First 1)
  }
  return $winners
}

function Get-FrameSearchGainResults {
  param([Parameter(Mandatory)][object[]]$Winners)
  $results = @()
  foreach ($board in @("Raspberry", "Orange")) {
    $inline = @($Winners | Where-Object { $_.Board -ceq $board -and $_.Mode -ceq "Inline" })[0]; $multicore = @($Winners | Where-Object { $_.Board -ceq $board -and $_.Mode -ceq "Multicore" })[0]; $delta = [int]$multicore.U - [int]$inline.U
    $results += ,([pscustomobject][ordered]@{ Board = $board; InlineU = [int]$inline.U; MulticoreU = [int]$multicore.U; DeltaU = $delta; GainPercent = if ($inline.U -eq 0) { $null } else { 100.0 * $delta / $inline.U }; SynthDelta3x = 3 * $delta; SampleDelta1x = $delta; InlineLatencyMs = [double]$inline.Effective / 44.1; MulticoreLatencyMs = [double]$multicore.Effective / 44.1 })
  }
  return $results
}

Export-ModuleMember -Function Get-FrameSearchAnalysisGradeOrder, Get-FrameSearchPairObservation, Get-FrameSearchPairObservations, Get-FrameSearchProfileAnalysis, Get-FrameSearchProfileAnalyses, Get-FrameSearchCampaignReadiness, Get-FrameSearchNondominatedProfiles, Get-FrameSearchSeedSkipReason, Get-FrameSearchReversalIntervals, Get-FrameSearchAdaptivePlan, Get-FrameSearchWinnerSet, Get-FrameSearchGainResults
