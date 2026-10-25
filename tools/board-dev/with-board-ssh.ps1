[CmdletBinding()]
param(
  [Parameter(Mandatory = $true, Position = 0)]
  [ValidateSet("ssh", "ssh-tty", "scp", "ssh-payload")]
  [string]$Command,

  [Parameter(Position = 1, ValueFromRemainingArguments = $true)]
  [string[]]$ArgumentList,

  [Parameter(Mandatory = $true)]
  [ValidateSet("rpi", "opi")]
  [string]$Board,

  [Parameter(Mandatory = $true)]
  [string]$Target
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "..\deployment-target.ps1")

function Resolve-RequiredLocalFile {
  param([string]$Path, [string]$Name)

  if ([string]::IsNullOrWhiteSpace($Path) -or $Path.IndexOfAny([char[]]@([char]0, "`r", "`n")) -ge 0) {
    throw "$Name must be a path without line breaks."
  }
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "$Name was not found at the required path: $Path"
  }
  if (((Get-Item -LiteralPath $Path).Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw "$Name must not be a reparse point: $Path"
  }
  (Resolve-Path -LiteralPath $Path).Path
}

function Test-TargetOperand {
  param([string]$Value)

  $Value -match '^[A-Za-z_][A-Za-z0-9_.-]*@[A-Za-z0-9._:-]+$'
}

function Get-TransportArguments {
  $values = @($ArgumentList | Where-Object { $null -ne $_ })
  foreach ($value in $values) {
    if ($value.IndexOf([char]0) -ge 0) {
      throw "$Command arguments must not contain null characters."
    }
    if ($value -match '^(?:-i|-F|-o|-S|-J|-W|-w|-P|-B|--config|--proxy-command|--proxy-jump|--control-path)' -or ($Command -ne "scp" -and $value -match '^(?:-l|-p)')) {
      throw "$Command arguments must not override the fixed board SSH identity, host-key, or connection settings."
    }
  }

  if ($Command -eq "scp") {
    $targetPrefix = $Target + ":"
    if (@($values | Where-Object { $_.StartsWith($targetPrefix, [StringComparison]::Ordinal) }).Count -ne 1) {
      throw "scp requires exactly one remote operand for the exact target: $targetPrefix"
    }
    foreach ($value in $values) {
      if ($value -notmatch '^[A-Za-z]:[\\/]' -and $value -match '^[A-Za-z0-9._-]+(?:@[A-Za-z0-9._:-]+)?:' -and -not $value.StartsWith($targetPrefix, [StringComparison]::Ordinal)) {
        throw "scp arguments must use only the exact target: $Target"
      }
    }
    return $values
  }

  if ($values.Count -gt 0 -and (Test-TargetOperand $values[0])) {
    if ($values[0] -cne $Target) {
      throw "$Command arguments must use only the exact target: $Target"
    }
    $values = @($values | Select-Object -Skip 1)
  }
  if ($Command -eq "ssh-payload" -and $values.Count -ne 1) {
    throw "ssh-payload requires one local payload file."
  }
  $values
}

function New-AskPassHelper {
  $helperPath = Join-Path ([IO.Path]::GetTempPath()) ("octessera-board-askpass-" + [guid]::NewGuid().ToString("N") + ".cmd")
  $contents = @(
    "@echo off"
    '"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$p = [Environment]::GetEnvironmentVariable(''OCTESSERA_PI_PASSPHRASE'', ''Process''); if ($null -eq $p) { exit 1 }; [Console]::Out.Write($p)"'
    'exit /b %ERRORLEVEL%'
    ''
  ) -join "`r`n"
  $stream = [IO.File]::Open($helperPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
  try {
    $bytes = (New-Object System.Text.UTF8Encoding($false)).GetBytes($contents)
    $stream.Write($bytes, 0, $bytes.Length)
  } finally {
    $stream.Dispose()
  }
  $helperPath
}

Assert-DeploymentTarget $Target | Out-Null
if ($null -eq [Environment]::GetEnvironmentVariable("OCTESSERA_PI_PASSPHRASE", "Process")) {
  throw "OCTESSERA_PI_PASSPHRASE must be set; interactive passphrase input is not supported."
}
$userProfile = [Environment]::GetEnvironmentVariable("USERPROFILE", "Process")
if ([string]::IsNullOrWhiteSpace($userProfile) -or $userProfile.IndexOfAny([char[]]@([char]0, "`r", "`n")) -ge 0 -or -not [IO.Path]::IsPathRooted($userProfile)) {
  throw "USERPROFILE must be an absolute path without line breaks."
}
$sshDirectory = Join-Path $userProfile ".ssh"
$keyName = if ($Board -eq "rpi") { "octessera_pi_dev" } else { "octessera_orange_pi_ed25519" }
$key = Resolve-RequiredLocalFile (Join-Path $sshDirectory $keyName) "Board SSH private key"
$knownHosts = Resolve-RequiredLocalFile (Join-Path $sshDirectory "known_hosts") "Board SSH known_hosts file"

$transportArguments = @(Get-TransportArguments)
if ($Command -eq "ssh-payload") {
  $payloadPath = Resolve-RequiredLocalFile $transportArguments[0] "SSH payload file"
}
$transportCommand = if ($Command -eq "scp") { "scp" } else { "ssh" }
$commandPath = [string]@(Get-Command -Name $transportCommand -CommandType Application -ErrorAction Stop)[0].Source
$nativeArguments = @(
  "-i", $key,
  "-o", "IdentitiesOnly=yes",
  "-o", "UserKnownHostsFile=$knownHosts",
  "-o", "StrictHostKeyChecking=yes",
  "-o", "BatchMode=no",
  "-o", "ConnectTimeout=10",
  "-o", "NumberOfPasswordPrompts=1"
)
$nativeArguments += switch ($Command) {
  "ssh" { @($Target) + $transportArguments }
  "ssh-tty" { @("-tt", $Target) + $transportArguments }
  "scp" { $transportArguments }
  "ssh-payload" { @($Target, "base64 --decode --ignore-garbage | bash -s --") }
}

$saved = @{}
foreach ($name in @("SSH_ASKPASS", "SSH_ASKPASS_REQUIRE", "DISPLAY")) {
  $saved[$name] = [Environment]::GetEnvironmentVariable($name, "Process")
}
$helperPath = $null
$exitCode = 1
try {
  $helperPath = New-AskPassHelper
  [Environment]::SetEnvironmentVariable("SSH_ASKPASS", $helperPath, "Process")
  [Environment]::SetEnvironmentVariable("SSH_ASKPASS_REQUIRE", "force", "Process")
  [Environment]::SetEnvironmentVariable("DISPLAY", "octessera", "Process")
  $ErrorActionPreference = "Continue"
  if ($Command -eq "ssh-payload") {
    $payloadBytes = [byte[]]@([IO.File]::ReadAllBytes($payloadPath) | Where-Object { $_ -ne 13 })
    [Convert]::ToBase64String($payloadBytes) | & $commandPath $nativeArguments
  } else {
    & $commandPath $nativeArguments
  }
  $exitCode = if ($null -eq $LASTEXITCODE) { 0 } else { $LASTEXITCODE }
} finally {
  $ErrorActionPreference = "Stop"
  foreach ($name in $saved.Keys) {
    [Environment]::SetEnvironmentVariable($name, $saved[$name], "Process")
  }
  if ($null -ne $helperPath) {
    Remove-Item -LiteralPath $helperPath -Force
  }
}

exit $exitCode
