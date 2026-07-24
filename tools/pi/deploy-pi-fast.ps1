param(
  [string]$Target = "pi@192.168.0.211",
  [string]$Key = "$env:USERPROFILE\.ssh\octessera_pi_dev",
  [string]$RemoteRepo = "/home/pi/octessera-dev",
  [string]$InstallDir = "/opt/octessera",
  [string]$Service = "octessera.service",
  [string]$LocalBinary = "",
  [string]$LocalMetadata = "",
  [string]$BuildProfile = "pi-dev",
  [string]$BoardProfile = "raspberry-pi-zero-2w",
  [switch]$BuildOnPi,
  [switch]$CleanRemote,
  [switch]$SyncOnly,
  [switch]$SkipBuild,
  [switch]$AllowServiceFailure,
  [switch]$NoTail
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "board-profile.ps1")
Assert-RaspberryBoardProfile $BoardProfile
Assert-OctesseraServiceName $Service

$sshArgs = @("-i", $Key, "-o", "IdentitiesOnly=yes", $Target)

function Assert-RemoteAbsolutePath {
  param(
    [string]$Path,
    [string]$Name
  )

  if ([string]::IsNullOrWhiteSpace($Path) -or $Path -notlike '/*' -or $Path.Contains("`n") -or $Path.Contains("`r") -or $Path.Contains([char]0)) {
    throw "$Name must be a non-empty absolute POSIX path without newlines."
  }
}

function ConvertTo-PosixShellSingleQuoted {
  param([string]$Value)

  "'" + $Value.Replace("'", "'\''") + "'"
}

function Join-PosixPath {
  param(
    [string]$Base,
    [string]$Child
  )

  $Base.TrimEnd("/") + "/" + $Child.TrimStart("/")
}

function Invoke-PiSsh {
  param([string]$Command)

  $output = if ($Command.Contains("`n")) {
    $Command | ssh @sshArgs "tr -d '\r' | bash -s"
  } else {
    ssh @sshArgs $Command
  }
  if ($LASTEXITCODE -ne 0) {
    throw "ssh command failed with exit code $LASTEXITCODE"
  }
  $output
}

function Invoke-PiLockedSsh {
  param([string]$Command)

  $output = $Command | ssh @sshArgs "tr -d '\r' | bash -s"
  if ($LASTEXITCODE -eq 75) {
    throw "Updater transaction lock is busy; fast deployment was refused."
  }
  if ($LASTEXITCODE -ne 0) {
    throw "locked ssh command failed with exit code $LASTEXITCODE"
  }
  $output
}

function Copy-ToPi {
  param([string]$Source, [string]$Destination)

  scp -i $Key -o IdentitiesOnly=yes $Source "${Target}:$Destination"
  if ($LASTEXITCODE -ne 0) {
    throw "scp failed with exit code $LASTEXITCODE"
  }
}

Assert-RemoteAbsolutePath $RemoteRepo "RemoteRepo"
Assert-RemoteAbsolutePath $InstallDir "InstallDir"
$remoteRepoValue = ConvertTo-PosixShellSingleQuoted $RemoteRepo
$serviceValue = ConvertTo-PosixShellSingleQuoted $Service
$remoteCandidatePath = "/tmp/octessera-pi-candidate"
$remoteCandidateMetadataPath = "/tmp/octessera-pi-candidate.metadata.json"
$remoteHelperPath = "/tmp/octessera-pi-fast-deploy-remote.sh"
$remoteLockHelperPath = "/tmp/octessera-pi-fast-deploy-lock.py"
$remoteCandidateValue = ConvertTo-PosixShellSingleQuoted $remoteCandidatePath
$remoteCandidateMetadataValue = ConvertTo-PosixShellSingleQuoted $remoteCandidateMetadataPath
$remoteHelperValue = ConvertTo-PosixShellSingleQuoted $remoteHelperPath
$remoteLockHelperValue = ConvertTo-PosixShellSingleQuoted $remoteLockHelperPath
$releasesDirValue = ConvertTo-PosixShellSingleQuoted (Join-PosixPath $InstallDir "releases")
$currentDirValue = ConvertTo-PosixShellSingleQuoted (Join-PosixPath $InstallDir "current")
$currentBinaryValue = ConvertTo-PosixShellSingleQuoted (Join-PosixPath $InstallDir "current/octessera-pi")
$statePathValue = ConvertTo-PosixShellSingleQuoted (Join-PosixPath $InstallDir "update-state.json")
$transactionPathValue = ConvertTo-PosixShellSingleQuoted (Join-PosixPath $InstallDir "update-transaction.json")
$profileMetadataPathValue = ConvertTo-PosixShellSingleQuoted (Join-PosixPath $InstallDir "board-profile.json")
$updaterLockPath = Join-PosixPath $InstallDir ".update.lock"
$updaterLockValue = ConvertTo-PosixShellSingleQuoted $updaterLockPath

$expectedMetadata = if ($LocalBinary -ne "") {
  if (-not (Test-Path -LiteralPath $LocalBinary -PathType Leaf)) {
    throw "LocalBinary was not found: $LocalBinary"
  }
  $resolvedBinary = (Resolve-Path -LiteralPath $LocalBinary).Path
  $metadataPath = if ($LocalMetadata -ne "") {
    $LocalMetadata
  } else {
    Join-Path (Split-Path -Parent $resolvedBinary) "octessera-pi.metadata.json"
  }
  Read-RaspberryBoardMetadata $metadataPath
} else {
  New-RaspberryBoardMetadata
}
Assert-RaspberryBoardMetadata $expectedMetadata | Out-Null

$helperPath = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "deploy-pi-fast-remote.sh")).Path
$lockHelperPath = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "deploy-pi-fast-lock.py")).Path
$helperArguments = @(
  $InstallDir,
  $Service,
  [string]$expectedMetadata.board_profile,
  [string]$expectedMetadata.schema_version,
  [string]$expectedMetadata.binary,
  [string]$expectedMetadata.arch,
  ([string]$expectedMetadata.arch).Split("-", 2)[0],
  [string]$expectedMetadata.cargo_feature,
  $remoteCandidatePath,
  $remoteCandidateMetadataPath,
  $(if ($AllowServiceFailure) { "1" } else { "0" })
)
$helperArgumentValues = $helperArguments | ForEach-Object { ConvertTo-PosixShellSingleQuoted ([string]$_) }
$helperArgumentString = $helperArgumentValues -join " "

try {
  Invoke-PiSsh "set -e; rm -f $remoteCandidateValue $remoteCandidateMetadataValue $remoteHelperValue $remoteLockHelperValue" | Out-Null

  if ($LocalBinary -ne "") {
    Copy-ToPi $resolvedBinary $remoteCandidatePath
    Copy-ToPi $metadataPath $remoteCandidateMetadataPath
  } else {
    $archive = Join-Path $env:TEMP "octessera-pi-source.tar.gz"
    if (Test-Path -LiteralPath $archive) {
      Remove-Item -LiteralPath $archive -Force
    }

    tar `
      --exclude .git `
      --exclude .github `
      --exclude .slim `
      --exclude .opencode `
      --exclude .opencode/node_modules `
      --exclude node_modules `
      --exclude target `
      --exclude dist `
      --exclude '*/dist' `
      --exclude coverage `
      --exclude '*/coverage' `
      --exclude hardware `
      --exclude apps/desktop/dist `
      --exclude apps/desktop/dist-desktop `
      --exclude apps/desktop/coverage `
      -czf $archive `
      -C (Resolve-Path ".") .
    if ($LASTEXITCODE -ne 0) {
      throw "creating Pi source archive failed with exit code $LASTEXITCODE"
    }

    $remoteArchive = "/tmp/octessera-pi-source.tar.gz"
    $remoteArchiveValue = ConvertTo-PosixShellSingleQuoted $remoteArchive
    $syncDirValue = ConvertTo-PosixShellSingleQuoted "$RemoteRepo-sync"
    Copy-ToPi $archive $remoteArchive
    if ($CleanRemote) {
      Invoke-PiSsh "set -e; rm -rf $remoteRepoValue; mkdir -p $remoteRepoValue; tar -xzf $remoteArchiveValue -C $remoteRepoValue; rm -f $remoteArchiveValue" | Out-Null
    } else {
      $syncCommand = @"
set -e
rm -rf $syncDirValue
mkdir -p $syncDirValue $remoteRepoValue
tar -xzf $remoteArchiveValue -C $syncDirValue
if command -v rsync >/dev/null 2>&1; then
  rsync -a --checksum --delete --exclude target/ $syncDirValue/ $remoteRepoValue/
else
  echo "warning: rsync not found; falling back to tar extraction, Cargo cache fingerprints may be invalidated" >&2
  tar -xzf $remoteArchiveValue -C $remoteRepoValue
fi
rm -rf $syncDirValue $remoteArchiveValue
"@
      Invoke-PiSsh $syncCommand | Out-Null
    }

    if ($SyncOnly -or -not $BuildOnPi) {
      return
    }

    $buildProfileValue = ConvertTo-PosixShellSingleQuoted $BuildProfile
    $buildBinary = Join-PosixPath $RemoteRepo ("target/{0}/octessera-pi" -f $BuildProfile)
    $buildBinaryValue = ConvertTo-PosixShellSingleQuoted $buildBinary
    $featureValue = ConvertTo-PosixShellSingleQuoted $RaspberryPiZero2WCargoFeature
    $metadataJsonValue = ConvertTo-PosixShellSingleQuoted (Get-RaspberryBoardMetadataJson)
    $buildCommands = @(
      "set -e",
      '. "$HOME/.cargo/env"',
      "cd $remoteRepoValue"
    )
    if (-not $SkipBuild) {
      $buildCommands += "CARGO_BUILD_JOBS=1 cargo build --profile $buildProfileValue -p octessera-pi --features $featureValue"
    }
    $buildCommands += @(
      "test -x $buildBinaryValue",
      "install -m 755 $buildBinaryValue $remoteCandidateValue",
      "printf '%s' $metadataJsonValue > $remoteCandidateMetadataValue"
    )
    Invoke-PiSsh ($buildCommands -join "`n") | Out-Null
  }

  Copy-ToPi $helperPath $remoteHelperPath
  Copy-ToPi $lockHelperPath $remoteLockHelperPath
  Invoke-PiLockedSsh "set -e; chmod 755 $remoteHelperValue; sudo python3 $remoteLockHelperValue $updaterLockValue 0 $transactionPathValue $remoteHelperValue $helperArgumentString"
}
finally {
  try {
    Invoke-PiSsh "rm -f $remoteCandidateValue $remoteCandidateMetadataValue $remoteHelperValue $remoteLockHelperValue" | Out-Null
  } catch {
  }
}

if (-not $NoTail) {
  ssh @sshArgs "journalctl -u $serviceValue -f"
}
