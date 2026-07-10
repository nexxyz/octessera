$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$artifactRoot = Join-Path $projectRoot "release-artifacts\enclosure\status"
$statusPath = Join-Path $artifactRoot "top_artifacts_async_status.json"
$logPath = Join-Path $artifactRoot "top_artifacts_async.log"
$workerPath = Join-Path $PSScriptRoot "generate_top_artifacts_worker.ps1"

if (-not (Test-Path -LiteralPath $artifactRoot)) {
    New-Item -ItemType Directory -Path $artifactRoot | Out-Null
}

if (Test-Path -LiteralPath $logPath) {
    Remove-Item -LiteralPath $logPath -Force
}

$initialStatus = [ordered]@{
    state = "starting"
    updatedAt = (Get-Date).ToString("o")
    logPath = $logPath
}
$initialStatus | ConvertTo-Json | Set-Content -LiteralPath $statusPath -Encoding UTF8

$args = @(
    "-NoProfile",
    "-ExecutionPolicy", "Bypass",
    "-File", $workerPath,
    "-StatusPath", $statusPath,
    "-LogPath", $logPath
)
$process = Start-Process -FilePath "powershell" -ArgumentList $args -WindowStyle Hidden -PassThru

$launchStatus = [ordered]@{
    state = "running"
    pid = $process.Id
    updatedAt = (Get-Date).ToString("o")
    logPath = $logPath
}
$launchStatus | ConvertTo-Json | Set-Content -LiteralPath $statusPath -Encoding UTF8

"__CAD_ASYNC_STARTED__ pid=$($process.Id)"
"status=$statusPath"
"log=$logPath"
