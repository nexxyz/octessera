[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
param(
  [string]$HostName = '',
  [string]$UserName = 'octessera',
  [string]$KeyPath = (Join-Path $HOME '.ssh\octessera_orange_pi_ed25519')
)

$ErrorActionPreference = 'Stop'

function Write-Utf8File {
  param(
    [string]$Path,
    [string]$Contents
  )

  $encoding = New-Object System.Text.UTF8Encoding($false)
  [System.IO.File]::WriteAllText($Path, $Contents, $encoding)
}

function ConvertTo-BashSingleQuotedLiteral {
  param([string]$Value)

  return "'" + $Value.Replace("'", "'\''") + "'"
}

function Get-PublicKeyMaterial {
  param([string]$PublicKeyLine)

  $parts = $PublicKeyLine -split '\s+'
  if ($parts.Count -lt 2) {
    throw "invalid public key in $PublicKeyLine"
  }

  return "$($parts[0]) $($parts[1])"
}

function Get-PublicKeyLineFromFile {
  param([string]$Path)

  $lines = @([System.IO.File]::ReadAllLines($Path) | Where-Object { $_.Trim().Length -gt 0 })
  if ($lines.Count -ne 1) {
    throw "public key file must contain exactly one non-empty line: $Path"
  }

  $line = $lines[0].Trim()
  if ($line -notmatch '^ssh-ed25519\s+\S+(\s+.*)?$') {
    throw "public key file is not an ssh-ed25519 public key: $Path"
  }

  return $line
}

function Get-DerivedPublicKeyLine {
  param([string]$PrivateKeyPath)

  $derived = (& ssh-keygen -y -f $PrivateKeyPath 2>$null | Out-String).Trim()
  if ($LASTEXITCODE -ne 0 -or $derived -notmatch '^ssh-ed25519\s+\S+(\s+.*)?$') {
    throw "could not derive the public key from $PrivateKeyPath"
  }

  return $derived
}

function Assert-NotReparsePoint {
  param([string]$Path)

  if (Test-Path -LiteralPath $Path) {
    $item = Get-Item -LiteralPath $Path
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
      throw "refusing to use a reparse point: $Path"
    }
  }
}

if (-not (Get-Command ssh-keygen -ErrorAction SilentlyContinue)) {
  throw 'ssh-keygen was not found. Install or enable the Windows OpenSSH client first.'
}

if ($HostName -eq '' -and $PSBoundParameters.ContainsKey('UserName')) {
  throw '-UserName requires -HostName when an SSH config stanza is requested.'
}

if ($HostName -match '[\r\n]' -or $HostName -match '\s') {
  throw '-HostName must not contain whitespace or line breaks.'
}

if ($UserName -notmatch '^[A-Za-z_][A-Za-z0-9._-]*$') {
  throw '-UserName contains characters that are not safe for an SSH config stanza.'
}

$privateKeyPath = [System.IO.Path]::GetFullPath($KeyPath)
$publicKeyPath = "$privateKeyPath.pub"
$sshDirectory = Split-Path -Parent $privateKeyPath
$configPath = Join-Path $sshDirectory 'config'
$configKeyPath = $privateKeyPath.Replace('\', '/')
$keyComment = 'octessera-orange-pi'

Assert-NotReparsePoint $privateKeyPath
Assert-NotReparsePoint $publicKeyPath

$privateExists = Test-Path -LiteralPath $privateKeyPath -PathType Leaf
$publicExists = Test-Path -LiteralPath $publicKeyPath -PathType Leaf
if ((Test-Path -LiteralPath $privateKeyPath) -and -not $privateExists) {
  throw "private key path is not a file: $privateKeyPath"
}
if ((Test-Path -LiteralPath $publicKeyPath) -and -not $publicExists) {
  throw "public key path is not a file: $publicKeyPath"
}
if (-not $privateExists -and $publicExists) {
  throw "public key exists without its private key; refusing to create or replace a key: $publicKeyPath"
}

$publicKeyLine = $null
if (-not $privateExists) {
  if ($PSCmdlet.ShouldProcess($privateKeyPath, 'create a dedicated Ed25519 SSH key')) {
    if (-not (Test-Path -LiteralPath $sshDirectory)) {
      New-Item -ItemType Directory -Path $sshDirectory -Force | Out-Null
    }
    Assert-NotReparsePoint $sshDirectory
    if (Test-Path -LiteralPath $privateKeyPath) {
      throw "key appeared while preparing to create it; refusing to overwrite: $privateKeyPath"
    }

    Write-Output "Creating $privateKeyPath. ssh-keygen will prompt for a passphrase; use one unless this machine is already protected."
    & ssh-keygen -t ed25519 -f $privateKeyPath -C $keyComment
    if ($LASTEXITCODE -ne 0) {
      throw "ssh-keygen failed with exit code $LASTEXITCODE"
    }
    if (-not (Test-Path -LiteralPath $privateKeyPath -PathType Leaf) -or -not (Test-Path -LiteralPath $publicKeyPath -PathType Leaf)) {
      throw 'ssh-keygen did not create both key files'
    }
  }

  if (Test-Path -LiteralPath $publicKeyPath -PathType Leaf) {
    $publicKeyLine = Get-PublicKeyLineFromFile $publicKeyPath
  }
} else {
  $derivedPublicKey = Get-DerivedPublicKeyLine $privateKeyPath
  if ($publicExists) {
    $publicKeyLine = Get-PublicKeyLineFromFile $publicKeyPath
    if ((Get-PublicKeyMaterial $publicKeyLine) -ne (Get-PublicKeyMaterial $derivedPublicKey)) {
      throw "public and private key files do not match; refusing to replace either file: $privateKeyPath"
    }
  } else {
    $publicKeyLine = "$(Get-PublicKeyMaterial $derivedPublicKey) $keyComment"
    if ($PSCmdlet.ShouldProcess($publicKeyPath, 'write the public half of the existing key')) {
      Write-Utf8File $publicKeyPath "$publicKeyLine`n"
    }
  }
}

if ($HostName -ne '') {
  $beginMarker = '# BEGIN OCTESSERA ORANGE PI'
  $endMarker = '# END OCTESSERA ORANGE PI'
  $stanza = @(
    $beginMarker
    'Host octessera-orange-pi'
    "    HostName $HostName"
    "    User $UserName"
    "    IdentityFile `"$configKeyPath`""
    '    IdentitiesOnly yes'
    $endMarker
  ) -join [Environment]::NewLine

  Assert-NotReparsePoint $configPath
  if (Test-Path -LiteralPath $configPath) {
    if (-not (Test-Path -LiteralPath $configPath -PathType Leaf)) {
      throw "SSH config path is not a file: $configPath"
    }
    $existingConfig = [System.IO.File]::ReadAllText($configPath)
    $normalizedExisting = $existingConfig -replace "`r`n?", "`n"
    $normalizedStanza = $stanza -replace "`r`n?", "`n"
    if ($normalizedExisting.Contains($normalizedStanza)) {
      Write-Output "SSH config stanza already present in $configPath."
    } elseif ($normalizedExisting -match '(?m)^Host\s+octessera-orange-pi\s*$') {
      throw "SSH config already contains Host octessera-orange-pi with different settings; refusing to overwrite: $configPath"
    } else {
      if ($PSCmdlet.ShouldProcess($configPath, 'append a labelled Octessera Orange Pi SSH config stanza')) {
        [System.IO.File]::AppendAllText($configPath, "`r`n$stanza`r`n", (New-Object System.Text.UTF8Encoding($false)))
      }
    }
  } elseif ($PSCmdlet.ShouldProcess($configPath, 'create a labelled Octessera Orange Pi SSH config stanza')) {
    if (-not (Test-Path -LiteralPath $sshDirectory)) {
      New-Item -ItemType Directory -Path $sshDirectory -Force | Out-Null
    }
    Write-Utf8File $configPath "$stanza`n"
  }

  Write-Output "SSH config alias: ssh octessera-orange-pi"
}

if ($null -eq $publicKeyLine) {
  Write-Output 'WhatIf: no public key exists yet, so no public key or runnable next command can be printed.'
  exit 0
}

$keyForShell = ConvertTo-BashSingleQuotedLiteral $publicKeyLine
Write-Output ''
Write-Output 'Public key (copy this exact line to the Orange Pi):'
Write-Output $publicKeyLine
Write-Output ''
Write-Output 'Next command on the Orange Pi terminal (after copying bootstrap-armbian-ssh.sh there):'
Write-Output "sudo bash ./bootstrap-armbian-ssh.sh $keyForShell"
Write-Output ''
Write-Output "Private key remains at: $privateKeyPath (not printed)"
