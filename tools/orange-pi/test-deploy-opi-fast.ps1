$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$source = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "deploy-opi-fast.ps1"))
$temp = Join-Path ([IO.Path]::GetTempPath()) ("octessera-opi-deploy-test-" + [guid]::NewGuid().ToString("N"))
$encoding = New-Object System.Text.UTF8Encoding($false)
New-Item -ItemType Directory -Path $temp | Out-Null
try {
  $binary = Join-Path $temp "octessera-pi"
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
      @{ Arguments = @{ Target = "pi@board"; LocalBinary = $binary }; Reason = "non-Orange target" },
      @{ Arguments = @{ Target = "octessera@board" }; Reason = "missing binary" },
      @{ Arguments = @{ Target = "octessera@board"; LocalBinary = $binary; Restore = $true }; Reason = "restore with binary" },
      @{ Arguments = @{ Target = "octessera@board"; LocalBinary = (Join-Path $temp "other") }; Reason = "missing file" }
    )) {
    $arguments = $case.Arguments
    $rejected = $false
    try { & $copy @arguments *> $null } catch { $rejected = $true }
    if (-not $rejected -or (Test-Path -LiteralPath $log)) { throw "Accepted $($case.Reason)" }
  }

  & $copy -Target "octessera@board" -LocalBinary $binary *> $null
  $hash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
  $calls = [IO.File]::ReadAllText($log) -replace "`r", ""
  $expected = "(?s)ssh-payload opi octessera@board .*ACTION='preflight'\nBOARD_PROFILE='orange-pi-zero-2w'.*" +
    "scp opi octessera@board octessera@board:/tmp/octessera-pi-dev\n" +
    "ssh-payload opi octessera@board .*ACTION='activate'\nBOARD_PROFILE='orange-pi-zero-2w'\nHASH='$hash'\n" +
    "ssh-payload opi octessera@board [^\n]*\nBOARD_PROFILE='orange-pi-zero-2w'\n[^\n]*\n[^\n]*\n$"
  if ($calls -notmatch $expected) { throw "Deploy did not preflight, upload, activate, and check the board in order:`n$calls" }

  [IO.File]::Delete($log)
  & $copy -Target "octessera@board" -Restore *> $null
  if (([IO.File]::ReadAllText($log) -replace "`r", "") -notmatch "(?s)^ssh-payload opi octessera@board .*ACTION='restore'") {
    throw "Restore did not send the restore action."
  }
  "Orange development deploy tests passed"
} finally {
  [Environment]::SetEnvironmentVariable("OCTESSERA_DEPLOY_TEST_LOG", $null, "Process")
  Remove-Item -LiteralPath $temp -Recurse -Force
}
