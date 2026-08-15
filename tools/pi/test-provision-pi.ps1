$ErrorActionPreference = "Stop"

$scriptPath = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "provision-pi.ps1")).Path
$workspace = Join-Path $env:TEMP "octessera-provision-pi-test-$PID"
$fakeBin = Join-Path $workspace "bin"
$userProfile = Join-Path $workspace "user profile"
$sshDirectory = Join-Path $userProfile ".ssh"
$log = Join-Path $workspace "ssh.log"
$oldPath = $env:PATH
$oldUserProfile = $env:USERPROFILE
$oldPassphrase = $env:OCTESSERA_PI_PASSPHRASE
$provisionText = [IO.File]::ReadAllText($scriptPath)
if ($provisionText.IndexOf('Target = "pi@192.168.0.218"', [StringComparison]::Ordinal) -lt 0) {
  throw "Pi provisioning default target must be pi@192.168.0.218."
}
if ($provisionText.IndexOf("with-pi-ssh.ps1", [StringComparison]::Ordinal) -lt 0 -or $provisionText -match '(?m)^\s*(?:ssh|scp)\s+@') {
  throw "Pi provisioning must route SSH and SCP through the canonical Pi transport wrapper."
}

New-Item -ItemType Directory -Path $fakeBin, $sshDirectory -Force | Out-Null
try {
  "fake private key" | Set-Content -LiteralPath (Join-Path $sshDirectory "octessera_pi_dev") -Encoding ASCII
  "192.168.0.218 ssh-ed25519 fake" | Set-Content -LiteralPath (Join-Path $sshDirectory "known_hosts") -Encoding ASCII
  @'
@echo off
set "archive="
:next
if "%~1"=="" goto done
if "%~1"=="-czf" set "archive=%~2"
shift
goto next
:done
if defined archive type nul > "%archive%"
exit /b 0
'@ | Set-Content -LiteralPath (Join-Path $fakeBin "tar.cmd") -Encoding ASCII
  @'
@echo off
exit /b 0
'@ | Set-Content -LiteralPath (Join-Path $fakeBin "scp.cmd") -Encoding ASCII
  @'
$encoded = [Console]::In.ReadToEnd()
$payload = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String(($encoded -replace '\s', '')))
Add-Content -LiteralPath $env:OCTESSERA_PROVISION_PI_SSH_LOG -Value $payload -Encoding UTF8
$exitCode = if ($null -eq $env:OCTESSERA_PROVISION_PI_SSH_EXIT_CODE) { 0 } else { [int]$env:OCTESSERA_PROVISION_PI_SSH_EXIT_CODE }
exit $exitCode
'@ | Set-Content -LiteralPath (Join-Path $fakeBin "fake-ssh.ps1") -Encoding UTF8
  @'
@echo off
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0fake-ssh.ps1"
exit /b %ERRORLEVEL%
'@ | Set-Content -LiteralPath (Join-Path $fakeBin "ssh.cmd") -Encoding ASCII

  $env:PATH = "$fakeBin;$oldPath"
  $env:USERPROFILE = $userProfile
  $env:OCTESSERA_PI_PASSPHRASE = "test-only-passphrase"
  $env:OCTESSERA_PROVISION_PI_SSH_LOG = $log

  function Invoke-ProvisionCase {
    param(
      [string]$Name,
      [string[]]$Options,
      [int]$ExpectedExit,
      [string]$ExpectedUpdate
    )

    if (Test-Path -LiteralPath $log) {
      Remove-Item -LiteralPath $log -Force
    }
    $env:OCTESSERA_PROVISION_PI_SSH_EXIT_CODE = [string]$ExpectedExit
    $stdout = Join-Path $workspace "$Name.stdout"
    $stderr = Join-Path $workspace "$Name.stderr"
    $arguments = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $scriptPath) + $Options
    $process = Start-Process -FilePath "powershell.exe" -ArgumentList $arguments -RedirectStandardOutput $stdout -RedirectStandardError $stderr -NoNewWindow -Wait -PassThru
    $actualExit = $process.ExitCode
    if ($actualExit -ne $ExpectedExit) {
      throw "$Name returned $actualExit, expected ${ExpectedExit}: $(Get-Content -Raw -LiteralPath $stderr)"
    }
    if ($ExpectedExit -eq 0) {
      $payload = Get-Content -Raw -LiteralPath $log
      if ($payload -notmatch "UPDATE_INITRAMFS=$ExpectedUpdate\b") {
        throw "$Name did not pass the expected initramfs flags: $payload"
      }
    }
  }

  Invoke-ProvisionCase "default" @() 0 "0"
  Invoke-ProvisionCase "explicit" @("-UpdateInitramfs") 0 "1"
  Invoke-ProvisionCase "transport-failure" @() 75 "0"
  Write-Output "PowerShell provisioning wrapper mock tests passed"
}
finally {
  $env:PATH = $oldPath
  if ($null -eq $oldUserProfile) { Remove-Item Env:\USERPROFILE -ErrorAction SilentlyContinue } else { $env:USERPROFILE = $oldUserProfile }
  if ($null -eq $oldPassphrase) { Remove-Item Env:\OCTESSERA_PI_PASSPHRASE -ErrorAction SilentlyContinue } else { $env:OCTESSERA_PI_PASSPHRASE = $oldPassphrase }
  Remove-Item Env:OCTESSERA_PROVISION_PI_SSH_LOG -ErrorAction SilentlyContinue
  Remove-Item Env:OCTESSERA_PROVISION_PI_SSH_EXIT_CODE -ErrorAction SilentlyContinue
  if (Test-Path -LiteralPath $workspace) {
    Remove-Item -LiteralPath $workspace -Recurse -Force
  }
}
