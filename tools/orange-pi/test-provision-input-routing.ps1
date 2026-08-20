$ErrorActionPreference = "Stop"

$scriptPath = Join-Path $PSScriptRoot "provision-input-routing.ps1"
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("octessera-input-routing-auth-test-" + [guid]::NewGuid().ToString("N"))
$fakeBin = Join-Path $testRoot "fake bin"
$keyPath = Join-Path $testRoot "orange-key"
$knownHostsPath = Join-Path $testRoot "known_hosts"
$recordPath = Join-Path $testRoot "transport.jsonl"
$oldPath = $env:PATH
$oldPassphrase = $env:OCTESSERA_PI_PASSPHRASE
$oldAskPass = $env:SSH_ASKPASS
$oldAskPassRequire = $env:SSH_ASKPASS_REQUIRE
$oldDisplay = $env:DISPLAY
$encoding = New-Object System.Text.UTF8Encoding($false)

function Write-Utf8NoBom {
  param([string]$Path, [string]$Contents)
  [IO.File]::WriteAllText($Path, $Contents, $encoding)
}

function Read-Records {
  return @([IO.File]::ReadAllLines($recordPath) | ForEach-Object { $_ | ConvertFrom-Json })
}

function Invoke-Provisioner {
  param([bool]$WithPassphrase)
  [IO.File]::WriteAllText($recordPath, "", $encoding)
  if ($WithPassphrase) {
    $env:OCTESSERA_PI_PASSPHRASE = "test-only-passphrase"
  } else {
    Remove-Item Env:\OCTESSERA_PI_PASSPHRASE -ErrorAction SilentlyContinue
  }
  $output = @(& $scriptPath -Target "octessera@test.invalid" -Key $keyPath -KnownHosts $knownHostsPath -Preflight 2>&1)
  if ($LASTEXITCODE -ne 0) {
    throw "Provisioner invocation failed: $($output -join "`n")"
  }
  return Read-Records
}

function Assert-TransportPolicy {
  param(
    [object[]]$Records,
    [bool]$WithPassphrase
  )
  if ($Records.Count -ne 10) {
    throw "Expected one SSH setup, seven SCP uploads, one SSH provision, and one SSH cleanup call; got $($Records.Count)."
  }
  $provisionArguments = @(
    $Records |
      Where-Object { $_.tool -eq "ssh" } |
      ForEach-Object { @($_.arguments) } |
      Where-Object { $_ -match "--armbian-environment-script" }
  )
  if ($provisionArguments.Count -ne 1) {
    throw "Input-routing SSH provision did not receive the Armbian environment script exactly once."
  }
  foreach ($record in $Records) {
    $arguments = @($record.arguments)
    if ($arguments -notcontains "-i" -or $arguments -notcontains $keyPath -or $arguments -notcontains "-o" -or $arguments -notcontains "IdentitiesOnly=yes" -or $arguments -notcontains "StrictHostKeyChecking=yes") {
      throw "Transport did not preserve the explicit key, IdentitiesOnly, and strict host checking policy."
    }
    if ($WithPassphrase) {
      if (-not $record.helperExists -or $record.helperContainsPassphrase -or -not $record.askPassMatches) {
        throw "Passphrase transport did not use a secret-free SSH_ASKPASS helper."
      }
      if ($arguments -notcontains "BatchMode=no") {
        throw "Passphrase transport did not disable BatchMode for SSH_ASKPASS."
      }
    } else {
      if ($record.helperExists -or $arguments -notcontains "BatchMode=yes") {
        throw "Agent transport did not retain BatchMode=yes without creating an askpass helper."
      }
    }
    if ($arguments -contains "test-only-passphrase") {
      throw "Passphrase was exposed as a transport argument."
    }
    if ($record.helperPath -and (Test-Path -LiteralPath $record.helperPath -PathType Leaf)) {
      throw "Temporary SSH_ASKPASS helper was not removed."
    }
  }
}

try {
  New-Item -ItemType Directory -Path $fakeBin -Force | Out-Null
  Write-Utf8NoBom $keyPath "fake key"
  Write-Utf8NoBom $knownHostsPath "test.invalid ssh-ed25519 fake"
  Write-Utf8NoBom (Join-Path $fakeBin "record-transport.ps1") @'
$tool = [string]$args[0]
$arguments = @($args | Select-Object -Skip 1)
$helperPath = [Environment]::GetEnvironmentVariable("SSH_ASKPASS", "Process")
$record = [ordered]@{
  tool = $tool
  arguments = $arguments
  helperPath = $helperPath
  helperExists = $false
  helperContainsPassphrase = $false
  askPassMatches = $false
  askPassRequire = [Environment]::GetEnvironmentVariable("SSH_ASKPASS_REQUIRE", "Process")
  display = [Environment]::GetEnvironmentVariable("DISPLAY", "Process")
}
if ($null -ne $helperPath -and (Test-Path -LiteralPath $helperPath -PathType Leaf)) {
  $record.helperExists = $true
  $helperText = [IO.File]::ReadAllText($helperPath)
  $expected = [Environment]::GetEnvironmentVariable("OCTESSERA_EXPECTED_ASKPASS", "Process")
  $record.helperContainsPassphrase = $helperText.Contains($expected)
  $askPassOutput = @(& $helperPath test-prompt)
  $record.askPassMatches = $askPassOutput.Count -eq 1 -and [string]$askPassOutput[0] -ceq $expected
}
[IO.File]::AppendAllText($env:OCTESSERA_INPUT_ROUTING_RECORD, ($record | ConvertTo-Json -Compress) + [Environment]::NewLine)
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
  $env:OCTESSERA_INPUT_ROUTING_RECORD = $recordPath
  $env:OCTESSERA_EXPECTED_ASKPASS = "test-only-passphrase"
  Remove-Item Env:\SSH_ASKPASS -ErrorAction SilentlyContinue
  Remove-Item Env:\SSH_ASKPASS_REQUIRE -ErrorAction SilentlyContinue
  Remove-Item Env:\DISPLAY -ErrorAction SilentlyContinue

  Assert-TransportPolicy (Invoke-Provisioner $true) $true
  if ($env:SSH_ASKPASS -or $env:SSH_ASKPASS_REQUIRE -or $env:DISPLAY) {
    throw "SSH_ASKPASS environment was not restored after passphrase transport."
  }
  Assert-TransportPolicy (Invoke-Provisioner $false) $false
} finally {
  $env:PATH = $oldPath
  if ($null -eq $oldPassphrase) { Remove-Item Env:\OCTESSERA_PI_PASSPHRASE -ErrorAction SilentlyContinue } else { $env:OCTESSERA_PI_PASSPHRASE = $oldPassphrase }
  if ($null -eq $oldAskPass) { Remove-Item Env:\SSH_ASKPASS -ErrorAction SilentlyContinue } else { $env:SSH_ASKPASS = $oldAskPass }
  if ($null -eq $oldAskPassRequire) { Remove-Item Env:\SSH_ASKPASS_REQUIRE -ErrorAction SilentlyContinue } else { $env:SSH_ASKPASS_REQUIRE = $oldAskPassRequire }
  if ($null -eq $oldDisplay) { Remove-Item Env:\DISPLAY -ErrorAction SilentlyContinue } else { $env:DISPLAY = $oldDisplay }
  Remove-Item Env:\OCTESSERA_INPUT_ROUTING_RECORD -ErrorAction SilentlyContinue
  Remove-Item Env:\OCTESSERA_EXPECTED_ASKPASS -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Output "Orange input-routing SSH authentication and cleanup tests passed"
