$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$driver = Join-Path $PSScriptRoot "run-pi-performance-baseline.ps1"
$runner = Join-Path $PSScriptRoot "run-pi-timing-probes.ps1"
$driverSource = [IO.File]::ReadAllText($driver)
$runnerSource = [IO.File]::ReadAllText($runner)
$manifestModule = Join-Path $PSScriptRoot "..\performance\performance-baseline-plan.psm1"
Import-Module $manifestModule -Force
$manifestPath = Join-Path $PSScriptRoot "..\performance\cross-board-baseline.json"
$manifest = Read-PerformanceBaselineManifest $manifestPath
if ($driverSource -notmatch 'Assert-RaspberryBuildMetadata[^\r\n]+\$repositoryIdentity\.Head') { throw "Raspberry baseline driver does not bind metadata to the clean repository HEAD." }

function Assert-Throws {
  param([Parameter(Mandatory)][scriptblock]$Action, [Parameter(Mandatory)][string]$Label)
  $threw = $false
  try { & $Action } catch { $threw = $true }
  if (-not $threw) { throw "Expected failure did not occur: $Label" }
}

function Invoke-PrintOnly {
  param([Parameter(Mandatory)][string]$Path, [hashtable]$Parameters = @{})
  $arguments = @("-NoLogo", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", $Path)
  foreach ($key in $Parameters.Keys) {
    if ($Parameters[$key] -is [bool]) { if ($Parameters[$key]) { $arguments += "-$key" } } else { $arguments += @("-$key", [string]$Parameters[$key]) }
  }
  $output = @(& (Join-Path $PSHOME "powershell.exe") @arguments 2>&1)
  if ($null -ne $LASTEXITCODE -and $LASTEXITCODE -ne 0) { throw "PrintOnly failed for $Path" }
  return ($output | ForEach-Object { [string]$_ }) -join "`n"
}

function Assert-Contains {
  param([Parameter(Mandatory)][string]$Text, [Parameter(Mandatory)][string]$Value)
  if ($Text.IndexOf($Value, [StringComparison]::Ordinal) -lt 0) { throw "Missing expected text: $Value" }
}

if ((Get-PerformanceBaselineOfflineCells $manifest "raspberry-pi-zero-2w").Count -ne 30 -or @($manifest.raspberry.live_cells).Count -ne 3) { throw "Manifest Raspberry plan counts changed." }
foreach ($cell in @($manifest.raspberry.live_cells)) { if ($null -ne $cell.callback_fields -or ($cell.probe_modes -join ",") -cne "Live,AudioDrain") { throw "Raspberry live callback or mode contract changed." } }
$offlinePlan = @(Get-PerformanceBaselineRoundRobinPlan (Get-PerformanceBaselineOfflineCells $manifest "raspberry-pi-zero-2w") $manifest.repetitions)
if ($offlinePlan.Count -ne 90 -or $offlinePlan[0].Repetition -ne 1 -or $offlinePlan[29].Repetition -ne 1 -or $offlinePlan[30].Repetition -ne 2 -or $offlinePlan[89].Repetition -ne 3) { throw "Raspberry offline plan is not round-robin by repetition." }
if (-not (Test-PerformanceBaselineMeasuredOutcome "over_budget") -or (Test-PerformanceBaselineMeasuredOutcome "safety_failure") -or (Test-PerformanceBaselineMeasuredOutcome "thermal_failure") -or (Test-PerformanceBaselineMeasuredOutcome "restoration_failure") -or (Test-PerformanceBaselineMeasuredOutcome "infrastructure_failure")) { throw "Measured/fatal outcome policy is incorrect." }

$canary = Invoke-PrintOnly $driver @{ PrintOnly = $true; CanaryOnly = $true }
Assert-Contains $canary "Raspberry performance baseline PrintOnly: no transport is invoked."
Assert-Contains $canary "Target: pi@192.168.0.218"
Assert-Contains $canary "02: offline repetition=1/3 cell=common_baseline_idle"
Assert-Contains $canary "05: live mode=Live repetition=1/3 cell=raspberry_live_output_256 scenario=pulses-stress output=256 internal=256 workers=2 callback=null"
Assert-Contains $canary "06: live mode=AudioDrain repetition=1/3 cell=raspberry_live_output_256 scenario=pulses-stress output=256 internal=256 workers=2 callback=null"
if ($canary -match "11:|with-pi-ssh.ps1.*ssh-payload") { throw "Raspberry canary PrintOnly emitted an extra cell or transport." }
$full = Invoke-PrintOnly $driver @{ PrintOnly = $true; Phase = "Full" }
Assert-Contains $full "offline repetition=2/3"
Assert-Contains $full "raspberry_live_output_128"
Assert-Contains $full "raspberry_live_output_512"
Assert-Throws { & $driver -Phase Offline -Metadata missing.json } "active consent"
Assert-Throws { & $driver -Phase Offline -AllowServiceInterruption -Metadata missing.json } "exact metadata"

$baselinePrint = Invoke-PrintOnly $runner @{ Mode = "ProfileBaseline"; Scenario = "synth_cross_slot_16"; AudioBlockFrames = 256; ProfileMeasureFrames = 256; SynthSlotWorkers = 2; PrintOnly = $true }
Assert-Contains $baselinePrint "OCTESSERA_PI_PROFILE_MODE='baseline'"
Assert-Contains $baselinePrint "OCTESSERA_PI_PROFILE_SAMPLE_RATE='44100'"
Assert-Contains $baselinePrint "OCTESSERA_PI_PROFILE_SCENARIO='synth_cross_slot_16'"
Assert-Contains $baselinePrint "OCTESSERA_AUDIO_BLOCK_FRAMES='256'"
Assert-Contains $baselinePrint "OCTESSERA_PI_PROFILE_MEASURE_FRAMES='256'"
Assert-Contains $baselinePrint "OCTESSERA_SYNTH_SLOT_WORKERS='2'"
Assert-Contains $baselinePrint "sudo systemctl stop 'octessera.service'"
Assert-Contains $baselinePrint "sudo systemctl start 'octessera.service'"
Assert-Contains $baselinePrint "restore_status"
Assert-Contains $baselinePrint "startup) temperature_limit=70000"
Assert-Contains $baselinePrint "runtime) temperature_limit=75000"
Assert-Contains $baselinePrint '[ "$thermal_max" -ge "$temperature_limit" ]'
$livePrint = Invoke-PrintOnly $runner @{ Mode = "Live"; Durations = "30s"; Scenarios = "pulses-stress"; AudioOutputBufferFrames = 128; AudioBlockFrames = 256; SynthSlotWorkers = 2; AllowServiceInterruption = $true; PrintOnly = $true }
Assert-Contains $livePrint "OCTESSERA_AUDIO_OUTPUT_BUFFER_FRAMES='128'"
Assert-Contains $livePrint "--timing-probe-scenarios 'pulses-stress'"
Assert-Contains $livePrint "restore_status"
$audioDrainPrint = Invoke-PrintOnly $runner @{ Mode = "AudioDrain"; Durations = "30s"; AudioOutputBufferFrames = 512; AudioBlockFrames = 256; SynthSlotWorkers = 2; AllowServiceInterruption = $true; PrintOnly = $true }
Assert-Contains $audioDrainPrint "OCTESSERA_AUDIO_OUTPUT_BUFFER_FRAMES='512'"
Assert-Contains $audioDrainPrint "--timing-probe-audio-drain"
Assert-Contains $audioDrainPrint "restore_status"
Assert-Throws { & $runner -Mode ProfileBaseline -Scenario synth_cross_slot_16 -AudioBlockFrames 256 -ProfileMeasureFrames 256 -SynthSlotWorkers 2 } "runner consent"
Assert-Throws { & $runner -Mode Live -Durations 30s } "live runner consent"
Assert-Throws { & $runner -Mode ProfileBaseline -Scenario unknown_scenario -AudioBlockFrames 256 -ProfileMeasureFrames 256 -SynthSlotWorkers 2 -PrintOnly } "native unknown ID remains fail closed at the host contract"
if ($runnerSource -notmatch "RuntimeOnly|DspFxLimits|DspSoak") { throw "Raspberry timing runner legacy modes were not retained." }
if ($driverSource -match "with-orange-ssh|run-orange-performance-baseline") { throw "Raspberry driver references Orange tooling." }
if ($runnerSource -match "with-orange-ssh|run-orange-performance-baseline") { throw "Raspberry runner references Orange tooling." }
Assert-Contains $driverSource 'CallbackFields = $null'
Assert-Contains $driverSource "Invoke-PiLiveProbe"
Assert-Contains $driverSource "local_sha256"
Assert-Contains $driverSource "Test-PerformanceBaselineMeasuredOutcome"
Assert-Contains $driverSource "Assert-RaspberrySystemEvidence"
Assert-Contains $driverSource '-BinaryPath $Artifact'
Assert-Contains $runnerSource "raspberry_system_sample"
Assert-Contains $runnerSource "vcgencmd get_throttled"
Assert-Contains $runnerSource "restore_status"
Assert-Throws { Assert-PerformanceBaselinePath "bad`"path" "quoted test path" } "quoted path"
$spacedDirectory = Join-Path ([IO.Path]::GetTempPath()) ("octessera pi spaced " + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $spacedDirectory | Out-Null
$spacedRunner = Join-Path $spacedDirectory "runner.ps1"
Copy-Item -LiteralPath $runner -Destination $spacedRunner
$spaced = Invoke-PrintOnly $driver @{ PrintOnly = $true; RunnerPath = $spacedRunner }
Assert-Contains $spaced "Raspberry performance baseline PrintOnly: no transport is invoked."
Remove-Item -LiteralPath $spacedDirectory -Recurse -Force

foreach ($payload in @($baselinePrint, $livePrint, $audioDrainPrint)) {
  $payloadPath = Join-Path ([IO.Path]::GetTempPath()) ("octessera-pi-baseline-payload-" + [guid]::NewGuid().ToString("N") + ".sh")
  try {
    [IO.File]::WriteAllText($payloadPath, $payload, (New-Object System.Text.UTF8Encoding($false)))
    $bash = Get-Command bash -ErrorAction SilentlyContinue
    if ($null -ne $bash -and [string]$bash.Source -notmatch "WindowsApps") {
      & bash -n $payloadPath
      if ($LASTEXITCODE -ne 0) { throw "Generated Raspberry baseline payload failed bash -n." }
    } elseif ($null -ne (Get-Command wsl.exe -ErrorAction SilentlyContinue)) {
      $drive = $payloadPath.Substring(0, 1).ToLowerInvariant()
      $wslPath = "/mnt/$drive" + ($payloadPath.Substring(2) -replace "\\", "/")
      & wsl.exe bash -n $wslPath
      if ($LASTEXITCODE -ne 0) { throw "Generated Raspberry baseline payload failed WSL bash -n." }
    }
  } finally {
    Remove-Item -LiteralPath $payloadPath -Force -ErrorAction SilentlyContinue
  }
}

Write-Output "Raspberry performance baseline manifest, plan, consent, isolation, continuation, nullable evidence, and payload tests passed"
