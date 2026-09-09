Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot "audio-capacity-frame-search-plan.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "audio-capacity-frame-search-domain.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "..\pi\raspberry-live-benchmark-metadata.psm1") -Force
Import-Module (Join-Path $PSScriptRoot "..\orange-pi\orange-cross-metadata.psm1") -Force

$script:FrameSearchRunnerVersion = "audio-capacity-frame-search/1.2.0"
$script:FrameSearchStateSchema = 3
$script:FrameSearchDefaultWallHours = 20
$script:FrameSearchReserveMinutes = 75

function Get-FrameSearchRunnerVersion { return $script:FrameSearchRunnerVersion }
function Get-FrameSearchStateSchema { return $script:FrameSearchStateSchema }

function Get-FrameSearchRepoCommit {
  param([Parameter(Mandatory)][string]$RepoRoot)
  $commit = (& git -C $RepoRoot rev-parse --verify HEAD 2>$null | Out-String).Trim().ToLowerInvariant()
  if ($LASTEXITCODE -ne 0 -or $commit -notmatch '^[0-9a-f]{40}$') { throw "Repository HEAD is not a reproducible full commit identity." }
  return $commit
}

function Get-FrameSearchArtifactDefinitions {
  param([string]$RaspberryArtifact = "", [string]$RaspberryMetadata = "", [string]$OrangeInlineArtifact = "", [string]$OrangeInlineMetadata = "", [string]$OrangeMulticoreArtifact = "", [string]$OrangeMulticoreMetadata = "")
  return @(
    [pscustomobject][ordered]@{ Name = "Raspberry"; Board = "Raspberry"; Mode = "Inline+Multicore"; ArtifactPath = $RaspberryArtifact; MetadataPath = if ($RaspberryMetadata) { $RaspberryMetadata } elseif ($RaspberryArtifact) { "$RaspberryArtifact.metadata.json" } else { "" }; BoardProfile = "raspberry-pi-zero-2w"; CargoFeature = "hardware-raspberry-pi-zero-2w routing-tree-benchmark benchmark-voice-pools-128" },
    [pscustomobject][ordered]@{ Name = "OrangeInline"; Board = "Orange"; Mode = "Inline"; ArtifactPath = $OrangeInlineArtifact; MetadataPath = if ($OrangeInlineMetadata) { $OrangeInlineMetadata } elseif ($OrangeInlineArtifact) { "$OrangeInlineArtifact.metadata.json" } else { "" }; BoardProfile = "orange-pi-zero-2w"; CargoFeature = "hardware-orange-pi-zero-2w benchmark-voice-pools-128" },
    [pscustomobject][ordered]@{ Name = "OrangeMulticore"; Board = "Orange"; Mode = "Multicore"; ArtifactPath = $OrangeMulticoreArtifact; MetadataPath = if ($OrangeMulticoreMetadata) { $OrangeMulticoreMetadata } elseif ($OrangeMulticoreArtifact) { "$OrangeMulticoreArtifact.metadata.json" } else { "" }; BoardProfile = "orange-pi-zero-2w"; CargoFeature = "hardware-orange-pi-zero-2w routing-tree-benchmark benchmark-voice-pools-128" }
  )
}

function Get-FrameSearchArtifactSet {
  param([Parameter(Mandatory)][object[]]$Definitions, [Parameter(Mandatory)][string]$SourceCommit, [switch]$RequireFiles)
  if ($Definitions.Count -ne 3) { throw "Frame-search artifact set must contain exactly three artifacts." }
  $artifacts = @()
  foreach ($definition in $Definitions) {
    if (-not $RequireFiles) {
      $artifacts += ,([pscustomobject][ordered]@{ Name = $definition.Name; Board = $definition.Board; Mode = $definition.Mode; ArtifactPath = [string]$definition.ArtifactPath; MetadataPath = [string]$definition.MetadataPath; Sha256 = ""; MetadataSourceCommit = ""; BoardProfile = $definition.BoardProfile; CargoFeature = $definition.CargoFeature; Resolved = $false })
      continue
    }
    if ([string]::IsNullOrWhiteSpace($definition.ArtifactPath) -or [string]::IsNullOrWhiteSpace($definition.MetadataPath)) { throw "Exact artifact and metadata paths are required for $($definition.Name)." }
    if (-not (Test-Path -LiteralPath $definition.ArtifactPath -PathType Leaf)) { throw "Frame-search artifact was not found: $($definition.ArtifactPath)" }
    if (-not (Test-Path -LiteralPath $definition.MetadataPath -PathType Leaf)) { throw "Frame-search artifact metadata was not found: $($definition.MetadataPath)" }
    if ($definition.Name -ceq "Raspberry") {
      $metadata = Read-RaspberryLiveBenchmarkMetadata $definition.MetadataPath
      Assert-RaspberryLiveBenchmarkMetadata -Metadata $metadata -SourceCommit $SourceCommit -BinaryPath $definition.ArtifactPath | Out-Null
      if ($metadata.cargo_feature -cne $definition.CargoFeature) { throw "Raspberry artifact cargo feature is not exact." }
      $source = [string]$metadata.source_commit
      $hash = (Get-FileHash -LiteralPath $definition.ArtifactPath -Algorithm SHA256).Hash.ToLowerInvariant()
    } else {
      $buildSpec = [pscustomobject]@{ Package = "octessera-pi"; Feature = $definition.CargoFeature; ArtifactKind = "diagnostic-only" }
      Assert-OrangeBuildMetadata -MetadataPath $definition.MetadataPath -BinaryPath $definition.ArtifactPath -SelectedBinary "octessera-pi" -SelectedTarget "aarch64-unknown-linux-gnu" -SelectedProfile "release" -BuildSpec $buildSpec -SourceCommit $SourceCommit
      $metadata = Read-OrangeBuildMetadata $definition.MetadataPath
      if ([string]$metadata.cargo_feature -cne $definition.CargoFeature) { throw "$($definition.Name) artifact cargo feature is not exact." }
      $source = [string]$metadata.source_commit
      $hash = (Get-FileHash -LiteralPath $definition.ArtifactPath -Algorithm SHA256).Hash.ToLowerInvariant()
    }
    if ($hash -notmatch '^[0-9a-f]{64}$' -or $source -notmatch '^[0-9a-f]{40}$') { throw "$($definition.Name) artifact identity is not canonical." }
    $artifacts += ,([pscustomobject][ordered]@{ Name = $definition.Name; Board = $definition.Board; Mode = $definition.Mode; ArtifactPath = (Resolve-Path -LiteralPath $definition.ArtifactPath).Path; MetadataPath = (Resolve-Path -LiteralPath $definition.MetadataPath).Path; Sha256 = $hash; MetadataSourceCommit = $source; BoardProfile = $definition.BoardProfile; CargoFeature = $definition.CargoFeature; Resolved = $true })
  }
  return $artifacts
}

function Assert-FrameSearchArtifactIdentity {
  param([Parameter(Mandatory)][object[]]$ExpectedArtifacts, [Parameter(Mandatory)][object[]]$ActualArtifacts)
  if ($ExpectedArtifacts.Count -ne 3 -or $ActualArtifacts.Count -ne 3) { throw "Frame-search artifact identity must contain exactly three artifacts." }
  foreach ($expectedArtifact in $ExpectedArtifacts) {
    $actualMatches = @($ActualArtifacts | Where-Object { $_.Name -ceq $expectedArtifact.Name })
    if ($actualMatches.Count -ne 1) { throw "Frame-search artifact identity changed for $($expectedArtifact.Name): matched $($actualMatches.Count) artifacts." }
    $differences = @()
    foreach ($field in @("ArtifactPath", "MetadataPath", "Sha256", "MetadataSourceCommit", "CargoFeature")) { if ([string]$actualMatches[0].$field -cne [string]$expectedArtifact.$field) { $differences += "$field expected='$($expectedArtifact.$field)' actual='$($actualMatches[0].$field)'" } }
    if ($differences.Count -gt 0) { throw "Frame-search artifact identity changed for $($expectedArtifact.Name): $($differences -join '; ')" }
  }
}

function Write-FrameSearchAtomicJson {
  param([Parameter(Mandatory)][string]$Path, [Parameter(Mandatory)][object]$Value)
  $directory = Split-Path -Parent $Path
  New-Item -ItemType Directory -Force -Path $directory | Out-Null
  $temporary = "$Path.tmp-$PID-$([guid]::NewGuid().ToString('N'))"
  $backup = "$Path.bak-$PID-$([guid]::NewGuid().ToString('N'))"
  $encoding = New-Object Text.UTF8Encoding($false)
  try {
    [IO.File]::WriteAllText($temporary, (($Value | ConvertTo-Json -Depth 30) + "`n"), $encoding)
    if (Test-Path -LiteralPath $Path -PathType Leaf) { [IO.File]::Replace($temporary, $Path, $backup, $true) } else { Move-Item -LiteralPath $temporary -Destination $Path }
  } finally { Remove-Item -LiteralPath $temporary, $backup -Force -ErrorAction SilentlyContinue }
}

function Read-FrameSearchState {
  param([Parameter(Mandatory)][string]$Path)
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Frame-search state was not found: $Path" }
  try { return Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json } catch { throw "Frame-search state is not valid JSON: $Path" }
}

function Enter-FrameSearchStudyLock {
  param([Parameter(Mandatory)][string]$StudyDirectory)
  New-Item -ItemType Directory -Force -Path $StudyDirectory | Out-Null
  $path = Join-Path $StudyDirectory "worker.lock"
  try {
    $stream = [IO.File]::Open($path, [IO.FileMode]::OpenOrCreate, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
  } catch {
    throw "Frame-search study is already owned by another worker: $StudyDirectory"
  }
  return [pscustomobject][ordered]@{ Path = $path; Stream = $stream }
}

function Exit-FrameSearchStudyLock {
  param([AllowNull()][object]$Lock)
  if ($null -ne $Lock -and $null -ne $Lock.Stream) { $Lock.Stream.Dispose() }
}

function Clear-FrameSearchPublicationState {
  param([Parameter(Mandatory)][object]$State)
  $State.provisional_winners = @()
  $State.winners = @()
  $State.gains = @()
  return $State
}

function New-FrameSearchState {
  param([Parameter(Mandatory)][string]$StudyId, [Parameter(Mandatory)][object]$Plan, [Parameter(Mandatory)][string]$SourceCommit, [Parameter(Mandatory)][object[]]$Artifacts, [datetime]$StartUtc = [datetime]::UtcNow, [int]$WallClockHours = 20)
  if ($WallClockHours -lt 1) { throw "Wall-clock limit must be positive." }
  $start = $StartUtc.ToUniversalTime()
  return [pscustomobject][ordered]@{
    schema_version = $script:FrameSearchStateSchema; runner_version = $script:FrameSearchRunnerVersion; study_id = $StudyId; plan_path = $Plan.Path; plan_sha256 = $Plan.Sha256; source_commit = $SourceCommit
    artifacts = @($Artifacts); started_utc = $start.ToString("o"); deadline_utc = $start.AddHours($WallClockHours).ToString("o"); wall_clock_max_hours = $WallClockHours; reserve_minutes = 75
    status = "running"; phase = "Preliminary"; runs = @(); adaptive_rows = @(); deferred_rows = @(); adaptive_reserve_reached = $false; analyses = @(); decisions = @(); provisional_winners = @(); winners = @(); gains = @(); soaks = @()
  }
}

function Assert-FrameSearchResumeIdentity {
  param([Parameter(Mandatory)][object]$State, [Parameter(Mandatory)][string]$StudyId, [Parameter(Mandatory)][object]$Plan, [Parameter(Mandatory)][string]$SourceCommit, [Parameter(Mandatory)][object[]]$Artifacts)
  if ([int]$State.schema_version -ne $script:FrameSearchStateSchema -or [string]$State.runner_version -cne $script:FrameSearchRunnerVersion -or [string]$State.study_id -cne $StudyId -or [string]$State.plan_path -cne $Plan.Path -or [string]$State.plan_sha256 -cne $Plan.Sha256 -or [string]$State.source_commit -cne $SourceCommit) { throw "Frame-search resume identity does not match the current plan, source, study, or runner schema." }
  Assert-FrameSearchArtifactIdentity -ExpectedArtifacts ([object[]]@($State.artifacts)) -ActualArtifacts ([object[]]$Artifacts)
}

function Reconcile-FrameSearchState {
  param([Parameter(Mandatory)][object]$State, [Parameter(Mandatory)][object[]]$Artifacts)
  $recovery = $false
  foreach ($run in @($State.runs)) {
    if ($run.Status -ceq "running") {
      $run.Status = "recovery_required"; $run.StructurallyComplete = $false; $run.Failure = "A prior process left this run running without a completed evidence decision."; $recovery = $true
      continue
    }
    if ($run.Status -ne "completed") { continue }
    try {
      $artifactName = if ($run.Board -ceq "Raspberry") { "Raspberry" } elseif ($run.Mode -ceq "Inline") { "OrangeInline" } else { "OrangeMulticore" }
      $artifactMatches = @($Artifacts | Where-Object { $_.Name -ceq $artifactName })
      if ($artifactMatches.Count -ne 1) { throw "No exact artifact identity is available for $($run.Board) $($run.Mode)." }
      $artifact = $artifactMatches[0]
      $evidenceDirectory = if ($run.PSObject.Properties["EvidenceDirectory"] -and $run.EvidenceDirectory) { [string]$run.EvidenceDirectory } else { [string]$run.EvidencePath }
      $expectedBoard = if ($run.Board -ceq "Raspberry") { "raspberry-pi-zero-2w" } else { "orange-pi-zero-2w" }
      $evidence = Assert-FrameSearchHostEvidence $evidenceDirectory $run $artifact.Sha256 $expectedBoard
      foreach ($property in @("StatusClass", "Grade", "Worst", "RepeatIncidents", "SilentIncidents", "AlsaRecoveryLogIncidents", "CpalStreamErrors", "CpalDeviceErrors", "CallbackOverBudget", "P999", "MaxRatio", "NativeStatus", "NativeWorker")) { $run.$property = $evidence.$property }
      $run.EvidenceDirectory = $evidence.EvidenceDirectory; $run.EvidencePath = $evidence.EvidencePath; $run.HostEvidencePath = $evidence.HostEvidencePath; $run.ResultPath = $evidence.ResultPath; $run.StructurallyComplete = $true
    } catch {
      $run.Status = "recovery_required"; $run.StructurallyComplete = $false; $run.Failure = "Completed evidence reconciliation failed: $($_.Exception.Message)"; $recovery = $true
    }
  }
  if ($recovery) { $State.status = "recovery_required"; Clear-FrameSearchPublicationState $State | Out-Null }
  return $State
}

function Test-FrameSearchReserveWindow {
  param([Parameter(Mandatory)][datetime]$DeadlineUtc, [datetime]$NowUtc = [datetime]::UtcNow, [int]$ReserveMinutes = 75)
  return ($DeadlineUtc.ToUniversalTime() - $NowUtc.ToUniversalTime()).TotalMinutes -le $ReserveMinutes
}

function Get-FrameSearchPendingAdaptiveRuns {
  param([Parameter(Mandatory)][object]$State)
  $pending = @($State.runs | Where-Object { $_.Wave -like "A*" -and $_.Status -ceq "pending" })
  $indexes = @($pending | Where-Object { $null -ne $_.PSObject.Properties["ScheduleIndex"] } | ForEach-Object { [int]$_.ScheduleIndex })
  if ($indexes.Count -eq $pending.Count -and $indexes.Count -eq @($indexes | Select-Object -Unique).Count) { return @($pending | Sort-Object @{ Expression = { [int]$_.ScheduleIndex }; Ascending = $true }) }
  return $pending
}

function Write-FrameSearchSentinel {
  param([Parameter(Mandatory)][string]$Directory, [Parameter(Mandatory)][string]$Name, [string]$Value = "")
  New-Item -ItemType Directory -Force -Path $Directory | Out-Null
  [IO.File]::WriteAllText((Join-Path $Directory $Name), "$Value`n", (New-Object Text.UTF8Encoding($false)))
}

function Quote-FrameSearchProcessArgument {
  param([Parameter(Mandatory)][string]$Value)
  return '"' + $Value.Replace('"', '\"') + '"'
}

function New-FrameSearchDetachedCommand {
  param([Parameter(Mandatory)][string]$ScriptPath, [Parameter(Mandatory)][string]$StudyId, [Parameter(Mandatory)][string]$PlanPath, [Parameter(Mandatory)][object[]]$Definitions, [int]$WallClockHours = 20, [switch]$Resume, [switch]$AllowServiceInterruption)
  $arguments = @("-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", $ScriptPath, "-StudyId", $StudyId, "-PlanPath", $PlanPath, "-WallClockHours", [string]$WallClockHours, "-RaspberryArtifact", $Definitions[0].ArtifactPath, "-RaspberryMetadata", $Definitions[0].MetadataPath, "-OrangeInlineArtifact", $Definitions[1].ArtifactPath, "-OrangeInlineMetadata", $Definitions[1].MetadataPath, "-OrangeMulticoreArtifact", $Definitions[2].ArtifactPath, "-OrangeMulticoreMetadata", $Definitions[2].MetadataPath, "-Worker")
  if ($Resume) { $arguments += "-Resume" }
  if ($AllowServiceInterruption) { $arguments += "-AllowServiceInterruption" }
  $quoted = @($arguments | ForEach-Object { Quote-FrameSearchProcessArgument $_ })
  return [pscustomobject][ordered]@{ FilePath = Join-Path $PSHOME "powershell.exe"; Arguments = $arguments; ArgumentString = ($quoted -join " ") }
}

function Invoke-FrameSearchChildProcess {
  param([Parameter(Mandatory)][string]$ScriptPath, [Parameter(Mandatory)][string[]]$Arguments, [Parameter(Mandatory)][string]$StdoutPath, [Parameter(Mandatory)][string]$StderrPath)
  New-Item -ItemType Directory -Force -Path (Split-Path -Parent $StdoutPath), (Split-Path -Parent $StderrPath) | Out-Null
  $info = New-Object Diagnostics.ProcessStartInfo
  $processArguments = @("-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", $ScriptPath) + $Arguments
  $info.FileName = Join-Path $PSHOME "powershell.exe"; $info.Arguments = (($processArguments | ForEach-Object { Quote-FrameSearchProcessArgument $_ }) -join " "); $info.UseShellExecute = $false; $info.CreateNoWindow = $true; $info.RedirectStandardOutput = $true; $info.RedirectStandardError = $true
  $process = New-Object Diagnostics.Process; $process.StartInfo = $info
  if (-not $process.Start()) { throw "Unable to start frame-search child runner." }
  $processId = $process.Id
  $stdoutTask = $process.StandardOutput.ReadToEndAsync(); $stderrTask = $process.StandardError.ReadToEndAsync(); $process.WaitForExit(); $stdout = $stdoutTask.Result; $stderr = $stderrTask.Result; $exitCode = $process.ExitCode; $process.Dispose()
  [IO.File]::WriteAllText($StdoutPath, $stdout, (New-Object Text.UTF8Encoding($false))); [IO.File]::WriteAllText($StderrPath, $stderr, (New-Object Text.UTF8Encoding($false)))
  return [pscustomobject][ordered]@{ ProcessId = $processId; ExitCode = $exitCode; StdoutPath = $StdoutPath; StderrPath = $StderrPath; Stdout = $stdout; Stderr = $stderr }
}

function Assert-FrameSearchChildExit {
  param([Parameter(Mandatory)][int]$ExitCode)
  if ($ExitCode -ne 0) { throw "Board runner exited with code $ExitCode." }
}

function Write-FrameSearchResultsMarkdown {
  param([Parameter(Mandatory)][object]$State, [Parameter(Mandatory)][string]$Path)
  $lines = @("# Pi audio capacity frame search results", "", "- Study: $($State.study_id)", "- Status: $($State.status)", "- Plan SHA256: $($State.plan_sha256)", "- Source commit: $($State.source_commit)", "", "## Paired cells", "", "| Cell | Profile | U | Grade | Worst | Rep 1 | Rep 2 | Native | CB over | P999 | Evidence |", "|---|---|---:|---|---:|---|---|---|---:|---:|---|")
  foreach ($pair in @(Get-FrameSearchPairObservations @($State.runs))) {
    $cellRuns = @($State.runs | Where-Object { $_.Cell -ceq $pair.Cell } | Sort-Object Rep)
    $rep1Status = if ($cellRuns.Count -gt 0) { $cellRuns[0].Status } else { "" }; $rep2Status = if ($cellRuns.Count -gt 1) { $cellRuns[1].Status } else { "" }
    $evidence = (($cellRuns | Where-Object { $_.EvidenceDirectory } | ForEach-Object { $_.EvidenceDirectory }) -join "<br>")
    $lines += "| $($pair.Cell) | $($pair.Profile) | $($pair.U) | $($pair.Grade) | $($pair.Worst) | $rep1Status | $rep2Status | $($pair.NativeStatus) | $($pair.CallbackOverBudget) | $($pair.P999) | $evidence |"
  }
  $lines += @("", "## Profile analyses", "", "| Profile | Board | Mode | Resolution | Top U | Top grade | Borders | Required probes |", "|---|---|---|---|---:|---|---|---|")
  foreach ($analysis in @($State.analyses)) { $lines += "| $($analysis.Profile) | $($analysis.Board) | $($analysis.Mode) | $($analysis.Resolution) | $($analysis.TopU) | $($analysis.TopGrade) | $((@($analysis.Borders | ForEach-Object { $_.Kind }) -join '<br>')) | $((@($analysis.RequiredProbes | ForEach-Object { $_.Cell }) -join '<br>')) |" }
  $lines += @("", "## Borders, reversals, and skipped rows", "", "| Kind | Profile/Cell | Decision |", "|---|---|---|")
  foreach ($decision in @($State.decisions)) { $lines += "| $($decision.Kind) | $($decision.Profile) | $($decision.Reason) |" }
  foreach ($run in @($State.runs | Where-Object { $_.Status -eq "skipped" })) { $lines += "| skipped | $($run.Cell) | $($run.SkipReason) |" }
  $lines += @("", "## Provisional winners", "", "| Board | Mode | Profile | U | Grade | Geometry |", "|---|---|---|---:|---|---|")
  foreach ($winner in @($State.provisional_winners)) { $lines += "| $($winner.Board) | $($winner.Mode) | $($winner.Profile) | $($winner.U) | $($winner.Grade) | $($winner.Geometry) |" }
  $lines += @("", "## Validated winners", "", "| Board | Mode | Profile | U | Grade | Geometry |", "|---|---|---|---:|---|---|")
  foreach ($winner in @($State.winners)) { $lines += "| $($winner.Board) | $($winner.Mode) | $($winner.Profile) | $($winner.U) | $($winner.Grade) | $($winner.Geometry) |" }
  $lines += @("", "## Soaks", "", "| Board | Mode | Profile | U | Grade | Worst | Geometry | Evidence |", "|---|---|---|---:|---|---:|---|---|")
  foreach ($soak in @($State.soaks)) { $lines += "| $($soak.Board) | $($soak.Mode) | $($soak.Profile) | $($soak.U) | $($soak.Grade) | $($soak.Worst) | $($soak.Geometry) | $($soak.EvidenceDirectory) |" }
  $lines += @("", "## Multicore gain", "", "| Board | Inline U | Multicore U | Delta U | Gain % | Synth delta3x | Sample delta1x |", "|---|---:|---:|---:|---:|---:|---:|")
  foreach ($gain in @($State.gains)) { $lines += "| $($gain.Board) | $($gain.InlineU) | $($gain.MulticoreU) | $($gain.DeltaU) | $([math]::Round($gain.GainPercent, 2)) | $($gain.SynthDelta3x) | $($gain.SampleDelta1x) |" }
  $directory = Split-Path -Parent $Path; New-Item -ItemType Directory -Force -Path $directory | Out-Null; $temporary = "$Path.tmp-$PID-$([guid]::NewGuid().ToString('N'))"; $backup = "$Path.bak-$PID-$([guid]::NewGuid().ToString('N'))"
  try { [IO.File]::WriteAllText($temporary, (($lines -join "`n") + "`n"), (New-Object Text.UTF8Encoding($false))); if (Test-Path -LiteralPath $Path -PathType Leaf) { [IO.File]::Replace($temporary, $Path, $backup, $true) } else { Move-Item -LiteralPath $temporary -Destination $Path } } finally { Remove-Item -LiteralPath $temporary, $backup -Force -ErrorAction SilentlyContinue }
}

Export-ModuleMember -Function Get-FrameSearchRunnerVersion, Get-FrameSearchStateSchema, Get-FrameSearchRepoCommit, Get-FrameSearchArtifactDefinitions, Get-FrameSearchArtifactSet, Assert-FrameSearchArtifactIdentity, Write-FrameSearchAtomicJson, Read-FrameSearchState, Enter-FrameSearchStudyLock, Exit-FrameSearchStudyLock, Clear-FrameSearchPublicationState, New-FrameSearchState, Assert-FrameSearchResumeIdentity, Reconcile-FrameSearchState, Test-FrameSearchReserveWindow, Get-FrameSearchPendingAdaptiveRuns, Write-FrameSearchSentinel, New-FrameSearchDetachedCommand, Invoke-FrameSearchChildProcess, Assert-FrameSearchChildExit, Write-FrameSearchResultsMarkdown
