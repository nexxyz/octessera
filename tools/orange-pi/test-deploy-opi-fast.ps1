$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$scriptPath = Join-Path $PSScriptRoot "deploy-opi-fast.ps1"
$remotePath = Join-Path $PSScriptRoot "deploy-opi-fast-remote.sh"
$source = [IO.File]::ReadAllText($scriptPath)
$remote = [IO.File]::ReadAllText($remotePath)
$temp = Join-Path ([IO.Path]::GetTempPath()) ("octessera-opi-deploy-test-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $temp | Out-Null
try {
  $binary = Join-Path $temp "octessera-pi"
  $metadata = "$binary.metadata.json"
  [IO.File]::WriteAllBytes($binary, [byte[]](127, 69, 76, 70, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 183, 0))
  $head = (& git -C (Resolve-Path (Join-Path $PSScriptRoot "..\..")) rev-parse HEAD).Trim()
  if ($LASTEXITCODE -ne 0) { throw "git HEAD unavailable" }
  Import-Module (Join-Path $PSScriptRoot "orange-cross-metadata.psm1") -Force
  $spec = [pscustomobject]@{ Package = "octessera-pi"; Feature = "hardware-orange-pi-zero-2w"; ArtifactKind = "runtime-candidate" }
  $params = @{ MetadataPath = $metadata; BinaryPath = $binary; SelectedBinary = "octessera-pi";
    SelectedTarget = "aarch64-unknown-linux-gnu"; SelectedProfile = "release"; BuildSpec = $spec; SourceCommit = $head }
  Publish-OrangeBuildMetadata @params
  $originalMetadata = [IO.File]::ReadAllText($metadata)

  $log = Join-Path $temp "calls.txt"
  $fake = Join-Path $temp "fake-transport.ps1"
  [IO.File]::WriteAllText($fake, @'
param([string]$Command, [string]$Target, [Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest)
$log = $env:OCTESSERA_DEPLOY_TEST_LOG
[IO.File]::AppendAllText($log, "$Command $Target`n")
if ($Command -eq 'ssh-payload') {
  [IO.File]::AppendAllText($log, [IO.File]::ReadAllText($Rest[-1]) + "`nEND-PAYLOAD`n")
}
exit 0
'@, (New-Object System.Text.UTF8Encoding($false)))
  $copy = Join-Path $temp "deploy-test.ps1"
  $rootLiteral = "'" + $PSScriptRoot.Replace("'", "''") + "'"
  $fakeLiteral = "'" + $fake.Replace("'", "''") + "'"
  $copySource = $source.Replace('$PSScriptRoot', $rootLiteral)
  $transportLine = '$transport = (Resolve-Path -LiteralPath (Join-Path ' + $rootLiteral + ' "with-opi-ssh.ps1")).Path'
  if (-not $copySource.Contains($transportLine)) { throw "test could not replace exact transport binding" }
  $copySource = $copySource.Replace($transportLine, '$transport = ' + $fakeLiteral)
  [IO.File]::WriteAllText($copy, $copySource, (New-Object System.Text.UTF8Encoding($false)))
  $env:OCTESSERA_DEPLOY_TEST_LOG = $log

  function Assert-Rejected {
    param([hashtable]$Arguments, [string]$Reason)
    $before = if (Test-Path -LiteralPath $log) { [IO.File]::ReadAllText($log) } else { "" }
    $rejected = $false
    try { & $copy @Arguments *> $null } catch { $rejected = $true }
    if (-not $rejected) { throw "Accepted $Reason" }
    $after = if (Test-Path -LiteralPath $log) { [IO.File]::ReadAllText($log) } else { "" }
    if ($after -cne $before) { throw "Transport called for $Reason" }
  }

  Assert-Rejected @{ Target = "pi@board"; LocalBinary = $binary } "non-Orange target"
  Assert-Rejected @{ Target = "octessera@board" } "missing binary"
  Assert-Rejected @{ Target = "octessera@board"; LocalBinary = $binary; Restore = $true } "restore with binary"
  Assert-Rejected @{ Target = "octessera@board"; LocalBinary = $binary; LocalMetadata = "$temp\elsewhere" } "nonadjacent metadata"
  foreach ($change in @(
      @('board_profile', 'raspberry-pi-zero-2w'),
      @('source_commit', ('a' * 40)),
      @('binary_sha256', ('0' * 64)),
      @('profile', 'pi-dev'),
      @('cargo_feature', 'hardware-raspberry-pi-zero-2w'),
      @('artifact_kind', 'diagnostic-only'),
      @('runtime_ready', 'true')
    )) {
    $altered = $originalMetadata.Replace('"' + $change[0] + '":' + $(if ($change[0] -eq 'runtime_ready') { 'false' } else { '"' + $(($originalMetadata | ConvertFrom-Json).($change[0])) + '"' }), '"' + $change[0] + '":' + $(if ($change[0] -eq 'runtime_ready') { 'true' } else { '"' + $change[1] + '"' }))
    [IO.File]::WriteAllText($metadata, $altered)
    Assert-Rejected @{ Target = "octessera@board"; LocalBinary = $binary } "incorrect $($change[0])"
  }
  [IO.File]::WriteAllText($metadata, $originalMetadata.Replace('"schema_version":2', '"schema_version":2,"unexpected":1'))
  Assert-Rejected @{ Target = "octessera@board"; LocalBinary = $binary } "extra metadata field"
  [IO.File]::WriteAllText($metadata, $originalMetadata)
  [IO.File]::Delete($metadata)
  Assert-Rejected @{ Target = "octessera@board"; LocalBinary = $binary } "missing sidecar"
  [IO.File]::WriteAllText($metadata, $originalMetadata)

  & $copy -Target "octessera@board" -LocalBinary $binary *> $null
  $calls = [IO.File]::ReadAllText($log)
  if ($calls -notmatch 'ACTION=''preflight''' -or $calls -notmatch 'ACTION=''activate''' -or
    $calls -notmatch 'ACTION=''cleanup''' -or $calls -notmatch "SOURCE='$head'" -or
    $calls -notmatch 'HASH=''[0-9a-f]{64}''' -or
    $calls -notmatch 'STAGE=''/tmp/octessera-dev-[0-9a-f]{32}''' -or
    $calls -notmatch 'scp octessera@board') {
    throw "Fake transport did not observe bounded Orange payload and staging"
  }
  & $copy -Target "octessera@board" -Restore *> $null
  $calls = [IO.File]::ReadAllText($log)
  if ($calls -notmatch 'ACTION=''restore''' -or $calls -notmatch 'wait_exe "\$old" "\$prior"' -or
    $calls -notmatch 'readlink -f -- "\$installed"' -or $calls -notmatch 'root readlink -f -- "/proc/\$pid/exe"' -or
    $calls -notmatch 'root systemctl daemon-reload' -or $calls -notmatch 'root systemctl restart') {
    throw "Restore and executable checks were not sent to fake transport"
  }
  foreach ($required in @('cmp -s /proc/device-tree/model', 'DropInPaths', 'root rm -- "$drop"',
      'wait_exe "$old" "$prior" "$restarts"', 'ROLLBACK FAILED:', 'trap rollback EXIT',
      '"$pid" != "$prior"', '"$(property NRestarts)" == "$baseline_restarts"',
      'root systemctl reset-failed "$service"', 'marker_ready "$pid" "$invocation"',
      'expected_drop "$SOURCE"', 'root journalctl -b', 'sha256sum -- "$dest/octessera-pi"')) {
    if (-not $remote.Contains($required)) { throw "Remote payload is missing $required" }
  }
  "Orange development deploy local rejection and fake-transport tests passed"
} finally {
  [Environment]::SetEnvironmentVariable("OCTESSERA_DEPLOY_TEST_LOG", $null, "Process")
  Remove-Item -LiteralPath $temp -Recurse -Force
}
