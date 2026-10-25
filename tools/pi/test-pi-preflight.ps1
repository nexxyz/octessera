$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$scriptPath = Join-Path $PSScriptRoot "pi-preflight.ps1"
$source = [IO.File]::ReadAllText($scriptPath)
$targetMandatory = (Get-Command $scriptPath).Parameters["Target"].Attributes | Where-Object { $_ -is [Parameter] -and $_.Mandatory }
if (-not $targetMandatory -or $source -notmatch 'Assert-DeploymentTarget\s+\$Target') {
  throw "Raspberry preflight must require and validate an explicit target."
}
foreach ($forbidden in @("systemctl\s+status", "journalctl", "--lines")) {
  if ($source -match $forbidden) {
    throw "Raspberry preflight contains forbidden shared-evidence command content: $forbidden"
  }
}

$matches = [regex]::Matches($source, "systemctl show octessera\.service[^']*--property=([A-Za-z,]+)")
if ($matches.Count -ne 1) {
  throw "Raspberry preflight must contain exactly one allowlisted systemctl show command."
}
$properties = $matches[0].Groups[1].Value.Split(",")
$allowlist = @("ActiveState", "SubState", "MainPID", "InvocationID", "User", "UnitFileState")
if ((@($properties | Sort-Object) -join ",") -cne (@($allowlist | Sort-Object) -join ",")) {
  throw "Raspberry preflight systemctl property allowlist changed unexpectedly."
}

Write-Output "Raspberry preflight shared-evidence sanitization tests passed"
