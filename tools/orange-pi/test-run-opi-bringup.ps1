$ErrorActionPreference = "Stop"

$scriptPath = Join-Path $PSScriptRoot "run-opi-bringup.ps1"
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("octessera-opi-bringup-test-" + [guid]::NewGuid().ToString("N"))
$fakeBin = Join-Path $testRoot "fake tools"
$recordPath = Join-Path $testRoot "transport.jsonl"
$oldPath = $env:PATH
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
  if ($Record.tool -cne $Tool) {
    throw "$Label tool mismatch: expected [$Tool], got [$($Record.tool)]"
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
  Write-Utf8NoBom (Join-Path $fakeBin "record-transport.ps1") @'
$tool = [string]$args[0]
$arguments = @($args | Select-Object -Skip 1)
$record = [ordered]@{ tool = $tool; arguments = $arguments }
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
exit 0
'@
  Write-Utf8NoBom (Join-Path $fakeBin "ssh.cmd") @'
@echo off
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0record-transport.ps1" ssh %*
exit /b %ERRORLEVEL%
'@
  Write-Utf8NoBom (Join-Path $fakeBin "scp.cmd") @'
@echo off
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0record-transport.ps1" scp %*
exit /b %ERRORLEVEL%
'@

  $env:OCTESSERA_FAKE_TRANSPORT_LOG = $recordPath
  $env:PATH = "$fakeBin;$oldPath"
  $resolvedSsh = (Get-Command ssh -ErrorAction Stop).Source
  $resolvedScp = (Get-Command scp -ErrorAction Stop).Source
  if ($resolvedSsh -ne (Join-Path $fakeBin "ssh.cmd") -or $resolvedScp -ne (Join-Path $fakeBin "scp.cmd")) {
    throw "fake ssh/scp executables were not selected"
  }

  $savedErrorActionPreference = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  $missingTargetOutput = @(& (Join-Path $PSHOME "powershell.exe") -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $scriptPath 2>&1)
  $missingTargetExitCode = $LASTEXITCODE
  $ErrorActionPreference = $savedErrorActionPreference
  if ($missingTargetExitCode -eq 0 -or ($missingTargetOutput -join "`n") -notmatch "Target") {
    throw "run-opi-bringup.ps1 did not require -Target"
  }

  $target = "test-target.invalid"
  $cases = @(
    [pscustomobject]@{
      Name = "simple"
      Key = (Join-Path $testRoot "key with spaces.pem")
      RemoteScript = "/tmp/octessera-opi-probe.sh"
      RemoteLogDir = "/tmp/octessera-opi-bringup"
      ExpectedRemoteCommand = "tr -d '\r' < '/tmp/octessera-opi-probe.sh' > '/tmp/octessera-opi-probe.sh'.lf && chmod +x '/tmp/octessera-opi-probe.sh'.lf && bash '/tmp/octessera-opi-probe.sh'.lf '--output-dir' '/tmp/octessera-opi-bringup'"
      ExpectedLatestLogCommand = "cat '/tmp/octessera-opi-bringup'/latest-log-path 2>/dev/null"
    }
    [pscustomobject]@{
      Name = "quoted-shell-values"
      Key = (Join-Path $testRoot "key's & deployment.pem")
      RemoteScript = "/tmp/octessera probe's; echo NOPE & *.sh"
      RemoteLogDir = "/tmp/octessera logs/it's safe; echo NOPE &"
      ExpectedRemoteCommand = "tr -d '\r' < '/tmp/octessera probe'\''s; echo NOPE & *.sh' > '/tmp/octessera probe'\''s; echo NOPE & *.sh'.lf && chmod +x '/tmp/octessera probe'\''s; echo NOPE & *.sh'.lf && bash '/tmp/octessera probe'\''s; echo NOPE & *.sh'.lf '--output-dir' '/tmp/octessera logs/it'\''s safe; echo NOPE &'"
      ExpectedLatestLogCommand = "cat '/tmp/octessera logs/it'\''s safe; echo NOPE &'/latest-log-path 2>/dev/null"
    }
  )

  foreach ($case in $cases) {
    [IO.File]::WriteAllText($recordPath, "", $utf8NoBom)
    $localOutputDir = Join-Path $testRoot ("local output " + $case.Name)
    $null = & $scriptPath `
      -Target $target `
      -Key $case.Key `
      -RemoteScript $case.RemoteScript `
      -RemoteLogDir $case.RemoteLogDir `
      -LocalOutputDir $localOutputDir
    if (-not (Test-Path -LiteralPath $localOutputDir -PathType Container)) {
      throw "local output directory was not created for $($case.Name)"
    }

    $records = @(Read-TransportRecords)
    if ($records.Count -ne 4) {
      throw "expected two ssh and two scp calls for $($case.Name), got $($records.Count)"
    }
    $baseArguments = @(
      "-i", $case.Key,
      "-o", "IdentitiesOnly=yes",
      "-o", "StrictHostKeyChecking=yes",
      "-o", "BatchMode=yes"
    )
    $probeScriptPath = Join-Path $PSScriptRoot "opi-bringup-probe.sh"
    Assert-TransportRecord $records[0] "scp" ($baseArguments + @($probeScriptPath, "${target}:$($case.RemoteScript)")) "$($case.Name) upload"
    Assert-TransportRecord $records[1] "ssh" ($baseArguments + @($target, $case.ExpectedRemoteCommand)) "$($case.Name) probe"
    Assert-TransportRecord $records[2] "ssh" ($baseArguments + @($target, $case.ExpectedLatestLogCommand)) "$($case.Name) latest-log-path"
    Assert-TransportRecord $records[3] "scp" ($baseArguments + @("${target}:/tmp/fake bringup.log", $localOutputDir)) "$($case.Name) log download"
  }

  $probeSource = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "opi-bringup-probe.sh"))
  $wrapperSource = [IO.File]::ReadAllText($scriptPath)
  if (($probeSource + $wrapperSource) -match "gadget-midi-smoke|i-understand-usb-risk|octessera-smoke|REQUESTED_UDC|--udc") {
    throw "bring-up probe or wrapper still contains the retired inline gadget smoke"
  }

  $failureParameters = @{
    Target = $target
    Key = (Join-Path $testRoot "failure key.pem")
    RemoteScript = "/tmp/failure probe.sh"
    RemoteLogDir = "/tmp/failure logs"
    LocalOutputDir = (Join-Path $testRoot "failure output")
  }

  [IO.File]::WriteAllText($recordPath, "", $utf8NoBom)
  $env:OCTESSERA_FAKE_SSH_LOG_LOOKUP = "failed"
  $lookupFailure = Invoke-BringupForTest -Parameters $failureParameters
  $lookupFailureText = ($lookupFailure.Output | ForEach-Object { [string]$_ }) -join "`n"
  if ($lookupFailure.Threw -or $lookupFailureText -notmatch "Could not read remote latest-log-path" -or $lookupFailureText -notmatch "Orange Pi bring-up probe complete") {
    throw "failed/no-output log lookup did not preserve warning and successful probe completion"
  }
  if (@(Read-TransportRecords).Count -ne 3) { throw "failed log lookup should not attempt log download" }
  Remove-Item Env:\OCTESSERA_FAKE_SSH_LOG_LOOKUP -ErrorAction SilentlyContinue

  [IO.File]::WriteAllText($recordPath, "", $utf8NoBom)
  $env:OCTESSERA_FAKE_SSH_LOG_LOOKUP = "multiple"
  $multipleLookup = Invoke-BringupForTest -Parameters $failureParameters
  $multipleLookupText = ($multipleLookup.Output | ForEach-Object { [string]$_ }) -join "`n"
  if ($multipleLookup.Threw -or $multipleLookupText -notmatch "Could not read remote latest-log-path" -or $multipleLookupText -notmatch "Orange Pi bring-up probe complete") {
    throw "multiple-line log lookup did not preserve warning and successful probe completion"
  }
  if (@(Read-TransportRecords).Count -ne 3) { throw "multiple-line log lookup should not attempt log download" }
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
  $env:PATH = $oldPath
  Remove-Item Env:\OCTESSERA_FAKE_TRANSPORT_LOG -ErrorAction SilentlyContinue
  Remove-Item Env:\OCTESSERA_FAKE_SSH_LOG_LOOKUP -ErrorAction SilentlyContinue
  Remove-Item Env:\OCTESSERA_FAKE_SSH_PROBE -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Output "Orange Pi bring-up wrapper local transport tests passed"
