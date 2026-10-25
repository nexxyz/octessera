[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)][string]$Target,
  [string]$LocalBinary,
  [switch]$Restore
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "..\deployment-target.ps1")
Assert-DeploymentTarget $Target | Out-Null
if (-not $Target.StartsWith("octessera@", [StringComparison]::Ordinal)) {
  throw "Orange development deployment requires octessera@host."
}
if ($Restore -and $PSBoundParameters.ContainsKey("LocalBinary")) {
  throw "-Restore does not accept a binary."
}
if (-not $Restore -and [string]::IsNullOrWhiteSpace($LocalBinary)) {
  throw "-LocalBinary is required for Orange development deployment."
}

$transport = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\board-dev\with-board-ssh.ps1")).Path
$remoteScript = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\board-dev\dev-runtime.sh")).Path
$boardCheck = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\board-dev\board-check.sh")).Path
$hash = ""
if (-not $Restore) {
  $binary = (Resolve-Path -LiteralPath $LocalBinary -ErrorAction Stop).Path
  if ((Split-Path -Leaf $binary) -cne "octessera-pi" -or -not (Test-Path -LiteralPath $binary -PathType Leaf)) {
    throw "LocalBinary must be an octessera-pi file."
  }
  $hash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Invoke-OrangePayload {
  param([string]$Contents, [string]$Label)
  $payloadPath = Join-Path ([IO.Path]::GetTempPath()) ("octessera-dev-payload-" + [guid]::NewGuid().ToString("N") + ".sh")
  try {
    [IO.File]::WriteAllText($payloadPath, $Contents, (New-Object System.Text.UTF8Encoding($false)))
    & $transport ssh-payload -Board opi -Target $Target $payloadPath | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "Orange $Label failed (exit $LASTEXITCODE)." }
  } finally {
    Remove-Item -LiteralPath $payloadPath -Force -ErrorAction SilentlyContinue
  }
}

function Invoke-OrangeAction {
  param([string]$Action)
  Invoke-OrangePayload ("ACTION='$Action'`nBOARD_PROFILE='orange-pi-zero-2w'`nHASH='$hash'`n" + [IO.File]::ReadAllText($remoteScript)) $Action
}

if ($Restore) {
  Invoke-OrangeAction "restore"
  return
}
Invoke-OrangeAction "preflight"
& $transport scp -Board opi -Target $Target $binary "${Target}:/tmp/octessera-pi-dev"
if ($LASTEXITCODE -ne 0) { throw "Orange binary upload failed (exit $LASTEXITCODE)." }
Invoke-OrangeAction "activate"
try {
  Invoke-OrangePayload ("BOARD_PROFILE='orange-pi-zero-2w'`n" + [IO.File]::ReadAllText($boardCheck)) "board check"
} catch {
  throw "Board check failed; run -Restore. $_"
}
