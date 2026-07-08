$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$artifactRoot = Join-Path $projectRoot "release-artifacts\enclosure\review"
$statusPath = Join-Path $artifactRoot "top_artifacts_async_status.json"

if (-not (Test-Path -LiteralPath $statusPath)) {
    "state=missing"
    "message=No async CAD status file found at $statusPath"
    "__CAD_ASYNC_STATUS_DONE__"
    exit 0
}

$status = Get-Content -LiteralPath $statusPath -Raw | ConvertFrom-Json
$state = $status.state
$pidText = if ($status.pid) { " pid=$($status.pid)" } else { "" }
$exitText = if ($null -ne $status.exitCode) { " exitCode=$($status.exitCode)" } else { "" }
"state=$state$pidText$exitText"
"updatedAt=$($status.updatedAt)"
"log=$($status.logPath)"
if ($status.message) {
    "message=$($status.message)"
}

if ($status.state -eq "running" -and $status.pid) {
    $process = Get-Process -Id $status.pid -ErrorAction SilentlyContinue
    if ($null -eq $process) {
        "warning=recorded process is no longer running; inspect log/status"
    }
}

if (Test-Path -LiteralPath $status.logPath) {
    try {
        $stream = [System.IO.File]::Open($status.logPath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
        try {
            $reader = New-Object System.IO.StreamReader($stream)
            $content = $reader.ReadToEnd()
        }
        finally {
            $reader.Close()
            $stream.Close()
        }
        $lines = $content -split "`r?`n"
        $start = [Math]::Max(0, $lines.Length - 20)
        "--- log tail ---"
        for ($index = $start; $index -lt $lines.Length; $index++) {
            $lines[$index]
        }
    }
    catch {
        "warning=could not read log tail: $($_.Exception.Message)"
    }
}

"__CAD_ASYNC_STATUS_DONE__"
