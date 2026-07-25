param(
  [Parameter(Mandatory = $true)]
  [ValidateNotNullOrEmpty()]
  [string]$Target,
  [string]$Key = "",
  [string]$RemoteScript = "/tmp/octessera-opi-bringup-probe.sh",
  [string]$RemoteLogDir = "/tmp/octessera-opi-bringup",
  [string]$LocalOutputDir = "artifacts/orange-pi-bringup",
  [switch]$WithSudoChecks
)

$ErrorActionPreference = "Stop"

function ConvertTo-ShellLiteral {
  param([string]$Value)
  return "'" + $Value.Replace("'", "'\''") + "'"
}

function Invoke-CheckedNative {
  param(
    [string]$Command,
    [string[]]$Arguments
  )
  & $Command @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "$Command failed with exit code $LASTEXITCODE"
  }
}

$scriptPath = Join-Path $PSScriptRoot "opi-bringup-probe.sh"
if (-not (Test-Path -LiteralPath $scriptPath)) {
  throw "missing probe script: $scriptPath"
}

$sshBaseArgs = @()
if ($Key -ne "") {
  $sshBaseArgs += @("-i", $Key, "-o", "IdentitiesOnly=yes")
}
$sshBaseArgs += @("-o", "StrictHostKeyChecking=yes", "-o", "BatchMode=yes")

$scpTarget = "${Target}:$RemoteScript"
Invoke-CheckedNative "scp" ($sshBaseArgs + @($scriptPath, $scpTarget))

$probeArgs = @("--output-dir", $RemoteLogDir)
if ($WithSudoChecks) {
  $probeArgs += "--with-sudo-checks"
}
$remoteScriptLiteral = ConvertTo-ShellLiteral $RemoteScript
$remoteArgs = ($probeArgs | ForEach-Object { ConvertTo-ShellLiteral $_ }) -join " "
$remoteCommand = "tr -d '\r' < $remoteScriptLiteral > $remoteScriptLiteral.lf && chmod +x $remoteScriptLiteral.lf && bash $remoteScriptLiteral.lf $remoteArgs"

$sshArgs = $sshBaseArgs + @($Target, $remoteCommand)
$probeExitCode = 0
& ssh @sshArgs
if ($LASTEXITCODE -ne 0) {
  $probeExitCode = $LASTEXITCODE
}

if (-not (Test-Path -LiteralPath $LocalOutputDir)) {
  New-Item -ItemType Directory -Path $LocalOutputDir -Force | Out-Null
}

$remoteLogDirLiteral = ConvertTo-ShellLiteral $RemoteLogDir
$latestLogOutput = @(& ssh @sshBaseArgs $Target "cat $remoteLogDirLiteral/latest-log-path 2>/dev/null")
$latestLogExitCode = $LASTEXITCODE
$latestLogPath = $null
if ($latestLogOutput.Count -eq 1) {
  $latestLogPath = ([string]$latestLogOutput[0]).Trim()
}
if ($latestLogExitCode -eq 0 -and $latestLogPath -ne $null -and $latestLogPath -ne "") {
  $remoteLogPath = "${Target}:$latestLogPath"
  & scp @sshBaseArgs $remoteLogPath $LocalOutputDir
  if ($LASTEXITCODE -ne 0) {
    Write-Warning "Could not copy remote bring-up log from $remoteLogPath"
  }
} else {
  Write-Warning "Could not read remote latest-log-path from $RemoteLogDir"
}

if ($probeExitCode -ne 0) {
  throw "remote bring-up probe failed with exit code $probeExitCode"
}

Write-Output "Orange Pi bring-up probe complete. Logs copied to $LocalOutputDir."
