$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot "install-factory-default.ps1") -RaspberryTarget "pi@test" -OrangeTarget "octessera@test"

$running = @(Get-RunningDesktopApps -ProcessNames @("explorer", "octessera", "octessera-desktop", "octessera-pi"))
if (($running -join ",") -ne "octessera,octessera-desktop") {
  throw "Expected the portable and dev desktop apps, got: $($running -join ',')"
}
if (@(Get-RunningDesktopApps -ProcessNames @("explorer", "code")).Count -ne 0) {
  throw "Unrelated processes must not count as the desktop app."
}

Assert-SameInstalledPatch -Source "abc" -Installed ([ordered]@{ rpi = "abc"; opi = "abc"; desktop = "abc" })
$failed = $false
try {
  Assert-SameInstalledPatch -Source "abc" -Installed ([ordered]@{ rpi = "abc"; opi = "abd"; desktop = "abc" })
} catch {
  $failed = $_.Exception.Message -like "*opi*abd*expected abc*"
}
if (-not $failed) { throw "A differing installed hash must name the device and fail." }

"install-factory-default tests passed."
