param(
  [string]$Target = "pi@192.168.0.211",
  [string]$Key = "$env:USERPROFILE\.ssh\octessera_pi_dev",
  [string]$RemoteRepo = "/home/pi/octessera-dev",
  [string]$Service = "octessera.service",
  [switch]$UpdateInitramfs,
  [switch]$WakeTrace
)

$ErrorActionPreference = "Stop"

$sshArgs = @("-i", $Key, "-o", "IdentitiesOnly=yes", $Target)

function Invoke-PiSsh {
  param([string]$Command)
  if ($Command.Contains("`n")) {
    $Command | ssh @sshArgs "tr -d '\r' | bash -s"
  } else {
    ssh @sshArgs $Command
  }
  if ($LASTEXITCODE -ne 0) {
    throw "ssh command failed with exit code $LASTEXITCODE"
  }
}

function Copy-ToPi {
  param([string]$Source, [string]$Destination)
  scp -i $Key -o IdentitiesOnly=yes $Source "${Target}:$Destination"
  if ($LASTEXITCODE -ne 0) {
    throw "scp failed with exit code $LASTEXITCODE"
  }
}

function ConvertTo-ShellSingleQuoted {
  param([string]$Value)
  "'" + $Value.Replace("'", "'\''") + "'"
}

$provisionRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "provision")).Path
$imageFilesRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\pi-image\stage4-octessera\files\root")).Path
$imageFilesParent = Split-Path -Parent $imageFilesRoot
$imageFilesName = Split-Path -Leaf $imageFilesRoot
$archive = Join-Path $env:TEMP "octessera-pi-provision.tar.gz"

try {
  if (Test-Path -LiteralPath $archive) {
    Remove-Item -LiteralPath $archive -Force
  }

  tar -czf $archive `
    -C $provisionRoot provision.sh files `
    -C $imageFilesParent $imageFilesName
  if ($LASTEXITCODE -ne 0) {
    throw "creating Pi provision archive failed with exit code $LASTEXITCODE"
  }

  $remoteArchive = "/tmp/octessera-pi-provision.tar.gz"
  $remotePackage = "/tmp/octessera-pi-provision"
  Copy-ToPi $archive $remoteArchive

  $remoteRepoValue = ConvertTo-ShellSingleQuoted $RemoteRepo
  $serviceValue = ConvertTo-ShellSingleQuoted $Service
  $updateInitramfsValue = if ($UpdateInitramfs) { "1" } else { "0" }
  $wakeTraceValue = if ($WakeTrace) { "1" } else { "0" }
  $provisionCommand = @"
set -e
rm -rf '$remotePackage'
mkdir -p '$remotePackage'
tar -xzf '$remoteArchive' -C '$remotePackage'
REMOTE_REPO=$remoteRepoValue SERVICE=$serviceValue UPDATE_INITRAMFS=$updateInitramfsValue WAKE_TRACE=$wakeTraceValue sh '$remotePackage/provision.sh'
rm -rf '$remotePackage' '$remoteArchive'
"@
  Invoke-PiSsh $provisionCommand
}
finally {
  if (Test-Path -LiteralPath $archive) {
    Remove-Item -LiteralPath $archive -Force
  }
}
