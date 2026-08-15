$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "deploy-target.ps1")

$deployScript = Join-Path $PSScriptRoot "deploy-pi-fast.ps1"
$deployText = [IO.File]::ReadAllText($deployScript)
$targetValidationIndex = $deployText.IndexOf('Assert-PiDeploymentTarget $Target | Out-Null', [StringComparison]::Ordinal)
$transportIndex = $deployText.IndexOf('with-pi-ssh.ps1', [StringComparison]::Ordinal)
if ($deployText.IndexOf('Target = "pi@192.168.0.218"', [StringComparison]::Ordinal) -lt 0) {
  throw "Fast deployment default target must be pi@192.168.0.218."
}
if ($targetValidationIndex -lt 0 -or $targetValidationIndex -ge $transportIndex) {
  throw "Fast deployment must validate its target before constructing SSH arguments."
}
if ($deployText.IndexOf("ConvertTo-PosixShellSingleQuoted", [StringComparison]::Ordinal) -lt 0) {
  throw "Fast deployment must retain POSIX shell quoting for remote values."
}
if ($transportIndex -lt 0 -or $deployText -match '(?m)^\s*(?:ssh|scp)\s+@') {
  throw "Fast deployment must route SSH and SCP through the canonical Pi transport wrapper."
}

function Assert-Rejected {
  param(
    [scriptblock]$Action,
    [string]$Label
  )

  try {
    & $Action | Out-Null
  } catch {
    return
  }
  throw "Expected deployment target to be rejected: $Label"
}

foreach ($validTarget in @(
    "pi@192.168.0.211",
    "octessera@octessera.local",
    "deploy@pi-zero-2w"
  )) {
  if ((Assert-PiDeploymentTarget $validTarget) -cne $validTarget) {
    throw "Deployment target validator changed a valid target: $validTarget"
  }
}

foreach ($invalidTarget in @(
    "",
    "pi@host`nnext",
    "pi@host`0",
    "pi@host name",
    "pi@host`tname",
    "-oProxyCommand=",
    "-oProxyCommand=touch /tmp/pwned",
    "pi@-oProxyCommand=touch",
    "pi@host;echo unsafe",
    "pi@one@two",
    "pi@@host",
    "@host",
    "pi@",
    "bad!user@host",
    "pi@host/var",
    "pi@host:22",
    "pi@host..local",
    "pi@1.2.3",
    "pi@999.1.1.1",
    "pi@256.256.256.256"
  )) {
  Assert-Rejected { Assert-PiDeploymentTarget $invalidTarget } $invalidTarget
}

$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("octessera-deploy-target-test-" + [guid]::NewGuid().ToString("N"))
$fakeBin = Join-Path $testRoot "bin"
$userProfile = Join-Path $testRoot "user profile"
$sshDirectory = Join-Path $userProfile ".ssh"
$localRoot = Join-Path $testRoot "repo with spaces"
$binaryPath = Join-Path $localRoot "binary with spaces\octessera-pi"
$metadataPath = Join-Path $localRoot "metadata with spaces.json"
$transportLog = Join-Path $testRoot "transport.log"
$oldPath = $env:PATH
$oldUserProfile = $env:USERPROFILE
$oldPassphrase = $env:OCTESSERA_PI_PASSPHRASE
$oldTransportLog = $env:OCTESSERA_PI_TRANSPORT_LOG
$oldTransportExitCode = $env:OCTESSERA_PI_TRANSPORT_EXIT_CODE
$encoding = New-Object System.Text.UTF8Encoding($false)

function Write-Utf8NoBom {
  param(
    [string]$Path,
    [string]$Contents
  )

  [IO.File]::WriteAllText($Path, $Contents, $encoding)
}

function Invoke-DeployScript {
  param([hashtable]$Parameters)

  try {
    & $deployScript @Parameters *> $null
    return 0
  } catch {
    return 1
  }
}

try {
  New-Item -ItemType Directory -Path $fakeBin, $sshDirectory, (Split-Path -Parent $binaryPath) -Force | Out-Null
  Write-Utf8NoBom (Join-Path $sshDirectory "known_hosts") "192.168.0.218 ssh-ed25519 fake"
  Write-Utf8NoBom (Join-Path $testRoot "key with spaces") "fake private key"
  Write-Utf8NoBom (Join-Path $fakeBin "record-transport.ps1") @'
[IO.File]::AppendAllText($env:OCTESSERA_PI_TRANSPORT_LOG, "$($args[0])`n")
$exitCode = if ($null -eq $env:OCTESSERA_PI_TRANSPORT_EXIT_CODE) { 0 } else { [int]$env:OCTESSERA_PI_TRANSPORT_EXIT_CODE }
exit $exitCode
'@
  Write-Utf8NoBom (Join-Path $fakeBin "ssh.cmd") @'
@echo off
powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0record-transport.ps1" ssh %*
exit /b %ERRORLEVEL%
'@
  Write-Utf8NoBom (Join-Path $fakeBin "scp.cmd") @'
@echo off
powershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0record-transport.ps1" scp %*
exit /b %ERRORLEVEL%
'@
  Write-Utf8NoBom $binaryPath "test binary"
  Write-Utf8NoBom $metadataPath '{"schema_version":1,"board_profile":"raspberry-pi-zero-2w","binary":"octessera-pi","arch":"aarch64-unknown-linux-gnu","cargo_feature":"hardware-raspberry-pi-zero-2w"}'

  $env:PATH = "$fakeBin;$oldPath"
  $env:USERPROFILE = $userProfile
  $env:OCTESSERA_PI_PASSPHRASE = "test-only-passphrase"
  $env:OCTESSERA_PI_TRANSPORT_LOG = $transportLog
  Remove-Item -LiteralPath $transportLog -Force -ErrorAction SilentlyContinue
  $validParameters = @{
    Key = Join-Path $testRoot "key with spaces"
    RemoteRepo = "/home/pi/O'Reilly repo with spaces"
    InstallDir = "/opt/octessera/O'Reilly install"
    LocalBinary = $binaryPath
    LocalMetadata = $metadataPath
    NoTail = $true
  }
  if ((Invoke-DeployScript $validParameters) -ne 0) {
    throw "Fast deployment rejected valid paths or the default target."
  }
  $transportCalls = @(Get-Content -LiteralPath $transportLog -ErrorAction Stop)
  if ($transportCalls.Count -eq 0 -or -not ($transportCalls -contains "scp") -or -not ($transportCalls -contains "ssh")) {
    throw "Valid fast deployment did not reach the mocked SSH/SCP transports."
  }

  $env:OCTESSERA_PI_TRANSPORT_EXIT_CODE = "75"
  $failureStdout = Join-Path $testRoot "failure.stdout"
  $failureStderr = Join-Path $testRoot "failure.stderr"
  $failureArguments = @(
    "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", ('"{0}"' -f $deployScript),
    "-Key", ('"{0}"' -f (Join-Path $testRoot "key with spaces")),
    "-LocalBinary", ('"{0}"' -f $binaryPath),
    "-LocalMetadata", ('"{0}"' -f $metadataPath),
    "-NoTail"
  )
  $failureProcess = Start-Process -FilePath "powershell.exe" -ArgumentList $failureArguments -RedirectStandardOutput $failureStdout -RedirectStandardError $failureStderr -NoNewWindow -Wait -PassThru
  if ($failureProcess.ExitCode -ne 75) {
    throw "Fast deployment did not preserve the mocked transport exit code 75; got $($failureProcess.ExitCode): $(Get-Content -Raw -LiteralPath $failureStderr)"
  }
  Remove-Item Env:\OCTESSERA_PI_TRANSPORT_EXIT_CODE -ErrorAction SilentlyContinue

  foreach ($hostileTarget in @(
      "-oProxyCommand=",
      "pi@host`nnext",
      "pi@host;echo unsafe",
      "pi@one@two"
    )) {
    Remove-Item -LiteralPath $transportLog -Force -ErrorAction SilentlyContinue
    $hostileParameters = @{
      Target = $hostileTarget
      NoTail = $true
    }
    if ((Invoke-DeployScript $hostileParameters) -eq 0) {
      throw "Fast deployment accepted hostile target: $hostileTarget"
    }
    if (Test-Path -LiteralPath $transportLog) {
      throw "Fast deployment reached a transport before rejecting target: $hostileTarget"
    }
  }
} finally {
  $env:PATH = $oldPath
  if ($null -eq $oldUserProfile) {
    Remove-Item Env:\USERPROFILE -ErrorAction SilentlyContinue
  } else {
    $env:USERPROFILE = $oldUserProfile
  }
  if ($null -eq $oldPassphrase) {
    Remove-Item Env:\OCTESSERA_PI_PASSPHRASE -ErrorAction SilentlyContinue
  } else {
    $env:OCTESSERA_PI_PASSPHRASE = $oldPassphrase
  }
  if ($null -eq $oldTransportLog) {
    Remove-Item Env:\OCTESSERA_PI_TRANSPORT_LOG -ErrorAction SilentlyContinue
  } else {
    $env:OCTESSERA_PI_TRANSPORT_LOG = $oldTransportLog
  }
  if ($null -eq $oldTransportExitCode) {
    Remove-Item Env:\OCTESSERA_PI_TRANSPORT_EXIT_CODE -ErrorAction SilentlyContinue
  } else {
    $env:OCTESSERA_PI_TRANSPORT_EXIT_CODE = $oldTransportExitCode
  }
  Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Output "Fast deployment target validation and transport-order tests passed"
