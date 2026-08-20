$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$source = [IO.File]::ReadAllText((Join-Path $PSScriptRoot "pi-preflight.ps1"))
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
