param(
  [switch]$IncludePlatformCore,
  [switch]$IncludePi,
  [switch]$BuildDesktopExe,
  [switch]$Typecheck
)

$ErrorActionPreference = "Stop"

function Invoke-Step {
  param(
    [string]$Label,
    [scriptblock]$Command
  )

  Write-Output ""
  Write-Output "== $Label =="
  & $Command
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
}

$testCrates = @("playback-runtime")
$clippyCrates = @("playback-runtime")

if ($IncludePlatformCore) {
  $testCrates += "platform-core"
  $clippyCrates += "platform-core"
}

if ($IncludePi) {
  $testCrates += "cellsymphony-pi"
  $clippyCrates += "cellsymphony-pi"
}

Invoke-Step "cargo fmt" { cargo fmt --all --check }

if ($Typecheck) {
  Invoke-Step "typecheck" { corepack pnpm run typecheck }
}

Invoke-Step "cargo test" { cargo test @($testCrates | ForEach-Object { @("-p", $_) }) }
Invoke-Step "cargo clippy" { cargo clippy @($clippyCrates | ForEach-Object { @("-p", $_) }) --all-targets -- -D warnings }

if ($BuildDesktopExe) {
  Invoke-Step "desktop portable exe" { corepack pnpm --filter @cellsymphony/desktop tauri:build:exe }
}
