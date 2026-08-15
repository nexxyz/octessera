param(
  [string]$Target = "pi@192.168.0.218",
  [string]$Key = "$env:USERPROFILE\.ssh\octessera_pi_dev",
  [string]$RemoteRepo = "/home/pi/octessera-dev",
  [string]$Service = "octessera.service",
  [string]$BoardProfile = "raspberry-pi-zero-2w",
  [switch]$UpdateInitramfs,
  [switch]$WakeTrace
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "board-profile.ps1")
Assert-RaspberryBoardProfile $BoardProfile
Assert-OctesseraServiceName $Service

$transport = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "with-pi-ssh.ps1")).Path
$script:PiTransportExitCode = 0
$script:PiFailureExitCode = 0

function Invoke-PiSsh {
  param([string]$Command)

  $payloadPath = Join-Path $env:TEMP ("octessera-pi-provision-command-" + [guid]::NewGuid().ToString("N") + ".sh")
  try {
    $encoding = New-Object System.Text.UTF8Encoding($false)
    [IO.File]::WriteAllText($payloadPath, $Command, $encoding)
    $output = @(& $transport "ssh-payload" -Target $Target -Key $Key $payloadPath)
    $script:PiTransportExitCode = if ($null -eq $LASTEXITCODE) { 0 } else { $LASTEXITCODE }
  } finally {
    Remove-Item -LiteralPath $payloadPath -Force -ErrorAction SilentlyContinue
  }
  if ($script:PiTransportExitCode -ne 0) {
    $script:PiFailureExitCode = $script:PiTransportExitCode
    throw "ssh command failed with exit code $script:PiTransportExitCode"
  }
  $output
}

function Copy-ToPi {
  param([string]$Source, [string]$Destination)

  $output = @(& $transport "scp" -Target $Target -Key $Key $Source "${Target}:$Destination")
  $script:PiTransportExitCode = if ($null -eq $LASTEXITCODE) { 0 } else { $LASTEXITCODE }
  if ($script:PiTransportExitCode -ne 0) {
    $script:PiFailureExitCode = $script:PiTransportExitCode
    throw "scp failed with exit code $script:PiTransportExitCode"
  }
  $output
}

function ConvertTo-ShellSingleQuoted {
  param([string]$Value)
  "'" + $Value.Replace("'", "'\''") + "'"
}

$provisionRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "provision")).Path
$imageFilesRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\pi-image\stage4-octessera\files\root")).Path
$imageFilesParent = Split-Path -Parent $imageFilesRoot
$imageFilesName = Split-Path -Leaf $imageFilesRoot
$deviceUpdateRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\device-update")).Path
$deviceUpdateParent = Split-Path -Parent $deviceUpdateRoot
$deviceUpdateName = Split-Path -Leaf $deviceUpdateRoot
$archive = Join-Path $env:TEMP "octessera-pi-provision.tar.gz"
$exitCode = 0

try {
  if (Test-Path -LiteralPath $archive) {
    Remove-Item -LiteralPath $archive -Force
  }

  tar -czf $archive `
    -C $provisionRoot provision.sh files `
    -C $imageFilesParent $imageFilesName `
    -C $deviceUpdateParent $deviceUpdateName
  if ($LASTEXITCODE -ne 0) {
    throw "creating Pi provision archive failed with exit code $LASTEXITCODE"
  }

  $remoteArchive = "/tmp/octessera-pi-provision.tar.gz"
  $remotePackage = "/tmp/octessera-pi-provision"
  Copy-ToPi $archive $remoteArchive

  $remoteRepoValue = ConvertTo-ShellSingleQuoted $RemoteRepo
  $serviceValue = ConvertTo-ShellSingleQuoted $Service
  $boardProfileValue = ConvertTo-ShellSingleQuoted $BoardProfile
  $updateInitramfsValue = if ($UpdateInitramfs) { "1" } else { "0" }
  $wakeTraceValue = if ($WakeTrace) { "1" } else { "0" }
  $provisionCommand = @"
set -e
rm -rf '$remotePackage'
mkdir -p '$remotePackage'
tar -xzf '$remoteArchive' -C '$remotePackage'
BOARD_PROFILE=$boardProfileValue REMOTE_REPO=$remoteRepoValue SERVICE=$serviceValue UPDATE_INITRAMFS=$updateInitramfsValue WAKE_TRACE=$wakeTraceValue sh '$remotePackage/provision.sh'
rm -rf '$remotePackage' '$remoteArchive'
"@
  Invoke-PiSsh $provisionCommand
}
catch {
  $exitCode = if ($script:PiFailureExitCode -ne 0) { $script:PiFailureExitCode } else { 1 }
  [Console]::Error.WriteLine($_.Exception.Message)
}
finally {
  if (Test-Path -LiteralPath $archive) {
    Remove-Item -LiteralPath $archive -Force
  }
}

if ($exitCode -ne 0) {
  exit $exitCode
}
