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

function Get-PosixDirectory {
  param([string]$Path)
  $separatorIndex = $Path.LastIndexOf('/')
  if ($separatorIndex -lt 0) {
    return "."
  }
  if ($separatorIndex -eq 0) {
    return "/"
  }
  return $Path.Substring(0, $separatorIndex)
}

function Assert-ValidRemotePath {
  param(
    [string]$Path,
    [string]$ParameterName
  )

  if ([string]::IsNullOrEmpty($Path) -or $Path -notmatch '\A/(?:[A-Za-z0-9._-]+/)*[A-Za-z0-9._-]+\z') {
    throw "$ParameterName must be a non-root absolute POSIX path with no trailing slash and only ASCII letters, digits, '.', '_', and '-': $Path"
  }
  foreach ($segment in $Path.Substring(1).Split('/')) {
    if ($segment -eq "." -or $segment -eq "..") {
      throw "$ParameterName must not contain dot path segments: $Path"
    }
  }
}

$scriptPath = Join-Path $PSScriptRoot "opi-bringup-probe.sh"
if (-not (Test-Path -LiteralPath $scriptPath)) {
  throw "missing probe script: $scriptPath"
}
$validatorPath = Join-Path $PSScriptRoot "opi-bringup-validator.sh"
$identityValidatorPath = Join-Path $PSScriptRoot "opi-bringup-identity-validator.sh"
$hardwareValidatorPath = Join-Path $PSScriptRoot "opi-bringup-hardware-validator.sh"
$supportFiles = @(
  [pscustomobject]@{ LocalPath = $validatorPath; Name = "opi-bringup-validator.sh" }
  [pscustomobject]@{ LocalPath = $identityValidatorPath; Name = "opi-bringup-identity-validator.sh" }
  [pscustomobject]@{ LocalPath = $hardwareValidatorPath; Name = "opi-bringup-hardware-validator.sh" }
)
Assert-ValidRemotePath $RemoteScript "RemoteScript"
Assert-ValidRemotePath $RemoteLogDir "RemoteLogDir"
foreach ($supportFile in $supportFiles) {
  if (-not (Test-Path -LiteralPath $supportFile.LocalPath -PathType Leaf)) {
    throw "missing probe support module: $($supportFile.LocalPath)"
  }
}

$sshBaseArgs = @()
if ($Key -ne "") {
  $sshBaseArgs += @("-i", $Key, "-o", "IdentitiesOnly=yes")
}
$sshBaseArgs += @("-o", "StrictHostKeyChecking=yes", "-o", "BatchMode=yes")

$remoteSupportFiles = @()
$remoteSupportDirectory = Get-PosixDirectory $RemoteScript
foreach ($supportFile in $supportFiles) {
  $remoteSupportPath = if ($remoteSupportDirectory -eq "/") {
    "/$($supportFile.Name)"
  } else {
    "$remoteSupportDirectory/$($supportFile.Name)"
  }
  if ($remoteSupportPath -ceq $RemoteScript) {
    throw "RemoteScript collides with derived support file: $remoteSupportPath"
  }
  $remoteSupportFiles += [pscustomobject]@{
    LocalPath = $supportFile.LocalPath
    Name = $supportFile.Name
    RemotePath = $remoteSupportPath
  }
}

$remoteFiles = @(
  [pscustomobject]@{ LocalPath = $scriptPath; RemotePath = $RemoteScript; Executable = $true }
)
foreach ($remoteSupportFile in $remoteSupportFiles) {
  $remoteFiles += [pscustomobject]@{
    LocalPath = $remoteSupportFile.LocalPath
    RemotePath = $remoteSupportFile.RemotePath
    Executable = $false
  }
}

foreach ($remoteFile in $remoteFiles) {
  Invoke-CheckedNative "scp" ($sshBaseArgs + @($remoteFile.LocalPath, "${Target}:$($remoteFile.RemotePath)"))
}

$probeArgs = @("--output-dir", $RemoteLogDir)
if ($WithSudoChecks) {
  $probeArgs += "--with-sudo-checks"
}
$remoteScriptLiteral = ConvertTo-ShellLiteral $RemoteScript
$remoteArgs = ($probeArgs | ForEach-Object { ConvertTo-ShellLiteral $_ }) -join " "
$remoteCommandParts = @("umask 077; IFS=; set -f")
for ($index = 0; $index -lt $remoteFiles.Count; $index++) {
  $remoteFile = $remoteFiles[$index]
  $remotePathLiteral = ConvertTo-ShellLiteral $remoteFile.RemotePath
  $remoteTempLiteral = ConvertTo-ShellLiteral "$($remoteFile.RemotePath).octessera-lf.XXXXXX"
  $tempVariable = "octessera_lf_$index"
  $tempVariableReference = "`$$tempVariable"
  $remoteCommandParts += "$tempVariable" + '=$(mktemp -- ' + $remoteTempLiteral + ')'
  $remoteCommandParts += "tr -d '\r' < $(ConvertTo-ShellLiteral $remoteFile.RemotePath) > $tempVariableReference"
  if ($remoteFile.Executable) {
    $remoteCommandParts += "chmod +x $tempVariableReference"
  }
  $remoteCommandParts += "mv -f -- $tempVariableReference $remotePathLiteral"
}
$remoteCommandParts += "bash $remoteScriptLiteral $remoteArgs"
$remoteCommand = $remoteCommandParts -join " && "

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
