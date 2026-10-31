$ErrorActionPreference = "Stop"

$realScriptPath = Join-Path $PSScriptRoot "run-opi-bringup.ps1"
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("octessera-opi-bringup-test-" + [guid]::NewGuid().ToString("N"))
$scriptPath = Join-Path $testRoot "run-opi-bringup.ps1"
$fakeBin = Join-Path $testRoot "fake tools"
$recordPath = Join-Path $testRoot "transport.jsonl"
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)

function Write-Utf8NoBom {
  param([string]$Path, [string]$Contents)
  [IO.File]::WriteAllText($Path, $Contents, $utf8NoBom)
}

function Assert-ArgumentsEqual {
  param([object[]]$Actual, [object[]]$Expected, [string]$Label)
  if ($Actual.Count -ne $Expected.Count) {
    throw "$Label argument count mismatch: expected $($Expected.Count), got $($Actual.Count)"
  }
  for ($index = 0; $index -lt $Expected.Count; $index++) {
    if ([string]$Actual[$index] -cne [string]$Expected[$index]) {
      throw "$Label argument $index mismatch: expected [$($Expected[$index])], got [$($Actual[$index])]"
    }
  }
}

function Read-TransportRecords {
  $records = @()
  foreach ($line in [IO.File]::ReadAllLines($recordPath)) {
    $records += ,($line | ConvertFrom-Json)
  }
  return $records
}

function Assert-TransportRecord {
  param([object]$Record, [string]$Tool, [object[]]$ExpectedArguments, [string]$Label)
  if ($Record.tool -cne $Tool -or $Record.board -cne "opi" -or $Record.target -cne $target) {
    throw "$Label transport mismatch: expected [$Tool opi $target], got [$($Record.tool) $($Record.board) $($Record.target)]"
  }
  Assert-ArgumentsEqual @($Record.arguments) $ExpectedArguments $Label
}

function Invoke-BringupForTest {
  param([hashtable]$Parameters)
  $output = @()
  $threw = $false
  try {
    & $scriptPath @Parameters 3>&1 2>&1 | ForEach-Object { $output += $_ }
  } catch {
    $threw = $true
    $output += $_
  }
  return [pscustomobject]@{ Output = $output; Threw = $threw }
}

try {
  New-Item -ItemType Directory -Path $fakeBin -Force | Out-Null
  $fakeTransport = Join-Path $fakeBin "fake-transport.ps1"
  Write-Utf8NoBom $fakeTransport @'
param([string]$Command, [string]$Board, [string]$Target, [Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest)
$tool = $Command
$arguments = @($Rest)
$record = [ordered]@{ tool = $tool; board = $Board; target = $Target; arguments = $arguments }
[IO.File]::AppendAllText($env:OCTESSERA_FAKE_TRANSPORT_LOG, ($record | ConvertTo-Json -Compress) + [Environment]::NewLine)
$isLatestLogLookup = $tool -eq "ssh" -and $arguments.Count -gt 0 -and $arguments[$arguments.Count - 1] -like "cat *latest-log-path*"
if ($isLatestLogLookup -and $env:OCTESSERA_FAKE_SSH_LOG_LOOKUP -eq "failed") { exit 23 }
if ($isLatestLogLookup -and $env:OCTESSERA_FAKE_SSH_LOG_LOOKUP -eq "empty") { exit 0 }
if ($isLatestLogLookup -and $env:OCTESSERA_FAKE_SSH_LOG_LOOKUP -eq "multiple") {
  Write-Output "/tmp/fake bringup-one.log"
  Write-Output "/tmp/fake bringup-two.log"
  exit 0
}
if ($tool -eq "ssh" -and -not $isLatestLogLookup -and $env:OCTESSERA_FAKE_SSH_PROBE -eq "failed") { exit 17 }
if ($isLatestLogLookup) { Write-Output "/tmp/fake bringup.log" }
if ($tool -eq "scp" -and $env:OCTESSERA_FAKE_SCP_FAILURE_SOURCE -ne $null) {
  foreach ($argument in $arguments) {
    if ([string]$argument -like "*$($env:OCTESSERA_FAKE_SCP_FAILURE_SOURCE)") { exit 19 }
  }
}
exit 0
'@
  $transportLine = '$transport = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\board-dev\with-board-ssh.ps1")).Path'
  $source = [IO.File]::ReadAllText($realScriptPath)
  if (-not $source.Contains($transportLine)) { throw "test could not replace the transport binding" }
  $rootLiteral = "'" + $PSScriptRoot.Replace("'", "''") + "'"
  $copySource = $source.Replace($transportLine, "`$transport = '$($fakeTransport.Replace("'", "''"))'").Replace('$PSScriptRoot', $rootLiteral)
  Write-Utf8NoBom $scriptPath $copySource
  $env:OCTESSERA_FAKE_TRANSPORT_LOG = $recordPath

  $savedErrorActionPreference = $ErrorActionPreference
  [IO.File]::WriteAllText($recordPath, "", $utf8NoBom)
  $ErrorActionPreference = "Continue"
  $missingTargetOutput = @(& (Join-Path $PSHOME "powershell.exe") -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $scriptPath 2>&1)
  $missingTargetExitCode = $LASTEXITCODE
  $ErrorActionPreference = $savedErrorActionPreference
  if ($missingTargetExitCode -eq 0 -or ($missingTargetOutput -join "`n") -notmatch "Target") {
    throw "run-opi-bringup.ps1 did not require -Target"
  }

  if (@(Read-TransportRecords).Count -ne 0) {
    throw "Omitted target reached mocked transport."
  }

  $malformedTarget = Invoke-BringupForTest -Parameters @{
    Target = "octessera@bad host"
    LocalOutputDir = (Join-Path $testRoot "malformed-target-output")
  }
  $malformedTargetText = ($malformedTarget.Output | ForEach-Object { [string]$_ }) -join "`n"
  if (-not $malformedTarget.Threw -or $malformedTargetText -notmatch "target" -or @(Read-TransportRecords).Count -ne 0 -or (Test-Path -LiteralPath (Join-Path $testRoot "malformed-target-output"))) {
    throw "Malformed target was not rejected before mocked transport or output creation."
  }

  $target = "octessera@orange.test.invalid"
  $cases = @(
    [pscustomobject]@{
      Name = "simple"
      RemoteScript = "/tmp/octessera-opi-probe.sh"
      RemoteLogDir = "/tmp/octessera-opi-bringup"
    }
  )

  foreach ($case in $cases) {
    [IO.File]::WriteAllText($recordPath, "", $utf8NoBom)
    $localOutputDir = Join-Path $testRoot ("local output " + $case.Name)
    $null = & $scriptPath `
      -Target $target `
      -RemoteScript $case.RemoteScript `
      -RemoteLogDir $case.RemoteLogDir `
      -LocalOutputDir $localOutputDir
    if (-not (Test-Path -LiteralPath $localOutputDir -PathType Container)) {
      throw "local output directory was not created for $($case.Name)"
    }

    $records = @(Read-TransportRecords)
    if ($records.Count -ne 7) {
      throw "expected two ssh and five scp calls for $($case.Name), got $($records.Count)"
    }
    $probeScriptPath = Join-Path $PSScriptRoot "opi-bringup-probe.sh"
    Assert-TransportRecord $records[0] "scp" @($probeScriptPath, "${target}:$($case.RemoteScript)") "$($case.Name) upload"
    $supportNames = @("opi-bringup-validator.sh", "opi-bringup-identity-validator.sh", "opi-bringup-hardware-validator.sh")
    for ($supportIndex = 0; $supportIndex -lt $supportNames.Count; $supportIndex++) {
      $supportName = $supportNames[$supportIndex]
      $supportPath = Join-Path $PSScriptRoot $supportName
      $remoteDirectory = $case.RemoteScript.Substring(0, $case.RemoteScript.LastIndexOf('/'))
      $remoteSupportPath = if ($remoteDirectory -eq "/") { "/$supportName" } else { "$remoteDirectory/$supportName" }
      Assert-TransportRecord $records[$supportIndex + 1] "scp" @($supportPath, "${target}:$remoteSupportPath") "$($case.Name) $supportName upload"
    }
    if (@($records[4].arguments).Count -ne 1 -or $records[4].arguments[0] -notlike "umask 077; *bash '$($case.RemoteScript)' *") { throw "$($case.Name) probe command mismatch" }
    Assert-TransportRecord $records[4] "ssh" @($records[4].arguments) "$($case.Name) probe"
    Assert-TransportRecord $records[5] "ssh" @("cat '$($case.RemoteLogDir)'/latest-log-path 2>/dev/null") "$($case.Name) latest-log-path"
    Assert-TransportRecord $records[6] "scp" @("${target}:/tmp/fake bringup.log", $localOutputDir) "$($case.Name) log download"
  }

  $collisionNames = @("opi-bringup-validator.sh", "opi-bringup-identity-validator.sh", "opi-bringup-hardware-validator.sh")
  foreach ($collisionName in $collisionNames) {
    [IO.File]::WriteAllText($recordPath, "", $utf8NoBom)
    $collision = Invoke-BringupForTest -Parameters @{
      Target = $target
      RemoteScript = "/tmp/$collisionName"
      LocalOutputDir = (Join-Path $testRoot ("collision-" + $collisionName))
    }
    $collisionText = ($collision.Output | ForEach-Object { [string]$_ }) -join "`n"
    if (-not $collision.Threw -or $collisionText -notmatch "collides with derived support file" -or $collisionText -match "Orange Pi bring-up probe complete") {
      throw "RemoteScript collision was not rejected before transfer: $collisionName"
    }
    if (@(Read-TransportRecords).Count -ne 0) {
      throw "RemoteScript collision attempted transport: $collisionName"
    }
  }

  $invalidRemotePathCases = @(
    [pscustomobject]@{ ParameterName = "RemoteScript"; Value = "" }
    [pscustomobject]@{ ParameterName = "RemoteScript"; Value = "relative-probe.sh" }
    [pscustomobject]@{ ParameterName = "RemoteScript"; Value = "/" }
    [pscustomobject]@{ ParameterName = "RemoteScript"; Value = "/tmp/" }
    [pscustomobject]@{ ParameterName = "RemoteScript"; Value = "/tmp/../probe.sh" }
    [pscustomobject]@{ ParameterName = "RemoteScript"; Value = "/tmp/probe name.sh" }
    [pscustomobject]@{ ParameterName = "RemoteScript"; Value = "/tmp/probe;echo.sh" }
    [pscustomobject]@{ ParameterName = "RemoteScript"; Value = "/tmp/probe`n.sh" }
    [pscustomobject]@{ ParameterName = "RemoteLogDir"; Value = "" }
    [pscustomobject]@{ ParameterName = "RemoteLogDir"; Value = "relative-logs" }
    [pscustomobject]@{ ParameterName = "RemoteLogDir"; Value = "/" }
    [pscustomobject]@{ ParameterName = "RemoteLogDir"; Value = "/tmp/" }
    [pscustomobject]@{ ParameterName = "RemoteLogDir"; Value = "/tmp/logs with spaces" }
    [pscustomobject]@{ ParameterName = "RemoteLogDir"; Value = "/tmp/logs&echo" }
    [pscustomobject]@{ ParameterName = "RemoteLogDir"; Value = "/tmp/logs`r`n" }
  )
  foreach ($invalidRemotePath in $invalidRemotePathCases) {
    [IO.File]::WriteAllText($recordPath, "", $utf8NoBom)
    $invalidParameters = @{
      Target = $target
      RemoteScript = "/tmp/octessera-opi-bringup-probe.sh"
      RemoteLogDir = "/tmp/octessera-opi-bringup"
      LocalOutputDir = (Join-Path $testRoot ("invalid-" + $invalidRemotePath.ParameterName))
    }
    $invalidParameters[$invalidRemotePath.ParameterName] = $invalidRemotePath.Value
    $invalid = Invoke-BringupForTest -Parameters @{
      Target = $invalidParameters.Target
      RemoteScript = $invalidParameters.RemoteScript
      RemoteLogDir = $invalidParameters.RemoteLogDir
      LocalOutputDir = $invalidParameters.LocalOutputDir
    }
    $invalidText = ($invalid.Output | ForEach-Object { [string]$_ }) -join "`n"
    if (-not $invalid.Threw -or $invalidText -notmatch $invalidRemotePath.ParameterName -or @(Read-TransportRecords).Count -ne 0) {
      throw "invalid $($invalidRemotePath.ParameterName) was not rejected before transfer: $($invalidRemotePath.Value)"
    }
  }

  foreach ($failedSource in @("opi-bringup-probe.sh", "opi-bringup-validator.sh", "opi-bringup-identity-validator.sh", "opi-bringup-hardware-validator.sh")) {
    [IO.File]::WriteAllText($recordPath, "", $utf8NoBom)
    $env:OCTESSERA_FAKE_SCP_FAILURE_SOURCE = $failedSource
    $uploadFailure = Invoke-BringupForTest -Parameters @{
      Target = $target
      RemoteScript = "/tmp/upload-failure.sh"
      LocalOutputDir = (Join-Path $testRoot ("upload-failure-" + $failedSource))
    }
    $uploadFailureText = ($uploadFailure.Output | ForEach-Object { [string]$_ }) -join "`n"
    if (-not $uploadFailure.Threw -or $uploadFailureText -match "Orange Pi bring-up probe complete" -or @((Read-TransportRecords) | Where-Object { $_.tool -eq "ssh" }).Count -ne 0) {
      throw "upload failure did not stop before SSH/completion: $failedSource"
    }
    Remove-Item Env:\OCTESSERA_FAKE_SCP_FAILURE_SOURCE -ErrorAction SilentlyContinue
  }

  $failureParameters = @{
    Target = $target
    RemoteScript = "/tmp/failure-probe.sh"
    RemoteLogDir = "/tmp/failure-logs"
    LocalOutputDir = (Join-Path $testRoot "failure output")
  }

  [IO.File]::WriteAllText($recordPath, "", $utf8NoBom)
  $env:OCTESSERA_FAKE_SSH_LOG_LOOKUP = "failed"
  $lookupFailure = Invoke-BringupForTest -Parameters $failureParameters
  $lookupFailureText = ($lookupFailure.Output | ForEach-Object { [string]$_ }) -join "`n"
  if ($lookupFailure.Threw -or $lookupFailureText -notmatch "Could not read remote latest-log-path" -or $lookupFailureText -notmatch "Orange Pi bring-up probe complete") {
    throw "failed/no-output log lookup did not preserve warning and successful probe completion"
  }
  if (@(Read-TransportRecords).Count -ne 6) { throw "failed log lookup should not attempt log download" }
  Remove-Item Env:\OCTESSERA_FAKE_SSH_LOG_LOOKUP -ErrorAction SilentlyContinue

  [IO.File]::WriteAllText($recordPath, "", $utf8NoBom)
  $env:OCTESSERA_FAKE_SSH_LOG_LOOKUP = "multiple"
  $multipleLookup = Invoke-BringupForTest -Parameters $failureParameters
  $multipleLookupText = ($multipleLookup.Output | ForEach-Object { [string]$_ }) -join "`n"
  if ($multipleLookup.Threw -or $multipleLookupText -notmatch "Could not read remote latest-log-path" -or $multipleLookupText -notmatch "Orange Pi bring-up probe complete") {
    throw "multiple-line log lookup did not preserve warning and successful probe completion"
  }
  if (@(Read-TransportRecords).Count -ne 6) { throw "multiple-line log lookup should not attempt log download" }
  Remove-Item Env:\OCTESSERA_FAKE_SSH_LOG_LOOKUP -ErrorAction SilentlyContinue

  [IO.File]::WriteAllText($recordPath, "", $utf8NoBom)
  $env:OCTESSERA_FAKE_SSH_LOG_LOOKUP = "empty"
  $env:OCTESSERA_FAKE_SSH_PROBE = "failed"
  $probeFailure = Invoke-BringupForTest -Parameters $failureParameters
  $probeFailureText = ($probeFailure.Output | ForEach-Object { [string]$_ }) -join "`n"
  if (-not $probeFailure.Threw -or $probeFailureText -notmatch "Could not read remote latest-log-path" -or $probeFailureText -notmatch "remote bring-up probe failed with exit code 17") {
    throw "empty log lookup did not preserve warning and original probe failure"
  }
  if ($probeFailureText -match "null-valued expression|Trim") { throw "empty log lookup still attempted an unsafe Trim" }
} finally {
  Remove-Item Env:\OCTESSERA_FAKE_TRANSPORT_LOG -ErrorAction SilentlyContinue
  Remove-Item Env:\OCTESSERA_FAKE_SSH_LOG_LOOKUP -ErrorAction SilentlyContinue
  Remove-Item Env:\OCTESSERA_FAKE_SSH_PROBE -ErrorAction SilentlyContinue
  Remove-Item Env:\OCTESSERA_FAKE_SCP_FAILURE_SOURCE -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Output "Orange Pi bring-up wrapper local transport tests passed"
