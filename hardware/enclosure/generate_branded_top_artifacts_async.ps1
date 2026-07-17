$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$artifactRoot = Join-Path $projectRoot "release-artifacts\enclosure\status"
$statusPath = Join-Path $artifactRoot "branded_top_artifacts_async_status.json"
$workerPath = Join-Path $PSScriptRoot "generate_branded_top_artifacts_worker.ps1"

if (-not (Test-Path -LiteralPath $artifactRoot)) {
    New-Item -ItemType Directory -Path $artifactRoot | Out-Null
}

if (Test-Path -LiteralPath $statusPath) {
    try {
        $existingStatus = Get-Content -LiteralPath $statusPath -Raw | ConvertFrom-Json
        if ($existingStatus.state -eq "running" -and $existingStatus.pid) {
            $existingProcess = Get-Process -Id $existingStatus.pid -ErrorAction SilentlyContinue
            if ($null -ne $existingProcess) {
                "__BRANDED_CAD_ASYNC_ALREADY_RUNNING__ pid=$($existingStatus.pid)"
                "status=$statusPath"
                "log=$($existingStatus.logPath)"
                exit 0
            }
        }
    }
    catch {
        "warning=could not read existing async status: $($_.Exception.Message)"
    }
}

$timestamp = (Get-Date).ToString("yyyyMMdd-HHmmss")
$logPath = Join-Path $artifactRoot "branded_top_artifacts_async_$timestamp.log"

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

"__BRANDED_CAD_ASYNC_STARTED__ pid=$($process.Id)"
"status=$statusPath"
"log=$logPath"
