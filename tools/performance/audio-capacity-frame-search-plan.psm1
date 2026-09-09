Set-StrictMode -Version Latest

$script:FrameSearchCandidateColumns = @("Profile", "Board", "Mode", "Output", "Period", "Internal", "Lookahead", "Effective", "Seed U", "Role")
$script:FrameSearchQueueColumns = @("Cell", "Wave", "Profile", "Board", "Mode", "Output", "Period", "Internal", "Lookahead", "Effective", "U", "Sec", "Reps", "State", "Condition")
$script:FrameSearchSoakColumns = @("Soak", "Board", "Mode", "Profile", "Geometry", "U", "Sec", "Reps", "State")
$script:FrameSearchProfiles = [ordered]@{
  RI64 = [pscustomobject][ordered]@{ Profile = "RI64"; Board = "Raspberry"; Mode = "Inline"; Output = 256; Period = 64; Internal = 64; Lookahead = 0; Effective = 256; Seeds = @(12, 24, 32); Role = "diagnostic alternative" }
  RI128 = [pscustomobject][ordered]@{ Profile = "RI128"; Board = "Raspberry"; Mode = "Inline"; Output = 256; Period = 64; Internal = 128; Lookahead = 0; Effective = 256; Seeds = @(8, 12, 16, 24, 32); Role = "current anchor" }
  RI512 = [pscustomobject][ordered]@{ Profile = "RI512"; Board = "Raspberry"; Mode = "Inline"; Output = 512; Period = 128; Internal = 128; Lookahead = 0; Effective = 512; Seeds = @(12, 24, 32); Role = "diagnostic alternative" }
  RM64 = [pscustomobject][ordered]@{ Profile = "RM64"; Board = "Raspberry"; Mode = "Multicore"; Output = 256; Period = 64; Internal = 64; Lookahead = 64; Effective = 320; Seeds = @(12, 24, 32); Role = "low-latency candidate; prior short evidence poor; diagnostic alternative" }
  RM128 = [pscustomobject][ordered]@{ Profile = "RM128"; Board = "Raspberry"; Mode = "Multicore"; Output = 256; Period = 64; Internal = 128; Lookahead = 128; Effective = 384; Seeds = @(12, 16, 20, 24, 32); Role = "current diagnostic anchor; non-monotonic check" }
  RM256 = [pscustomobject][ordered]@{ Profile = "RM256"; Board = "Raspberry"; Mode = "Multicore"; Output = 256; Period = 64; Internal = 256; Lookahead = 256; Effective = 512; Seeds = @(12, 24, 32); Role = "diagnostic alternative" }
  OI32 = [pscustomobject][ordered]@{ Profile = "OI32"; Board = "Orange"; Mode = "Inline"; Output = 128; Period = 32; Internal = 32; Lookahead = 0; Effective = 128; Seeds = @(12, 16, 24, 32); Role = "current anchor" }
  OI64 = [pscustomobject][ordered]@{ Profile = "OI64"; Board = "Orange"; Mode = "Inline"; Output = 128; Period = 32; Internal = 64; Lookahead = 0; Effective = 128; Seeds = @(12, 24, 32); Role = "diagnostic alternative" }
  OI256 = [pscustomobject][ordered]@{ Profile = "OI256"; Board = "Orange"; Mode = "Inline"; Output = 256; Period = 64; Internal = 64; Lookahead = 0; Effective = 256; Seeds = @(12, 24, 32); Role = "diagnostic alternative" }
  OM32 = [pscustomobject][ordered]@{ Profile = "OM32"; Board = "Orange"; Mode = "Multicore"; Output = 128; Period = 32; Internal = 32; Lookahead = 32; Effective = 160; Seeds = @(12, 24, 32); Role = "diagnostic alternative" }
  OM64 = [pscustomobject][ordered]@{ Profile = "OM64"; Board = "Orange"; Mode = "Multicore"; Output = 256; Period = 64; Internal = 64; Lookahead = 64; Effective = 320; Seeds = @(12, 16, 24, 32); Role = "current anchor" }
  OM128 = [pscustomobject][ordered]@{ Profile = "OM128"; Board = "Orange"; Mode = "Multicore"; Output = 256; Period = 64; Internal = 128; Lookahead = 128; Effective = 384; Seeds = @(12, 24, 32); Role = "diagnostic alternative" }
}

function Get-FrameSearchProfileDefinitions {
  return @($script:FrameSearchProfiles.Values)
}

function Get-FrameSearchProfileDefinition {
  param([Parameter(Mandatory)][string]$Profile)
  $canonical = @($script:FrameSearchProfiles.Keys | Where-Object { $_ -ceq $Profile })
  if ($canonical.Count -ne 1) { throw "Unsupported frame-search profile: $Profile" }
  return $script:FrameSearchProfiles[$canonical[0]]
}

function Read-FrameSearchUtf8Text {
  param([Parameter(Mandatory)][string]$Path)
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Frame-search plan was not found: $Path" }
  try { return [IO.File]::ReadAllText($Path, (New-Object Text.UTF8Encoding($false, $true))) } catch { throw "Frame-search plan is not valid UTF-8: $Path" }
}

function ConvertFrom-FrameSearchMarkdownRow {
  param([Parameter(Mandatory)][string]$Line, [Parameter(Mandatory)][string]$Context)
  $trimmed = $Line.Trim()
  if (-not ($trimmed.StartsWith("|") -and $trimmed.EndsWith("|"))) { throw "$Context is not a Markdown table row." }
  $inner = $trimmed.Substring(1, $trimmed.Length - 2)
  return @($inner.Split("|") | ForEach-Object { $_.Trim() })
}

function Find-FrameSearchMarkdownTable {
  param([Parameter(Mandatory)][string]$Text, [Parameter(Mandatory)][string[]]$Headers, [Parameter(Mandatory)][string]$Name)
  $lines = $Text -split "`r?`n"
  $foundTables = @()
  for ($index = 0; $index -lt $lines.Count; $index++) {
    if ($lines[$index].Trim() -notmatch '^\|.*\|$') { continue }
    $header = ConvertFrom-FrameSearchMarkdownRow $lines[$index] "$Name header"
    if (($header -join "`0") -cne ($Headers -join "`0")) { continue }
    if ($index + 1 -ge $lines.Count) { throw "$Name table is missing its separator row." }
    $separator = ConvertFrom-FrameSearchMarkdownRow $lines[$index + 1] "$Name separator"
    if ($separator.Count -ne $Headers.Count -or @($separator | Where-Object { $_ -notmatch '^:?-{3,}:?$' }).Count -ne 0) { throw "$Name table has an invalid separator row." }
    $rows = @()
    for ($rowIndex = $index + 2; $rowIndex -lt $lines.Count; $rowIndex++) {
      if ([string]::IsNullOrWhiteSpace($lines[$rowIndex])) { break }
      if ($lines[$rowIndex].Trim() -notmatch '^\|.*\|$') { break }
      $row = ConvertFrom-FrameSearchMarkdownRow $lines[$rowIndex] "$Name row $($rowIndex + 1)"
      if ($row.Count -ne $Headers.Count) { throw "$Name row $($rowIndex + 1) has $($row.Count) columns; expected $($Headers.Count)." }
      $rows += ,$row
    }
    if ($rows.Count -eq 0) { throw "$Name table has no data rows." }
    $foundTables += ,([pscustomobject]@{ Headers = $header; Rows = $rows })
  }
  if ($foundTables.Count -ne 1) { throw "Expected exactly one $Name Markdown table; found $($foundTables.Count)." }
  return $foundTables[0]
}

function ConvertTo-FrameSearchInteger {
  param([Parameter(Mandatory)][string]$Value, [Parameter(Mandatory)][string]$Context, [int]$Minimum = 0, [int]$Maximum = [int]::MaxValue)
  if ($Value -notmatch '^(0|[1-9][0-9]*)$') { throw "$Context must be a canonical integer." }
  try { $number = [int]$Value } catch { throw "$Context is outside the supported integer range." }
  if ($number -lt $Minimum -or $number -gt $Maximum) { throw "$Context must be between $Minimum and $Maximum." }
  return $number
}

function ConvertTo-FrameSearchRowObject {
  param([Parameter(Mandatory)][string[]]$Headers, [Parameter(Mandatory)][string[]]$Values)
  $row = [ordered]@{}
  for ($index = 0; $index -lt $Headers.Count; $index++) { $row[$Headers[$index]] = $Values[$index] }
  return [pscustomobject]$row
}

function ConvertTo-FrameSearchSeedValues {
  param([Parameter(Mandatory)][string]$Value, [Parameter(Mandatory)][string]$Context)
  if ([string]::IsNullOrWhiteSpace($Value)) { throw "$Context must not be empty." }
  $values = @($Value -split ',\s*' | ForEach-Object { ConvertTo-FrameSearchInteger $_ "$Context entry" 1 42 })
  if ($values.Count -ne (@($values | Select-Object -Unique).Count)) { throw "$Context contains duplicate U values." }
  return $values
}

function Assert-FrameSearchCandidateTable {
  param([Parameter(Mandatory)][object]$Table)
  if ($Table.Rows.Count -ne $script:FrameSearchProfiles.Count) { throw "Candidate profile count must be exactly $($script:FrameSearchProfiles.Count)." }
  $seen = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
  foreach ($values in $Table.Rows) {
    $row = ConvertTo-FrameSearchRowObject $Table.Headers $values
    $profile = Get-FrameSearchProfileDefinition $row.Profile
    if (-not $seen.Add($row.Profile)) { throw "Duplicate candidate profile: $($row.Profile)." }
    if ($row.Board -cne $profile.Board -or $row.Mode -cne $profile.Mode) { throw "Candidate $($row.Profile) board or mode identity changed." }
    foreach ($field in @("Output", "Period", "Internal", "Lookahead", "Effective")) {
      if ((ConvertTo-FrameSearchInteger $row.$field "Candidate $($row.Profile).$field") -ne $profile.$field) { throw "Candidate $($row.Profile) geometry tuple changed." }
    }
    $seeds = ConvertTo-FrameSearchSeedValues $row.'Seed U' "Candidate $($row.Profile).Seed U"
    if (($seeds -join ",") -cne ($profile.Seeds -join ",")) { throw "Candidate $($row.Profile) seed U values changed." }
    if ($row.Role -cne $profile.Role) { throw "Candidate $($row.Profile) role changed." }
  }
  foreach ($profile in $script:FrameSearchProfiles.Values) { if (-not $seen.Contains($profile.Profile)) { throw "Candidate profile is missing: $($profile.Profile)." } }
}

function Assert-FrameSearchQueueTable {
  param([Parameter(Mandatory)][object]$Table)
  $seen = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
  $coverage = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
  $allowedWaves = @("W1", "W2", "W3", "W4")
  $allowedConditions = @("always", "skip if profile dominated/unsafe")
  if ($Table.Rows.Count -lt 4) { throw "Seed queue must cover all four board/mode groups." }
  foreach ($values in $Table.Rows) {
    $row = ConvertTo-FrameSearchRowObject $Table.Headers $values
    if (-not $seen.Add($row.Cell)) { throw "Duplicate seed cell ID: $($row.Cell)." }
    $cellMatch = [regex]::Match([string]$row.Cell, '^(?<profile>RI64|RI128|RI512|RM64|RM128|RM256|OI32|OI64|OI256|OM32|OM64|OM128)-U(?<u>[1-9]|[1-3][0-9]|4[0-2])$', [Text.RegularExpressions.RegexOptions]::CultureInvariant)
    if (-not $cellMatch.Success) { throw "Unsupported or altered seed cell ID: $($row.Cell)." }
    $cellProfile = $cellMatch.Groups["profile"].Value; $cellU = $cellMatch.Groups["u"].Value
    $profile = Get-FrameSearchProfileDefinition $cellProfile
    if (-not [StringComparer]::Ordinal.Equals([string]$row.Profile, [string]$cellProfile)) { throw "Seed $($row.Cell) profile does not match its cell identity." }
    $u = ConvertTo-FrameSearchInteger $row.U "Seed $($row.Cell).U" 1 42
    if ([int]$cellU -ne $u) { throw "Seed $($row.Cell) U does not match its cell identity." }
    if ($allowedWaves -cnotcontains $row.Wave -or $allowedConditions -cnotcontains $row.Condition) { throw "Seed $($row.Cell) has an unsupported wave or condition." }
    if ($row.State -cne "pending") { throw "Seed $($row.Cell) must start pending." }
    if ((ConvertTo-FrameSearchInteger $row.Sec "Seed $($row.Cell).Sec") -ne 180 -or (ConvertTo-FrameSearchInteger $row.Reps "Seed $($row.Cell).Reps") -ne 2) { throw "Seed $($row.Cell) must be 180 seconds with two repetitions." }
    if ($row.Board -cne $profile.Board -or $row.Mode -cne $profile.Mode) { throw "Seed $($row.Cell) board or mode identity changed." }
    foreach ($field in @(@("Output", "Output"), @("Period", "Period"), @("Internal", "Internal"), @("Lookahead", "Lookahead"), @("Effective", "Effective"))) {
      if ((ConvertTo-FrameSearchInteger $row.$($field[0]) "Seed $($row.Cell).$($field[0])") -ne $profile.$($field[1])) { throw "Seed $($row.Cell) geometry tuple changed." }
    }
    $coverage.Add("$($profile.Board)/$($profile.Mode)") | Out-Null
  }
  foreach ($group in @("Raspberry/Inline", "Raspberry/Multicore", "Orange/Inline", "Orange/Multicore")) { if (-not $coverage.Contains($group)) { throw "Seed queue lacks required board/mode coverage: $group." } }
}

function Assert-FrameSearchSoakTable {
  param([Parameter(Mandatory)][object]$Table)
  $expected = @(
    @("SOAK-RI", "Raspberry", "Inline"), @("SOAK-RM", "Raspberry", "Multicore"),
    @("SOAK-OI", "Orange", "Inline"), @("SOAK-OM", "Orange", "Multicore")
  )
  if ($Table.Rows.Count -ne $expected.Count) { throw "Soak placeholder count must be exactly four." }
  $seen = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::Ordinal)
  foreach ($values in $Table.Rows) {
    $row = ConvertTo-FrameSearchRowObject $Table.Headers $values
    $entry = @($expected | Where-Object { $_[0] -ceq $row.Soak })
    if ($entry.Count -ne 1 -or -not $seen.Add($row.Soak)) { throw "Unsupported or duplicate soak placeholder: $($row.Soak)." }
    if ($row.Board -cne $entry[0][1] -or $row.Mode -cne $entry[0][2] -or $row.Profile -cne "TBD from winner" -or $row.Geometry -cne "TBD from winner" -or $row.U -cne "TBD" -or (ConvertTo-FrameSearchInteger $row.Sec "Soak $($row.Soak).Sec") -ne 600 -or (ConvertTo-FrameSearchInteger $row.Reps "Soak $($row.Soak).Reps") -ne 1 -or $row.State -cne "pending") { throw "Soak placeholder $($row.Soak) changed." }
  }
  if ($seen.Count -ne $expected.Count) { throw "A soak placeholder is missing." }
}

function Read-FrameSearchPlan {
  param([Parameter(Mandatory)][string]$Path)
  $resolved = (Resolve-Path -LiteralPath $Path -ErrorAction Stop).Path
  $text = Read-FrameSearchUtf8Text $resolved
  $candidateTable = Find-FrameSearchMarkdownTable $text $script:FrameSearchCandidateColumns "candidate profile"
  $queueTable = Find-FrameSearchMarkdownTable $text $script:FrameSearchQueueColumns "seed queue"
  $soakTable = Find-FrameSearchMarkdownTable $text $script:FrameSearchSoakColumns "soak placeholder"
  Assert-FrameSearchCandidateTable $candidateTable
  Assert-FrameSearchQueueTable $queueTable
  Assert-FrameSearchSoakTable $soakTable
  $profiles = @($candidateTable.Rows | ForEach-Object { ConvertTo-FrameSearchRowObject $candidateTable.Headers $_ })
  $queue = @($queueTable.Rows | ForEach-Object { ConvertTo-FrameSearchRowObject $queueTable.Headers $_ })
  $soaks = @($soakTable.Rows | ForEach-Object { ConvertTo-FrameSearchRowObject $soakTable.Headers $_ })
  foreach ($row in $profiles) { foreach ($field in @("Output", "Period", "Internal", "Lookahead", "Effective")) { $row.$field = ConvertTo-FrameSearchInteger $row.$field "Candidate $($row.Profile).$field" } ; $row.'Seed U' = @(ConvertTo-FrameSearchSeedValues $row.'Seed U' "Candidate $($row.Profile).Seed U") }
  foreach ($row in $queue) { foreach ($field in @("Output", "Period", "Internal", "Lookahead", "Effective", "U", "Sec", "Reps")) { $row.$field = ConvertTo-FrameSearchInteger $row.$field "Seed $($row.Cell).$field" } }
  foreach ($row in $soaks) { $row.Sec = ConvertTo-FrameSearchInteger $row.Sec "Soak $($row.Soak).Sec"; $row.Reps = ConvertTo-FrameSearchInteger $row.Reps "Soak $($row.Soak).Reps" }
  return [pscustomobject][ordered]@{
    Path = $resolved
    Sha256 = (Get-FileHash -LiteralPath $resolved -Algorithm SHA256).Hash.ToLowerInvariant()
    CandidateProfiles = $profiles
    SeedQueue = $queue
    SoakPlaceholders = $soaks
  }
}

Export-ModuleMember -Function Get-FrameSearchProfileDefinitions, Get-FrameSearchProfileDefinition, Read-FrameSearchPlan
