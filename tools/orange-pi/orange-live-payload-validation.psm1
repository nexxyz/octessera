Set-StrictMode -Version Latest

function Assert-OrangeGeneratedLivePayloadSyntax {
  param([Parameter(Mandatory)][string]$Payload)
  $bashCommand = Get-Command bash -ErrorAction SilentlyContinue
  $wslCommand = Get-Command wsl.exe -ErrorAction SilentlyContinue
  if ($null -ne $bashCommand -and [string]$bashCommand.Source -notmatch "WindowsApps") {
    $temporary = Join-Path ([IO.Path]::GetTempPath()) ("octessera-live-payload-" + [guid]::NewGuid().ToString("N") + ".sh")
    try {
      [IO.File]::WriteAllText($temporary, $Payload, (New-Object System.Text.UTF8Encoding($false)))
      & bash -n $temporary
      if ($LASTEXITCODE -ne 0) { throw "Generated live payload failed bash -n." }
    } finally {
      Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue
    }
  } elseif ($null -ne $wslCommand) {
    $temporary = Join-Path ([IO.Path]::GetTempPath()) ("octessera-live-payload-" + [guid]::NewGuid().ToString("N") + ".sh")
    try {
      [IO.File]::WriteAllText($temporary, $Payload, (New-Object System.Text.UTF8Encoding($false)))
      $drive = $temporary.Substring(0, 1).ToLowerInvariant()
      $wslPath = "/mnt/$drive" + ($temporary.Substring(2) -replace "\\", "/")
      & wsl.exe bash -n $wslPath
      if ($LASTEXITCODE -ne 0) { throw "Generated live payload failed WSL bash -n." }
    } finally {
      Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue
    }
  }
}

Export-ModuleMember -Function Assert-OrangeGeneratedLivePayloadSyntax
