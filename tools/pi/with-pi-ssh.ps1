[CmdletBinding()]
param(
  [Parameter(Mandatory = $true, Position = 0)]
  [ValidateSet("ssh", "scp", "ssh-payload")]
  [Alias("Mode")]
  [string]$Command,

  [Parameter(Position = 1, ValueFromRemainingArguments = $true)]
  [Alias("Arguments")]
  [string[]]$ArgumentList,

  [string]$Target = "pi@192.168.0.218",
  [string]$Key = "$env:USERPROFILE\.ssh\octessera_pi_dev",
  [string]$KnownHosts = "$env:USERPROFILE\.ssh\known_hosts"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot "deploy-target.ps1")

function Resolve-RequiredLocalFile {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path,
    [Parameter(Mandatory = $true)]
    [string]$Name
  )

  if ([string]::IsNullOrWhiteSpace($Path) -or $Path.IndexOfAny([char[]]@([char]0, "`r", "`n")) -ge 0) {
    throw "$Name must be a path without line breaks."
  }
  if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    throw "$Name was not found at the required path: $Path"
  }

  $item = Get-Item -LiteralPath $Path
  if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw "$Name must not be a reparse point: $Path"
  }
  (Resolve-Path -LiteralPath $Path).Path
}

function Assert-SafeOptionArguments {
  param(
    [string[]]$Arguments,
    [Parameter(Mandatory = $true)]
    [ValidateSet("ssh", "scp")]
    [string]$Tool
  )

  foreach ($value in @($Arguments)) {
    if ($null -eq $value -or $value.IndexOf([char]0) -ge 0) {
      throw "$Tool arguments must not contain null characters."
    }
    if ($value -match '^(?:-i|-F|-o|-S|-J|-W|-w|-P|-B|--config|--proxy-command|--proxy-jump|--control-path)' -or ($Tool -eq "ssh" -and $value -match '^(?:-l|-p)')) {
      throw "$Tool arguments must not override the fixed Pi SSH identity, host-key, or connection settings."
    }
  }
}

function Test-TargetOperand {
  param([string]$Value)

  $Value -match '^[A-Za-z_][A-Za-z0-9_.-]*@[A-Za-z0-9._:-]+$'
}

function Normalize-SshArguments {
  param(
    [string[]]$Arguments,
    [Parameter(Mandatory = $true)]
    [string]$ExpectedTarget
  )

  $values = @($Arguments)
  Assert-SafeOptionArguments $values "ssh"
  if ($values.Count -gt 0 -and (Test-TargetOperand $values[0])) {
    if ($values[0] -cne $ExpectedTarget) {
      throw "ssh arguments must use only the exact Pi target: $ExpectedTarget"
    }
    $values = @($values | Select-Object -Skip 1)
  }
  $values
}

function Normalize-ScpArguments {
  param(
    [string[]]$Arguments,
    [Parameter(Mandatory = $true)]
    [string]$ExpectedTarget
  )

  $values = @($Arguments)
  Assert-SafeOptionArguments $values "scp"
  $targetPrefix = $ExpectedTarget + ":"
  $remoteOperands = @($values | Where-Object { $_.StartsWith($targetPrefix, [StringComparison]::Ordinal) })
  if ($remoteOperands.Count -ne 1) {
    throw "scp requires exactly one remote operand for the exact Pi target: $targetPrefix"
  }

  foreach ($value in $values) {
    if ($value -notmatch '^[A-Za-z]:[\\/]' -and $value -match '^[A-Za-z0-9._-]+(?:@[A-Za-z0-9._:-]+)?:' -and -not $value.StartsWith($targetPrefix, [StringComparison]::Ordinal)) {
      throw "scp arguments must use only the exact Pi target: $ExpectedTarget"
    }
  }
  $values
}

function Resolve-PayloadArgument {
  param(
    [string[]]$Arguments,
    [Parameter(Mandatory = $true)]
    [string]$ExpectedTarget
  )

  $values = @($Arguments)
  if ($values.Count -eq 2 -and (Test-TargetOperand $values[0])) {
    if ($values[0] -cne $ExpectedTarget) {
      throw "ssh-payload arguments must use only the exact Pi target: $ExpectedTarget"
    }
    return $values[1]
  }
  if ($values.Count -ne 1) {
    throw "ssh-payload requires one local payload file for the exact Pi target."
  }
  $values[0]
}

function New-AskPassHelper {
  $helperPath = $null
  $stream = $null
  try {
    for ($attempt = 0; $attempt -lt 16 -and $null -eq $stream; $attempt++) {
      $candidatePath = Join-Path ([IO.Path]::GetTempPath()) ("octessera-pi-askpass-" + [guid]::NewGuid().ToString("N") + ".cmd")
      try {
        $stream = [IO.File]::Open($candidatePath, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        $helperPath = $candidatePath
      } catch [IO.IOException] {
      }
    }
    if ($null -eq $stream) {
      throw "Could not create a unique temporary SSH_ASKPASS helper."
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
    return $helperPath
  } catch {
    if ($null -ne $stream) {
      $stream.Dispose()
    }
    if ($null -ne $helperPath) {
      [IO.File]::Delete($helperPath)
    }
    throw
  }
}

Assert-PiDeploymentTarget $Target | Out-Null
if ($null -eq [Environment]::GetEnvironmentVariable("OCTESSERA_PI_PASSPHRASE", "Process")) {
  throw "OCTESSERA_PI_PASSPHRASE must be set; interactive passphrase input is not supported."
}

$userProfile = [Environment]::GetEnvironmentVariable("USERPROFILE", "Process")
if ([string]::IsNullOrWhiteSpace($userProfile) -or $userProfile.IndexOfAny([char[]]@([char]0, "`r", "`n")) -ge 0 -or -not [IO.Path]::IsPathRooted($userProfile)) {
  throw "USERPROFILE must be an absolute path without line breaks."
}
$sshDirectory = Join-Path $userProfile ".ssh"
if (-not (Test-Path -LiteralPath $sshDirectory -PathType Container)) {
  throw "Pi SSH directory was not found at the required path: $sshDirectory"
}
$sshDirectoryItem = Get-Item -LiteralPath $sshDirectory
if (($sshDirectoryItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
  throw "Pi SSH directory must not be a reparse point: $sshDirectory"
}
$Key = Resolve-RequiredLocalFile $Key "Pi SSH private key"
$KnownHosts = Resolve-RequiredLocalFile $KnownHosts "Pi SSH known_hosts file"

$transportCommand = if ($Command -eq "ssh-payload") { "ssh" } else { $Command }
$transportArguments = if ($Command -eq "ssh") {
  Normalize-SshArguments $ArgumentList $Target
} elseif ($Command -eq "scp") {
  Normalize-ScpArguments $ArgumentList $Target
} else {
  $payloadPath = Resolve-PayloadArgument $ArgumentList $Target
  $payloadPath = Resolve-RequiredLocalFile $payloadPath "SSH payload file"
  @()
}

$commandInfo = @(Get-Command -Name $transportCommand -CommandType Application -ErrorAction Stop)
$commandPath = [string]$commandInfo[0].Source
$fixedArguments = @(
  "-i", $Key,
  "-o", "IdentitiesOnly=yes",
  "-o", "UserKnownHostsFile=$KnownHosts",
  "-o", "StrictHostKeyChecking=yes",
  "-o", "BatchMode=no",
  "-o", "ConnectTimeout=10",
  "-o", "NumberOfPasswordPrompts=1"
)
$helperPath = $null
$exitCode = 1
$savedAskPass = [Environment]::GetEnvironmentVariable("SSH_ASKPASS", "Process")
$savedAskPassRequire = [Environment]::GetEnvironmentVariable("SSH_ASKPASS_REQUIRE", "Process")
$savedDisplay = [Environment]::GetEnvironmentVariable("DISPLAY", "Process")

try {
  $helperPath = New-AskPassHelper
  [Environment]::SetEnvironmentVariable("SSH_ASKPASS", $helperPath, "Process")
  [Environment]::SetEnvironmentVariable("SSH_ASKPASS_REQUIRE", "force", "Process")
  [Environment]::SetEnvironmentVariable("DISPLAY", "octessera", "Process")

  $payload = $null
  if ($Command -eq "ssh-payload") {
    $payload = [Convert]::ToBase64String([IO.File]::ReadAllBytes($payloadPath))
    $nativeArguments = $fixedArguments + @($Target, "tr -d '\r' | base64 --decode | bash -s --")
  } elseif ($Command -eq "scp") {
    $nativeArguments = $fixedArguments + $transportArguments
  } else {
    $nativeArguments = $fixedArguments + @($Target) + $transportArguments
  }

  $savedNativeErrorActionPreference = $ErrorActionPreference
  try {
    $ErrorActionPreference = "Continue"
    if ($Command -eq "ssh-payload") {
      $payload | & $commandPath $nativeArguments
    } else {
      & $commandPath $nativeArguments
    }
  } finally {
    $ErrorActionPreference = $savedNativeErrorActionPreference
  }
  $exitCode = $LASTEXITCODE
  if ($null -eq $exitCode) {
    $exitCode = 0
  }
} finally {
  try {
    [Environment]::SetEnvironmentVariable("SSH_ASKPASS", $savedAskPass, "Process")
    [Environment]::SetEnvironmentVariable("SSH_ASKPASS_REQUIRE", $savedAskPassRequire, "Process")
    [Environment]::SetEnvironmentVariable("DISPLAY", $savedDisplay, "Process")
  } finally {
    if ($null -ne $helperPath) {
      try {
        [IO.File]::Delete($helperPath)
      } catch {
        throw "Could not remove the temporary SSH_ASKPASS helper: $helperPath"
      }
    }
  }
}

exit $exitCode
