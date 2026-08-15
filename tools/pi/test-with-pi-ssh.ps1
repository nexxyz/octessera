$ErrorActionPreference = "Stop"

$scriptPath = Join-Path $PSScriptRoot "with-pi-ssh.ps1"
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("octessera-pi-ssh-test-" + [guid]::NewGuid().ToString("N"))
$fakeBin = Join-Path $testRoot "bin"
$userProfile = Join-Path $testRoot "user profile"
$sshDirectory = Join-Path $userProfile ".ssh"
$recordPath = Join-Path $testRoot "transport.json"
$payloadPath = Join-Path $testRoot "remote payload.sh"
$oldPath = $env:PATH
$oldUserProfile = $env:USERPROFILE
$oldPassphrase = $env:OCTESSERA_PI_PASSPHRASE
$oldAskPass = $env:SSH_ASKPASS
$oldAskPassRequire = $env:SSH_ASKPASS_REQUIRE
$oldDisplay = $env:DISPLAY
$encoding = New-Object System.Text.UTF8Encoding($false)

function Write-Utf8NoBom {
  param(
    [string]$Path,
    [string]$Contents
  )

  [IO.File]::WriteAllText($Path, $Contents, $encoding)
}

function Invoke-Wrapper {
  param([object[]]$Arguments)

  $threw = $false
  try {
    $output = @(& $scriptPath @Arguments 2>&1)
  } catch {
    $threw = $true
    $output = @($_)
  }
  [pscustomobject]@{
    ExitCode = if ($threw) { 1 } else { $LASTEXITCODE }
    Threw = $threw
    Output = $output
  }
}

function Get-TransportRecord {
  Get-Content -LiteralPath $recordPath -Raw | ConvertFrom-Json
}

function Assert-EqualText {
  param(
    [string]$Actual,
    [string]$Expected,
    [string]$Message
  )

  if ($Actual -cne $Expected) {
    throw $Message
  }
}

try {
  New-Item -ItemType Directory -Path $sshDirectory, $fakeBin -Force | Out-Null
  Write-Utf8NoBom (Join-Path $sshDirectory "octessera_pi_dev") "fake private key"
  Write-Utf8NoBom (Join-Path $sshDirectory "known_hosts") "192.168.0.218 ssh-ed25519 fake"

  Write-Utf8NoBom (Join-Path $fakeBin "record-transport.ps1") @'
$arguments = @($args | Select-Object -Skip 1)
$record = [ordered]@{
  tool = [string]$args[0]
  arguments = $arguments
  askPass = $env:SSH_ASKPASS
  askPassRequire = $env:SSH_ASKPASS_REQUIRE
  display = $env:DISPLAY
  helperExists = Test-Path -LiteralPath $env:SSH_ASKPASS -PathType Leaf
  helperContainsPassphrase = $false
  askPassMatches = $false
  stdin = $null
}
$isPayload = $record.tool -eq "ssh" -and @($record.arguments)[-1] -ceq "tr -d '\r' | base64 --decode | bash -s --"
if ($isPayload) {
  $record.stdin = [Console]::In.ReadToEnd()
}
$helperText = [IO.File]::ReadAllText($env:SSH_ASKPASS)
$record.helperContainsPassphrase = $helperText.Contains($env:OCTESSERA_EXPECTED_ASKPASS)
$askPassOutput = @(& $env:SSH_ASKPASS test-prompt)
$record.askPassMatches = $askPassOutput.Count -eq 1 -and [string]$askPassOutput[0] -ceq $env:OCTESSERA_EXPECTED_ASKPASS
[IO.File]::WriteAllText($env:OCTESSERA_PI_SSH_RECORD, ($record | ConvertTo-Json -Compress))
$exitCode = if ($null -eq $env:OCTESSERA_PI_SSH_EXIT_CODE) { 0 } else { [int]$env:OCTESSERA_PI_SSH_EXIT_CODE }
if ($null -ne $env:OCTESSERA_PI_SSH_STDERR) {
  [Console]::Error.WriteLine($env:OCTESSERA_PI_SSH_STDERR)
}
exit $exitCode
'@
  Write-Utf8NoBom (Join-Path $fakeBin "ssh.cmd") @'
@echo off
powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0record-transport.ps1" ssh %*
exit /b %ERRORLEVEL%
'@
  Write-Utf8NoBom (Join-Path $fakeBin "scp.cmd") @'
@echo off
powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0record-transport.ps1" scp %*
exit /b %ERRORLEVEL%
'@

  $env:PATH = "$fakeBin;$oldPath"
  $env:USERPROFILE = $userProfile
  $env:OCTESSERA_PI_PASSPHRASE = "test-only-passphrase"
  $env:OCTESSERA_EXPECTED_ASKPASS = $env:OCTESSERA_PI_PASSPHRASE
  $env:OCTESSERA_PI_SSH_RECORD = $recordPath
  Remove-Item Env:\OCTESSERA_PI_SSH_EXIT_CODE -ErrorAction SilentlyContinue
  Remove-Item Env:\OCTESSERA_PI_SSH_STDERR -ErrorAction SilentlyContinue
  $env:SSH_ASKPASS = "prior-askpass"
  $env:SSH_ASKPASS_REQUIRE = "prior-require"
  $env:DISPLAY = "prior-display"

  $defaultResult = Invoke-Wrapper @("ssh", "printf safe")
  if ($defaultResult.ExitCode -ne 0) {
    throw "Default SSH wrapper invocation failed: $($defaultResult.Output -join "`n")"
  }
  $defaultRecord = Get-TransportRecord
  $expectedDefaultArguments = @(
    "-i", (Join-Path $sshDirectory "octessera_pi_dev"),
    "-o", "IdentitiesOnly=yes",
    "-o", "UserKnownHostsFile=$(Join-Path $sshDirectory "known_hosts")",
    "-o", "StrictHostKeyChecking=yes",
    "-o", "BatchMode=no",
    "-o", "ConnectTimeout=10",
    "-o", "NumberOfPasswordPrompts=1",
    "pi@192.168.0.218",
    "printf safe"
  )
  if ((@($defaultRecord.arguments) -join "`n") -cne ($expectedDefaultArguments -join "`n")) {
    throw "Default SSH wrapper arguments did not use the exact Pi identity, host, and bounded transport options."
  }
  if (-not $defaultRecord.helperExists -or $defaultRecord.helperContainsPassphrase -or -not $defaultRecord.askPassMatches) {
    throw "SSH_ASKPASS helper did not safely read only the process environment."
  }
  if ($defaultRecord.askPassRequire -cne "force" -or $defaultRecord.display -cne "octessera") {
    throw "SSH_ASKPASS environment was not configured for the child transport."
  }
  if (Test-Path -LiteralPath $defaultRecord.askPass -PathType Leaf) {
    throw "Temporary SSH_ASKPASS helper was not removed after success."
  }
  Assert-EqualText $env:SSH_ASKPASS "prior-askpass" "SSH_ASKPASS was not restored after success."
  Assert-EqualText $env:SSH_ASKPASS_REQUIRE "prior-require" "SSH_ASKPASS_REQUIRE was not restored after success."
  Assert-EqualText $env:DISPLAY "prior-display" "DISPLAY was not restored after success."

  $explicitTarget = "pi@192.168.0.219"
  $explicitOutput = @(& $scriptPath -Mode ssh -Target $explicitTarget "printf explicit" 2>&1)
  if ($LASTEXITCODE -ne 0) {
    throw "Explicit target parameter invocation failed: $($explicitOutput -join "`n")"
  }
  $explicitRecord = Get-TransportRecord
  if (-not (@($explicitRecord.arguments) -contains $explicitTarget)) {
    throw "Explicit target parameter was not passed to the child transport."
  }

  $target = "pi@192.168.0.218"
  $scpDestination = "$target`:/tmp/candidate"
  $scpResult = Invoke-Wrapper @("scp", "candidate.bin", $scpDestination)
  if ($scpResult.ExitCode -ne 0) {
    throw "SCP wrapper invocation failed: $($scpResult.Output -join "`n")"
  }
  $scpRecord = Get-TransportRecord
  $expectedUploadArguments = @(
    "-i", (Join-Path $sshDirectory "octessera_pi_dev"),
    "-o", "IdentitiesOnly=yes",
    "-o", "UserKnownHostsFile=$(Join-Path $sshDirectory "known_hosts")",
    "-o", "StrictHostKeyChecking=yes",
    "-o", "BatchMode=no",
    "-o", "ConnectTimeout=10",
    "-o", "NumberOfPasswordPrompts=1",
    "candidate.bin",
    $scpDestination
  )
  if ($scpRecord.tool -cne "scp" -or (@($scpRecord.arguments) -join "`n") -cne ($expectedUploadArguments -join "`n")) {
    throw "SCP upload wrapper argv did not contain exactly the fixed options, source, and remote destination."
  }

  $scpSource = "$target`:/tmp/candidate"
  $scpDownloadResult = Invoke-Wrapper @("scp", $scpSource, "download.bin")
  if ($scpDownloadResult.ExitCode -ne 0) {
    throw "SCP download wrapper invocation failed: $($scpDownloadResult.Output -join "`n")"
  }
  $scpDownloadRecord = Get-TransportRecord
  $expectedDownloadArguments = @(
    "-i", (Join-Path $sshDirectory "octessera_pi_dev"),
    "-o", "IdentitiesOnly=yes",
    "-o", "UserKnownHostsFile=$(Join-Path $sshDirectory "known_hosts")",
    "-o", "StrictHostKeyChecking=yes",
    "-o", "BatchMode=no",
    "-o", "ConnectTimeout=10",
    "-o", "NumberOfPasswordPrompts=1",
    $scpSource,
    "download.bin"
  )
  if ($scpDownloadRecord.tool -cne "scp" -or (@($scpDownloadRecord.arguments) -join "`n") -cne ($expectedDownloadArguments -join "`n")) {
    throw "SCP download wrapper argv did not contain exactly the fixed options, remote source, and destination."
  }

  $payloadContents = @(
    'set -eu'
    'remote_value=''$(must stay remote)'''
    'printf ''%s\n'' "$remote_value"'
  ) -join "`r`n"
  $payloadContents += "`r`n"
  [IO.File]::WriteAllText($payloadPath, $payloadContents, $encoding)
  $payloadOutput = @(& $scriptPath "ssh-payload" -Target $target $payloadPath 2>&1)
  if ($LASTEXITCODE -ne 0) {
    throw "SSH payload wrapper invocation failed: $($payloadOutput -join "`n")"
  }
  $payloadRecord = Get-TransportRecord
  $expectedPayloadArguments = @(
    "-i", (Join-Path $sshDirectory "octessera_pi_dev"),
    "-o", "IdentitiesOnly=yes",
    "-o", "UserKnownHostsFile=$(Join-Path $sshDirectory "known_hosts")",
    "-o", "StrictHostKeyChecking=yes",
    "-o", "BatchMode=no",
    "-o", "ConnectTimeout=10",
    "-o", "NumberOfPasswordPrompts=1",
    $target,
    "tr -d '\r' | base64 --decode | bash -s --"
  )
  if ((@($payloadRecord.arguments) -join "`n") -cne ($expectedPayloadArguments -join "`n")) {
    throw "SSH payload wrapper did not use the fixed remote decoder command."
  }
  $decodedPayload = [Convert]::FromBase64String(([string]$payloadRecord.stdin -replace "\s", ""))
  $expectedPayloadBytes = [Text.Encoding]::UTF8.GetBytes($payloadContents)
  if ([Convert]::ToBase64String($decodedPayload) -cne [Convert]::ToBase64String($expectedPayloadBytes)) {
    throw "SSH payload contents were changed or PowerShell-expanded before transport."
  }

  $env:OCTESSERA_PI_SSH_EXIT_CODE = "23"
  $failureResult = Invoke-Wrapper @("ssh", "printf failure")
  if ($failureResult.ExitCode -ne 23) {
    throw "SSH wrapper did not preserve a mocked transport failure exit code."
  }
  $failureRecord = Get-TransportRecord
  if (Test-Path -LiteralPath $failureRecord.askPass -PathType Leaf) {
    throw "Temporary SSH_ASKPASS helper was not removed after failure."
  }
  Assert-EqualText $env:SSH_ASKPASS "prior-askpass" "SSH_ASKPASS was not restored after failure."
  Assert-EqualText $env:SSH_ASKPASS_REQUIRE "prior-require" "SSH_ASKPASS_REQUIRE was not restored after failure."
  Assert-EqualText $env:DISPLAY "prior-display" "DISPLAY was not restored after failure."
  Remove-Item Env:\OCTESSERA_PI_SSH_EXIT_CODE -ErrorAction SilentlyContinue

  $timeoutMessage = "ssh: connect to host 192.168.0.218 port 22: Connection timed out"
  $env:OCTESSERA_PI_SSH_EXIT_CODE = "42"
  $env:OCTESSERA_PI_SSH_STDERR = $timeoutMessage
  $timeoutResult = Invoke-Wrapper @("ssh", "printf timeout")
  if ($timeoutResult.Threw -or $timeoutResult.ExitCode -ne 42 -or ($timeoutResult.Output -join "`n") -notlike "*$timeoutMessage*") {
    throw "SSH connection-timeout stderr did not return normally with the exact native exit code."
  }
  $timeoutRecord = Get-TransportRecord
  if (Test-Path -LiteralPath $timeoutRecord.askPass -PathType Leaf) {
    throw "Temporary SSH_ASKPASS helper was not removed after connection timeout."
  }
  Assert-EqualText $env:SSH_ASKPASS "prior-askpass" "SSH_ASKPASS was not restored after connection timeout."
  Assert-EqualText $env:SSH_ASKPASS_REQUIRE "prior-require" "SSH_ASKPASS_REQUIRE was not restored after connection timeout."
  Assert-EqualText $env:DISPLAY "prior-display" "DISPLAY was not restored after connection timeout."
  Remove-Item Env:\OCTESSERA_PI_SSH_EXIT_CODE -ErrorAction SilentlyContinue
  Remove-Item Env:\OCTESSERA_PI_SSH_STDERR -ErrorAction SilentlyContinue

  Write-Utf8NoBom $recordPath ""
  Remove-Item Env:\OCTESSERA_PI_PASSPHRASE -ErrorAction SilentlyContinue
  $missingPassphraseResult = Invoke-Wrapper @("ssh", "printf missing-passphrase")
  if ($missingPassphraseResult.ExitCode -eq 0 -or (Get-Item -LiteralPath $recordPath).Length -ne 0) {
    throw "SSH wrapper allowed transport without the process passphrase environment."
  }
  $env:OCTESSERA_PI_PASSPHRASE = "test-only-passphrase"

  Write-Utf8NoBom $recordPath ""
  $unsafeResult = Invoke-Wrapper @("ssh", "-i", "other-key", $target, "printf unsafe")
  if ($unsafeResult.ExitCode -eq 0 -or (Get-Item -LiteralPath $recordPath).Length -ne 0) {
    throw "Unsafe SSH identity arguments were not rejected before transport."
  }

  Write-Utf8NoBom $recordPath ""
  $wrongTargetResult = Invoke-Wrapper @("ssh", "pi@192.168.0.219", "printf unsafe")
  if ($wrongTargetResult.ExitCode -eq 0 -or (Get-Item -LiteralPath $recordPath).Length -ne 0) {
    throw "A mismatched SSH target was not rejected before transport."
  }

  Write-Utf8NoBom $recordPath ""
  $wrongScpResult = Invoke-Wrapper @("scp", "candidate.bin", "pi@192.168.0.219:/tmp/unsafe")
  if ($wrongScpResult.ExitCode -eq 0 -or (Get-Item -LiteralPath $recordPath).Length -ne 0) {
    throw "A mismatched SCP target was not rejected before transport."
  }
} finally {
  $env:PATH = $oldPath
  if ($null -eq $oldUserProfile) { Remove-Item Env:\USERPROFILE -ErrorAction SilentlyContinue } else { $env:USERPROFILE = $oldUserProfile }
  if ($null -eq $oldPassphrase) { Remove-Item Env:\OCTESSERA_PI_PASSPHRASE -ErrorAction SilentlyContinue } else { $env:OCTESSERA_PI_PASSPHRASE = $oldPassphrase }
  if ($null -eq $oldAskPass) { Remove-Item Env:\SSH_ASKPASS -ErrorAction SilentlyContinue } else { $env:SSH_ASKPASS = $oldAskPass }
  if ($null -eq $oldAskPassRequire) { Remove-Item Env:\SSH_ASKPASS_REQUIRE -ErrorAction SilentlyContinue } else { $env:SSH_ASKPASS_REQUIRE = $oldAskPassRequire }
  if ($null -eq $oldDisplay) { Remove-Item Env:\DISPLAY -ErrorAction SilentlyContinue } else { $env:DISPLAY = $oldDisplay }
  Remove-Item Env:\OCTESSERA_EXPECTED_ASKPASS -ErrorAction SilentlyContinue
  Remove-Item Env:\OCTESSERA_PI_SSH_RECORD -ErrorAction SilentlyContinue
  Remove-Item Env:\OCTESSERA_PI_SSH_EXIT_CODE -ErrorAction SilentlyContinue
  Remove-Item Env:\OCTESSERA_PI_SSH_STDERR -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Output "Pi SSH wrapper syntax, transport, payload, cleanup, and argument-safety tests passed"
