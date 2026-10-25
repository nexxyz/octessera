$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "..\deployment-target.ps1")

foreach ($validTarget in @("pi@192.0.2.211", "octessera@octessera.local", "deploy@pi-zero-2w")) {
  if ((Assert-DeploymentTarget $validTarget) -cne $validTarget) {
    throw "Deployment target validator changed a valid target: $validTarget"
  }
}
foreach ($invalidTarget in @(
    "", "pi@host`nnext", "pi@host`0", "pi@host name", "pi@host`tname", "-oProxyCommand=",
    "-oProxyCommand=touch /tmp/pwned", "pi@-oProxyCommand=touch", "pi@host;echo unsafe", "pi@one@two",
    "pi@@host", "@host", "pi@", "bad!user@host", "pi@host/var", "pi@host:22", "pi@host..local",
    "pi@1.2.3", "pi@999.1.1.1", "pi@256.256.256.256"
  )) {
  $rejected = $false
  try { Assert-DeploymentTarget $invalidTarget | Out-Null } catch { $rejected = $true }
  if (-not $rejected) { throw "Deployment target validator accepted: $invalidTarget" }
}

$source = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "deploy-rpi-fast.ps1"))
$temp = Join-Path ([IO.Path]::GetTempPath()) ("octessera-rpi-deploy-test-" + [guid]::NewGuid().ToString("N"))
$encoding = New-Object System.Text.UTF8Encoding($false)
New-Item -ItemType Directory -Path $temp | Out-Null
try {
  $binary = Join-Path $temp "binary with spaces"
  [IO.File]::WriteAllText($binary, "fake binary", $encoding)
  $log = Join-Path $temp "calls.txt"
  $fake = Join-Path $temp "fake-transport.ps1"
  [IO.File]::WriteAllText($fake, @'
param([string]$Command, [string]$Board, [string]$Target, [Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest)
[IO.File]::AppendAllText($env:OCTESSERA_DEPLOY_TEST_LOG, "$Command $Board $Target $($Rest[-1])`n")
if ($Command -eq 'ssh-payload') {
  [IO.File]::AppendAllText($env:OCTESSERA_DEPLOY_TEST_LOG, (([IO.File]::ReadAllText($Rest[-1]) -split "`n" | Select-Object -First 3) -join "`n") + "`n")
}
exit 0
'@, $encoding)
  $transportLine = '$transport = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\board-dev\with-board-ssh.ps1")).Path'
  if (-not $source.Contains($transportLine)) { throw "test could not replace the transport binding" }
  $rootLiteral = "'" + $PSScriptRoot.Replace("'", "''") + "'"
  $copySource = $source.Replace($transportLine, "`$transport = '$($fake.Replace("'", "''"))'").Replace('$PSScriptRoot', $rootLiteral)
  $copy = Join-Path $temp "deploy-test.ps1"
  [IO.File]::WriteAllText($copy, $copySource, $encoding)
  $env:OCTESSERA_DEPLOY_TEST_LOG = $log

  foreach ($case in @(
      @{ Target = "pi@board" },
      @{ Target = "pi@board"; LocalBinary = $binary; Restore = $true },
      @{ Target = "pi@board"; SyncOnly = $true; Restore = $true },
      @{ Target = "pi@board"; LocalBinary = $binary; RemoteRepo = "relative" },
      @{ Target = "pi@host;echo unsafe"; LocalBinary = $binary }
    )) {
    $rejected = $false
    try { & $copy @case *> $null } catch { $rejected = $true }
    if (-not $rejected -or (Test-Path -LiteralPath $log)) { throw "Accepted $($case | ConvertTo-Json -Compress)" }
  }

  & $copy -Target "pi@board" -LocalBinary $binary -NoTail *> $null
  $hash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
  $calls = [IO.File]::ReadAllText($log) -replace "`r", ""
  $expected = "(?s)^ssh-payload rpi pi@board .*ACTION='preflight'\nBOARD_PROFILE='raspberry-pi-zero-2w'.*" +
    "scp rpi pi@board pi@board:/tmp/octessera-pi-dev\n" +
    "ssh-payload rpi pi@board .*ACTION='activate'\nBOARD_PROFILE='raspberry-pi-zero-2w'\nHASH='$hash'\n" +
    "ssh-payload rpi pi@board [^\n]*\nBOARD_PROFILE='raspberry-pi-zero-2w'\n#!/usr/bin/env bash\n# Runs on the board, piped over SSH[^\n]*\n$"
  if ($calls -notmatch $expected) { throw "Deploy did not preflight, upload, activate, and check the board in order:`n$calls" }

  [IO.File]::Delete($log)
  & $copy -Target "pi@board" -Restore *> $null
  if (([IO.File]::ReadAllText($log) -replace "`r", "") -notmatch "(?s)^ssh-payload rpi pi@board .*ACTION='restore'") {
    throw "Restore did not send the restore action."
  }
  "Raspberry development deploy tests passed"
} finally {
  [Environment]::SetEnvironmentVariable("OCTESSERA_DEPLOY_TEST_LOG", $null, "Process")
  Remove-Item -LiteralPath $temp -Recurse -Force
}
