$ErrorActionPreference = "Stop"

$scriptPath = Join-Path $PSScriptRoot "with-opi-ssh.ps1"
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("octessera-orange-ssh-test-" + [guid]::NewGuid().ToString("N"))
$fakeBin = Join-Path $testRoot "bin"
$userProfile = Join-Path $testRoot "user profile"
$sshDirectory = Join-Path $userProfile ".ssh"
$recordPath = Join-Path $testRoot "transport.json"
$oldPath = $env:PATH
$oldUserProfile = $env:USERPROFILE
$oldPassphrase = $env:OCTESSERA_PI_PASSPHRASE
$oldAskPass = $env:SSH_ASKPASS
$oldAskPassRequire = $env:SSH_ASKPASS_REQUIRE
$oldDisplay = $env:DISPLAY
$oldConcurrentRecordDir = $env:OCTESSERA_ORANGE_SSH_RECORD_DIR
$oldConcurrentBarrierDir = $env:OCTESSERA_ORANGE_SSH_BARRIER_DIR
$payloadPath = Join-Path $testRoot "remote payload.sh"
$concurrentRecordDir = Join-Path $testRoot "concurrent records"
$concurrentBarrierDir = Join-Path $testRoot "concurrent barrier"
$staleBasePath = $null
$staleSiblingPath = $null
$concurrentProcesses = @()
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
    if ($Arguments.Count -gt 0 -and [string]$Arguments[0] -ceq "-Command") {
      $command = [string]$Arguments[1]
      $target = [string]$Arguments[3]
      $remaining = @($Arguments | Select-Object -Skip 4)
      $output = @(& $scriptPath -Command $command -Target $target @remaining 2>&1)
    } else {
      $command = [string]$Arguments[0]
      $remaining = @($Arguments | Select-Object -Skip 1)
      $output = @(& $scriptPath -Command $command @remaining 2>&1)
    }
  } catch {
    $threw = $true
    $output = @($_)
  }
  [pscustomobject]@{
    ExitCode = if ($threw) { 1 } else { $LASTEXITCODE }
    Output = $output
  }
}

try {
  New-Item -ItemType Directory -Path $sshDirectory -Force | Out-Null
  New-Item -ItemType Directory -Path $fakeBin -Force | Out-Null
  Write-Utf8NoBom (Join-Path $sshDirectory "octessera_orange_pi_ed25519") "fake private key"
  Write-Utf8NoBom (Join-Path $sshDirectory "known_hosts") "orange.test.invalid ssh-ed25519 fake"

  Write-Utf8NoBom (Join-Path $fakeBin "record-transport.ps1") @'
$recordPath = $env:OCTESSERA_ORANGE_SSH_RECORD
if ($null -ne $env:OCTESSERA_ORANGE_SSH_RECORD_DIR) {
  $markerPath = Join-Path $env:OCTESSERA_ORANGE_SSH_BARRIER_DIR (([guid]::NewGuid().ToString("N")) + ".ready")
  [IO.File]::WriteAllText($markerPath, "ready")
  $deadline = [DateTime]::UtcNow.AddSeconds(10)
  while ([IO.Directory]::GetFiles($env:OCTESSERA_ORANGE_SSH_BARRIER_DIR, "*.ready").Count -lt 2 -and [DateTime]::UtcNow -lt $deadline) {
    Start-Sleep -Milliseconds 10
  }
  if ([IO.Directory]::GetFiles($env:OCTESSERA_ORANGE_SSH_BARRIER_DIR, "*.ready").Count -lt 2) {
    exit 91
  }
  $recordPath = Join-Path $env:OCTESSERA_ORANGE_SSH_RECORD_DIR (([guid]::NewGuid().ToString("N")) + ".json")
}
$record = [ordered]@{
  tool = [string]$args[0]
  arguments = @($args | Select-Object -Skip 1)
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
[IO.File]::WriteAllText($recordPath, ($record | ConvertTo-Json -Compress))
exit 0
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
  $env:OCTESSERA_ORANGE_SSH_RECORD = $recordPath
  Remove-Item Env:\SSH_ASKPASS -ErrorAction SilentlyContinue
  Remove-Item Env:\SSH_ASKPASS_REQUIRE -ErrorAction SilentlyContinue
  Remove-Item Env:\DISPLAY -ErrorAction SilentlyContinue
  $staleBasePath = [IO.Path]::GetTempFileName()
  $staleSiblingPath = "$staleBasePath.cmd"
  Write-Utf8NoBom $staleSiblingPath "unrelated stale helper"

  $target = "octessera@orange.test.invalid"
  Write-Utf8NoBom $recordPath ""
  $omittedResult = Invoke-Wrapper @("ssh", "printf omitted")
  if ($omittedResult.ExitCode -eq 0 -or (Get-Item -LiteralPath $recordPath).Length -ne 0) {
    throw "SSH wrapper allowed an omitted target or reached transport."
  }
  $sshResult = Invoke-Wrapper @("-Command", "ssh", "-Target", $target, "printf safe")
  if ($sshResult.ExitCode -ne 0) {
    throw "SSH wrapper invocation failed: $($sshResult.Output -join "`n")"
  }
  $sshRecord = Get-Content -LiteralPath $recordPath -Raw | ConvertFrom-Json
  $expectedSshArguments = @(
    "-i", (Join-Path $sshDirectory "octessera_orange_pi_ed25519"),
    "-o", "IdentitiesOnly=yes",
    "-o", "UserKnownHostsFile=$(Join-Path $sshDirectory "known_hosts")",
    "-o", "StrictHostKeyChecking=yes",
    "-o", "BatchMode=no",
    $target,
    "printf safe"
  )
  if ((@($sshRecord.arguments) -join "`n") -cne ($expectedSshArguments -join "`n")) {
    throw "SSH wrapper arguments did not use the exact Orange identity and known_hosts paths."
  }
  if (-not $sshRecord.helperExists -or $sshRecord.helperContainsPassphrase -or -not $sshRecord.askPassMatches) {
    throw "SSH_ASKPASS helper did not safely read the process environment."
  }
  if ($sshRecord.askPassRequire -cne "force" -or $sshRecord.display -cne "octessera") {
    throw "SSH_ASKPASS environment was not configured for the child transport."
  }
  if (Test-Path -LiteralPath $sshRecord.askPass -PathType Leaf) {
    throw "Temporary SSH_ASKPASS helper was not removed."
  }
  if (-not (Test-Path -LiteralPath $staleSiblingPath -PathType Leaf) -or [IO.File]::ReadAllText($staleSiblingPath) -cne "unrelated stale helper") {
    throw "Wrapper touched an unrelated stale askpass sibling."
  }

  New-Item -ItemType Directory -Path $concurrentRecordDir, $concurrentBarrierDir -Force | Out-Null
  $env:OCTESSERA_ORANGE_SSH_RECORD_DIR = $concurrentRecordDir
  $env:OCTESSERA_ORANGE_SSH_BARRIER_DIR = $concurrentBarrierDir
  $concurrentProcesses = @(
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", $scriptPath, "-Command", "ssh", "-Target", $target, "printf concurrent") -PassThru -WindowStyle Hidden
    Start-Process -FilePath "powershell.exe" -ArgumentList @("-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", $scriptPath, "-Command", "ssh", "-Target", $target, "printf concurrent") -PassThru -WindowStyle Hidden
  )
  foreach ($process in $concurrentProcesses) {
    $process.WaitForExit()
    $process.Refresh()
    if ($process.ExitCode -ne 0) {
      throw "Concurrent Orange wrapper invocation failed with exit code $($process.ExitCode)."
    }
  }
  $concurrentRecords = @(Get-ChildItem -LiteralPath $concurrentRecordDir -Filter "*.json" -File)
  if ($concurrentRecords.Count -ne 2) {
    throw "Concurrent Orange wrapper invocations did not produce two transport records."
  }
  $concurrentHelperPaths = @($concurrentRecords | ForEach-Object { (Get-Content -LiteralPath $_.FullName -Raw | ConvertFrom-Json).askPass })
  if (@($concurrentHelperPaths | Select-Object -Unique).Count -ne 2) {
    throw "Concurrent Orange wrapper invocations did not create distinct askpass helpers."
  }
  foreach ($concurrentHelperPath in $concurrentHelperPaths) {
    if (Test-Path -LiteralPath $concurrentHelperPath -PathType Leaf) {
      throw "A concurrent Orange askpass helper was not removed."
    }
  }
  $concurrentProcesses = @()
  if ($null -eq $oldConcurrentRecordDir) { Remove-Item Env:\OCTESSERA_ORANGE_SSH_RECORD_DIR -ErrorAction SilentlyContinue } else { $env:OCTESSERA_ORANGE_SSH_RECORD_DIR = $oldConcurrentRecordDir }
  if ($null -eq $oldConcurrentBarrierDir) { Remove-Item Env:\OCTESSERA_ORANGE_SSH_BARRIER_DIR -ErrorAction SilentlyContinue } else { $env:OCTESSERA_ORANGE_SSH_BARRIER_DIR = $oldConcurrentBarrierDir }
  if ($null -ne $oldAskPass) { $env:SSH_ASKPASS = $oldAskPass }
  if ($null -ne $oldAskPassRequire) { $env:SSH_ASKPASS_REQUIRE = $oldAskPassRequire }
  if ($null -ne $oldDisplay) { $env:DISPLAY = $oldDisplay }

  $scpDestination = $target + ":/tmp/candidate"
  $scpResult = Invoke-Wrapper @("-Command", "scp", "-Target", $target, "candidate.bin", $scpDestination)
  if ($scpResult.ExitCode -ne 0) {
    throw "SCP wrapper invocation failed: $($scpResult.Output -join "`n")"
  }
  $scpRecord = Get-Content -LiteralPath $recordPath -Raw | ConvertFrom-Json
  $expectedScpUploadArguments = @(
    "-i", (Join-Path $sshDirectory "octessera_orange_pi_ed25519"),
    "-o", "IdentitiesOnly=yes",
    "-o", "UserKnownHostsFile=$(Join-Path $sshDirectory "known_hosts")",
    "-o", "StrictHostKeyChecking=yes",
    "-o", "BatchMode=no",
    "candidate.bin",
    $scpDestination
  )
  if ($scpRecord.tool -cne "scp" -or (@($scpRecord.arguments) -join "`n") -cne ($expectedScpUploadArguments -join "`n")) {
    throw "SCP upload wrapper arguments did not preserve the exact ordered transport operands."
  }
  $scpDownloadPath = Join-Path $testRoot "downloaded candidate.bin"
  $scpDownloadResult = Invoke-Wrapper @("-Command", "scp", "-Target", $target, $scpDestination, $scpDownloadPath)
  if ($scpDownloadResult.ExitCode -ne 0) {
    throw "SCP download wrapper invocation failed: $($scpDownloadResult.Output -join "`n")"
  }
  $scpDownloadRecord = Get-Content -LiteralPath $recordPath -Raw | ConvertFrom-Json
  $expectedScpDownloadArguments = @(
    "-i", (Join-Path $sshDirectory "octessera_orange_pi_ed25519"),
    "-o", "IdentitiesOnly=yes",
    "-o", "UserKnownHostsFile=$(Join-Path $sshDirectory "known_hosts")",
    "-o", "StrictHostKeyChecking=yes",
    "-o", "BatchMode=no",
    $scpDestination,
    $scpDownloadPath
  )
  if ($scpDownloadRecord.tool -cne "scp" -or (@($scpDownloadRecord.arguments) -join "`n") -cne ($expectedScpDownloadArguments -join "`n")) {
    throw "SCP download wrapper arguments did not preserve the exact ordered transport operands."
  }
  Write-Utf8NoBom $recordPath ""
  $wrongScpTargetResult = Invoke-Wrapper @("-Command", "scp", "-Target", $target, "candidate.bin", "octessera@other.test.invalid:/tmp/candidate")
  if ($wrongScpTargetResult.ExitCode -eq 0 -or (Get-Item -LiteralPath $recordPath).Length -ne 0) {
    throw "SCP wrapper accepted a remote operand for a different target."
  }

  $payloadContents = @'
set -eu
remote_value='$(must stay remote)'
printf '%s\n' "$remote_value"
'@
  Write-Utf8NoBom $payloadPath $payloadContents
  $payloadResult = Invoke-Wrapper @("-Command", "ssh-payload", "-Target", $target, $payloadPath)
  if ($payloadResult.ExitCode -ne 0) {
    throw "SSH payload wrapper invocation failed: $($payloadResult.Output -join "`n")"
  }
  $payloadRecord = Get-Content -LiteralPath $recordPath -Raw | ConvertFrom-Json
  $payloadArguments = @(
    "-i", (Join-Path $sshDirectory "octessera_orange_pi_ed25519"),
    "-o", "IdentitiesOnly=yes",
    "-o", "UserKnownHostsFile=$(Join-Path $sshDirectory "known_hosts")",
    "-o", "StrictHostKeyChecking=yes",
    "-o", "BatchMode=no",
    $target,
    "tr -d '\r' | base64 --decode | bash -s --"
  )
  if ((@($payloadRecord.arguments) -join "`n") -cne ($payloadArguments -join "`n")) {
    throw "SSH payload wrapper did not use the fixed remote decoder command."
  }
  $payloadText = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String(([string]$payloadRecord.stdin -replace "\s", "")))
  if ($payloadText -cne $payloadContents) {
    throw "SSH payload contents were changed or PowerShell-expanded before transport."
  }
  Write-Utf8NoBom $recordPath ""
  $wrongPayloadTargetResult = Invoke-Wrapper @("-Command", "ssh-payload", "-Target", $target, "octessera@other.test.invalid", $payloadPath)
  if ($wrongPayloadTargetResult.ExitCode -eq 0 -or (Get-Item -LiteralPath $recordPath).Length -ne 0) {
    throw "SSH payload wrapper accepted a different explicit target."
  }

  Write-Utf8NoBom $recordPath ""
  $unsafeResult = Invoke-Wrapper @("-Command", "ssh", "-Target", $target, "-i", "other-key", "printf unsafe")
  if ($unsafeResult.ExitCode -eq 0 -or (Get-Item -LiteralPath $recordPath).Length -ne 0) {
    throw "Unsafe SSH identity arguments were not rejected before transport."
  }

  Write-Utf8NoBom $recordPath ""
  $wrongTargetResult = Invoke-Wrapper @("-Command", "ssh", "-Target", $target, "octessera@other.test.invalid", "printf unsafe")
  if ($wrongTargetResult.ExitCode -eq 0 -or (Get-Item -LiteralPath $recordPath).Length -ne 0) {
    throw "A non-Orange target was not rejected before transport."
  }
} finally {
  foreach ($process in $concurrentProcesses) {
    if (-not $process.HasExited) { $process.Kill() }
  }
  $env:PATH = $oldPath
  if ($null -eq $oldUserProfile) { Remove-Item Env:\USERPROFILE -ErrorAction SilentlyContinue } else { $env:USERPROFILE = $oldUserProfile }
  if ($null -eq $oldPassphrase) { Remove-Item Env:\OCTESSERA_PI_PASSPHRASE -ErrorAction SilentlyContinue } else { $env:OCTESSERA_PI_PASSPHRASE = $oldPassphrase }
  if ($null -eq $oldAskPass) { Remove-Item Env:\SSH_ASKPASS -ErrorAction SilentlyContinue } else { $env:SSH_ASKPASS = $oldAskPass }
  if ($null -eq $oldAskPassRequire) { Remove-Item Env:\SSH_ASKPASS_REQUIRE -ErrorAction SilentlyContinue } else { $env:SSH_ASKPASS_REQUIRE = $oldAskPassRequire }
  if ($null -eq $oldDisplay) { Remove-Item Env:\DISPLAY -ErrorAction SilentlyContinue } else { $env:DISPLAY = $oldDisplay }
  Remove-Item Env:\OCTESSERA_EXPECTED_ASKPASS -ErrorAction SilentlyContinue
  Remove-Item Env:\OCTESSERA_ORANGE_SSH_RECORD -ErrorAction SilentlyContinue
  if ($null -eq $oldConcurrentRecordDir) { Remove-Item Env:\OCTESSERA_ORANGE_SSH_RECORD_DIR -ErrorAction SilentlyContinue } else { $env:OCTESSERA_ORANGE_SSH_RECORD_DIR = $oldConcurrentRecordDir }
  if ($null -eq $oldConcurrentBarrierDir) { Remove-Item Env:\OCTESSERA_ORANGE_SSH_BARRIER_DIR -ErrorAction SilentlyContinue } else { $env:OCTESSERA_ORANGE_SSH_BARRIER_DIR = $oldConcurrentBarrierDir }
  if ($null -ne $staleSiblingPath) { Remove-Item -LiteralPath $staleSiblingPath -Force -ErrorAction SilentlyContinue }
  if ($null -ne $staleBasePath) { Remove-Item -LiteralPath $staleBasePath -Force -ErrorAction SilentlyContinue }
  Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Output "Orange SSH wrapper syntax, transport, cleanup, and argument-safety tests passed"
