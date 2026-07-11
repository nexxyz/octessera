param(
    [Parameter(Mandatory = $true)]
    [string] $StatusPath,

    [Parameter(Mandatory = $true)]
    [string] $LogPath
)

$ErrorActionPreference = "Stop"
$OutputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::InputEncoding = [System.Text.UTF8Encoding]::new($false)
$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$checkedWrapper = Join-Path $PSScriptRoot "generate_branded_top_artifacts_checked.ps1"

function Write-Status($state, $exitCode = $null, $message = $null) {
    $payload = [ordered]@{
        state = $state
        pid = $PID
        updatedAt = (Get-Date).ToString("o")
        logPath = $LogPath
    }
    if ($null -ne $exitCode) {
        $payload.exitCode = $exitCode
    }
    if ($null -ne $message) {
        $payload.message = $message
    }
    $payload | ConvertTo-Json | Set-Content -LiteralPath $StatusPath -Encoding UTF8
}

Push-Location $projectRoot
try {
    Write-Status "running"
    "__BRANDED_CAD_ASYNC_WORKER_STARTED__" | Add-Content -LiteralPath $LogPath -Encoding UTF8

    & powershell -NoProfile -ExecutionPolicy Bypass -File $checkedWrapper 2>&1 | Out-File -LiteralPath $LogPath -Encoding utf8 -Append
    if ($LASTEXITCODE -ne 0) {
        throw "Branded top artifact generation failed with exit code $LASTEXITCODE"
    }

    Write-Status "succeeded" 0
}
catch {
    $_ | Out-String | Add-Content -LiteralPath $LogPath -Encoding UTF8
    Write-Status "failed" 1 $_.Exception.Message
    exit 1
}
finally {
    Pop-Location
}
