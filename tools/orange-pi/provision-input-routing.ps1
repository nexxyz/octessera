[CmdletBinding()]
param(
  [string]$Target = "octessera@192.168.0.217",
  [string]$Key = "$env:USERPROFILE\.ssh\octessera_orange_pi_ed25519",
  [string]$KnownHosts = "$env:USERPROFILE\.ssh\known_hosts",
  [switch]$Apply,
  [switch]$Preflight,
  [string]$RollbackId
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

if (-not $Apply -and -not $Preflight -and [string]::IsNullOrWhiteSpace($RollbackId)) {
  $Preflight = $true
}
$selectedModes = @(
  $(if ($Apply) { 1 } else { 0 }),
  $(if ($Preflight) { 1 } else { 0 }),
  $(if (-not [string]::IsNullOrWhiteSpace($RollbackId)) { 1 } else { 0 })
) | Where-Object { $_ -eq 1 }
$selectedModes = @($selectedModes)
if ($selectedModes.Count -ne 1) {
  throw "Choose exactly one of -Apply, -Preflight, or -RollbackId."
}
if (-not (Test-Path -LiteralPath $Key -PathType Leaf)) {
  throw "Orange Pi SSH key was not found: $Key"
}
if (-not (Test-Path -LiteralPath $KnownHosts -PathType Leaf)) {
  throw "Orange Pi SSH known_hosts file was not found: $KnownHosts"
}

function New-AskPassScript {
  $askPassScriptPath = $null
  $stream = $null
  try {
    for ($attempt = 0; $attempt -lt 16 -and $null -eq $stream; $attempt++) {
      $candidatePath = Join-Path ([IO.Path]::GetTempPath()) ("octessera-orange-routing-askpass-" + [guid]::NewGuid().ToString("N") + ".cmd")
      try {
        $stream = [IO.File]::Open($candidatePath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        $askPassScriptPath = $candidatePath
      } catch [IO.IOException] {
      }
    }
    if ($null -eq $stream) {
      throw "Could not create a unique temporary SSH_ASKPASS script."
    }

    $contents = @(
      "@echo off"
      '"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$p = [Environment]::GetEnvironmentVariable(''OCTESSERA_PI_PASSPHRASE'', ''Process''); if ($null -eq $p) { exit 1 }; [Console]::Out.Write($p)"'
      'exit /b %ERRORLEVEL%'
    ) -join "`r`n"
    $contents += "`r`n"
    $encoding = New-Object System.Text.UTF8Encoding($false)
    $bytes = $encoding.GetBytes($contents)
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.Flush()
    $stream.Dispose()
    $stream = $null
    return $askPassScriptPath
  } catch {
    if ($null -ne $stream) {
      $stream.Dispose()
    }
    if ($null -ne $askPassScriptPath) {
      [IO.File]::Delete($askPassScriptPath)
    }
    throw
  }
}

$passphraseConfigured = $null -ne [Environment]::GetEnvironmentVariable("OCTESSERA_PI_PASSPHRASE", "Process")
$batchMode = if ($passphraseConfigured) { "no" } else { "yes" }
$transportArgs = @(
  "-i", $Key,
  "-o", "IdentitiesOnly=yes",
  "-o", "UserKnownHostsFile=$KnownHosts",
  "-o", "StrictHostKeyChecking=yes",
  "-o", "BatchMode=$batchMode"
)
$sshArgs = $transportArgs + @(
  $Target
)
$scpArgs = $transportArgs

function Invoke-Checked {
  param(
    [string]$Command,
    [string[]]$Arguments
  )
  & $Command @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "$Command failed with exit code $LASTEXITCODE"
  }
}

function ConvertTo-ShellLiteral {
  param([string]$Value)
  return "'" + $Value.Replace("'", "'\''") + "'"
}

$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")).Path
$remoteRoot = "/tmp/octessera-input-routing"
$remoteScript = "$remoteRoot/input-routing-provision.sh"
$localFiles = @(
  [pscustomobject]@{ Local = (Join-Path $PSScriptRoot "input-routing-provision.sh"); Remote = $remoteScript },
  [pscustomobject]@{ Local = (Join-Path $root "userpatches\overlay\usr\local\share\octessera\device-tree\octessera-h618-input-routing.dts"); Remote = "$remoteRoot/octessera-h618-input-routing.dts" },
  [pscustomobject]@{ Local = (Join-Path $root "userpatches\overlay\usr\local\share\octessera\device-tree\input-routing-overlay-validation.sh"); Remote = "$remoteRoot/input-routing-overlay-validation.sh" },
  [pscustomobject]@{ Local = (Join-Path $root "userpatches\overlay\usr\local\share\octessera\device-tree\input-routing-boot-config.sh"); Remote = "$remoteRoot/input-routing-boot-config.sh" },
  [pscustomobject]@{ Local = (Join-Path $root "userpatches\overlay\usr\local\share\octessera\device-tree\boot-dtb-selection.sh"); Remote = "$remoteRoot/boot-dtb-selection.sh" },
  [pscustomobject]@{ Local = (Join-Path $root "userpatches\overlay\usr\local\share\octessera\device-tree\spi-overlay-validation.sh"); Remote = "$remoteRoot/spi-overlay-validation.sh" },
  [pscustomobject]@{ Local = (Join-Path $root "userpatches\overlay\usr\local\share\octessera\device-tree\armbian-env-token.sh"); Remote = "$remoteRoot/armbian-env-token.sh" }
)
foreach ($file in $localFiles) {
  if (-not (Test-Path -LiteralPath $file.Local -PathType Leaf)) {
    throw "Missing input-routing deployment file: $($file.Local)"
  }
}

$askPassScriptPath = $null
$savedAskPass = [Environment]::GetEnvironmentVariable("SSH_ASKPASS", "Process")
$savedAskPassRequire = [Environment]::GetEnvironmentVariable("SSH_ASKPASS_REQUIRE", "Process")
$savedDisplay = [Environment]::GetEnvironmentVariable("DISPLAY", "Process")

try {
  if ($passphraseConfigured) {
    $askPassScriptPath = New-AskPassScript
    [Environment]::SetEnvironmentVariable("SSH_ASKPASS", $askPassScriptPath, "Process")
    [Environment]::SetEnvironmentVariable("SSH_ASKPASS_REQUIRE", "force", "Process")
    [Environment]::SetEnvironmentVariable("DISPLAY", "octessera", "Process")
  }

  try {
    Invoke-Checked "ssh" ($sshArgs + @("umask 077; rm -rf -- $(ConvertTo-ShellLiteral $remoteRoot); mkdir -p -- $(ConvertTo-ShellLiteral $remoteRoot)"))
    foreach ($file in $localFiles) {
      Invoke-Checked "scp" ($scpArgs + @($file.Local, "${Target}:$($file.Remote)"))
    }

    $arguments = @(
      "sudo", "bash", $remoteScript
    )
    if ($Apply) { $arguments += "--apply" } elseif ($Preflight) { $arguments += "--preflight" } else { $arguments += @("--rollback", $RollbackId) }
    if ([string]::IsNullOrWhiteSpace($RollbackId)) {
      $arguments += @(
        "--overlay-source", "$remoteRoot/octessera-h618-input-routing.dts",
        "--overlay-validation-script", "$remoteRoot/input-routing-overlay-validation.sh",
        "--boot-config-script", "$remoteRoot/input-routing-boot-config.sh",
        "--boot-dtb-selection-script", "$remoteRoot/boot-dtb-selection.sh",
        "--spi-overlay-validation-script", "$remoteRoot/spi-overlay-validation.sh",
        "--armbian-environment-script", "$remoteRoot/armbian-env-token.sh"
      )
    }
    Invoke-Checked "ssh" ($sshArgs + @((($arguments | ForEach-Object { ConvertTo-ShellLiteral ([string]$_) }) -join " ")))
  } finally {
    try {
      & ssh @sshArgs "rm -rf -- $(ConvertTo-ShellLiteral $remoteRoot)" | Out-Null
    } catch {
    }
  }
} finally {
  [Environment]::SetEnvironmentVariable("SSH_ASKPASS", $savedAskPass, "Process")
  [Environment]::SetEnvironmentVariable("SSH_ASKPASS_REQUIRE", $savedAskPassRequire, "Process")
  [Environment]::SetEnvironmentVariable("DISPLAY", $savedDisplay, "Process")

  if ($null -ne $askPassScriptPath) {
    try {
      [IO.File]::Delete($askPassScriptPath)
    } catch {
      throw "Could not remove the temporary SSH_ASKPASS script: $askPassScriptPath"
    }
  }
}
