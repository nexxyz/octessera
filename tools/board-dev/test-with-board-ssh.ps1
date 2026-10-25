$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$scriptPath = Join-Path $PSScriptRoot "with-board-ssh.ps1"
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ("octessera-board-ssh-test-" + [guid]::NewGuid().ToString("N"))
$fakeBin = Join-Path $testRoot "bin"
$sshDirectory = Join-Path $testRoot "profile\.ssh"
$recordPath = Join-Path $testRoot "record.json"
$encoding = New-Object System.Text.UTF8Encoding($false)
$savedEnvironment = @{}
foreach ($name in @("PATH", "USERPROFILE", "OCTESSERA_PI_PASSPHRASE", "SSH_ASKPASS", "OCTESSERA_BOARD_SSH_RECORD")) {
  $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, "Process")
}

function Invoke-Wrapper {
  param([object[]]$Arguments)

  [IO.File]::WriteAllText($recordPath, "")
  $quoted = @($Arguments | ForEach-Object { '"' + ($_ -replace '"', '\"') + '"' }) -join " "
  $process = Start-Process powershell.exe -NoNewWindow -PassThru `
    -ArgumentList "-NoProfile -NonInteractive -ExecutionPolicy Bypass -File `"$scriptPath`" $quoted" `
    -RedirectStandardOutput (Join-Path $testRoot "out.log") -RedirectStandardError (Join-Path $testRoot "err.log")
  $null = $process.Handle
  if (-not $process.WaitForExit(60000)) {
    $process.Kill()
    throw "Wrapper did not finish: $($Arguments -join ' ')"
  }
  $process.ExitCode
}

function Read-Record {
  $text = [IO.File]::ReadAllText($recordPath)
  if ($text.Length -eq 0) { return $null }
  $text | ConvertFrom-Json
}

try {
  New-Item -ItemType Directory -Path $sshDirectory, $fakeBin -Force | Out-Null
  foreach ($name in @("octessera_pi_dev", "octessera_orange_pi_ed25519", "known_hosts")) {
    [IO.File]::WriteAllText((Join-Path $sshDirectory $name), "fake", $encoding)
  }
  [IO.File]::WriteAllText((Join-Path $fakeBin "record.ps1"), @'
$record = [ordered]@{
  tool = [string]$args[0]
  arguments = @($args | Select-Object -Skip 1)
  askPass = @(& $env:SSH_ASKPASS) -join ""
  askPassRequire = $env:SSH_ASKPASS_REQUIRE
  helper = $env:SSH_ASKPASS
  stdin = $null
}
if ($record.arguments[-1] -ceq "base64 --decode --ignore-garbage | bash -s --") {
  $record.stdin = [Console]::In.ReadToEnd().Trim()
}
[IO.File]::WriteAllText($env:OCTESSERA_BOARD_SSH_RECORD, ($record | ConvertTo-Json -Compress))
exit 7
'@, $encoding)
  foreach ($tool in @("ssh", "scp")) {
    [IO.File]::WriteAllText((Join-Path $fakeBin "$tool.cmd"), "@echo off`r`npowershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File `"%~dp0record.ps1`" $tool %*`r`nexit /b %ERRORLEVEL%`r`n", $encoding)
  }
  $env:PATH = "$fakeBin;$($savedEnvironment.PATH)"
  $env:USERPROFILE = Join-Path $testRoot "profile"
  $env:OCTESSERA_BOARD_SSH_RECORD = $recordPath
  $env:SSH_ASKPASS = "caller-askpass"
  $env:OCTESSERA_PI_PASSPHRASE = $null

  if ((Invoke-Wrapper @("ssh", "-Board", "rpi", "-Target", "pi@board.test", "true")) -eq 7 -or $null -ne (Read-Record)) {
    throw "Wrapper reached ssh without OCTESSERA_PI_PASSPHRASE."
  }
  $env:OCTESSERA_PI_PASSPHRASE = "test-only-passphrase"

  foreach ($rejected in @(
      @("ssh", "-Board", "rpi", "-Target", "pi@board.test", "-o", "StrictHostKeyChecking=no", "true"),
      @("ssh", "-Board", "rpi", "-Target", "pi@board.test", "pi@other.test", "true"),
      @("scp", "-Board", "opi", "-Target", "octessera@board.test", "local", "octessera@other.test:/tmp/x"),
      @("scp", "-Board", "opi", "-Target", "octessera@board.test", "local", "remote"),
      @("ssh", "-Board", "opi", "-Target", "-oProxyCommand=x@board", "true")
    )) {
    if ((Invoke-Wrapper $rejected) -eq 7 -or $null -ne (Read-Record)) {
      throw "Wrapper accepted unsafe arguments: $($rejected -join ' ')"
    }
  }

  if ((Invoke-Wrapper @("ssh", "-Board", "rpi", "-Target", "pi@board.test", "printf ok")) -ne 7) {
    throw "Wrapper did not return the ssh exit code: $([IO.File]::ReadAllText((Join-Path $testRoot 'err.log')))"
  }
  $record = Read-Record
  $expected = @("-i", (Join-Path $sshDirectory "octessera_pi_dev"), "-o", "IdentitiesOnly=yes",
    "-o", "UserKnownHostsFile=$(Join-Path $sshDirectory 'known_hosts')", "-o", "StrictHostKeyChecking=yes",
    "-o", "BatchMode=no", "-o", "ConnectTimeout=10", "-o", "NumberOfPasswordPrompts=1", "pi@board.test", "printf ok")
  if ($record.tool -cne "ssh" -or (@($record.arguments) -join "|") -cne ($expected -join "|")) {
    throw "Raspberry ssh arguments were not fixed: $(@($record.arguments) -join ' ')"
  }
  if ($record.askPass -cne "test-only-passphrase" -or $record.askPassRequire -cne "force") {
    throw "Askpass helper did not supply the passphrase."
  }
  if ((Test-Path -LiteralPath $record.helper) -or $env:SSH_ASKPASS -cne "caller-askpass" -or $null -ne $env:SSH_ASKPASS_REQUIRE) {
    throw "Askpass helper or environment was not cleaned up."
  }
  if ([IO.File]::ReadAllText($scriptPath).Contains("test-only-passphrase")) {
    throw "Passphrase leaked into the wrapper."
  }

  Invoke-Wrapper @("scp", "-Board", "opi", "-Target", "octessera@board.test", "C:\local file", "octessera@board.test:/tmp/x") | Out-Null
  $record = Read-Record
  if ($record.tool -cne "scp" -or $record.arguments[1] -cne (Join-Path $sshDirectory "octessera_orange_pi_ed25519") -or $record.arguments[-1] -cne "octessera@board.test:/tmp/x") {
    throw "Orange scp did not use the Orange key and exact operands."
  }

  $payload = Join-Path $testRoot "payload.sh"
  [IO.File]::WriteAllText($payload, "echo one`r`necho two`r`n", $encoding)
  Invoke-Wrapper @("ssh-payload", "-Board", "opi", "-Target", "octessera@board.test", $payload) | Out-Null
  $record = Read-Record
  if ($record.arguments[-2] -cne "octessera@board.test" -or $record.stdin -cne [Convert]::ToBase64String($encoding.GetBytes("echo one`necho two`n"))) {
    throw "ssh-payload did not send the CR-stripped payload."
  }
  "Board SSH wrapper tests passed."
} finally {
  foreach ($name in $savedEnvironment.Keys) {
    [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name], "Process")
  }
  Remove-Item -LiteralPath $testRoot -Recurse -Force -ErrorAction SilentlyContinue
}
