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
    "__PROTECTIVE_CASE_ASYNC_WORKER_STARTED__" | Set-Content -LiteralPath $LogPath -Encoding UTF8

    & python "hardware\enclosure\generate_protective_case_cadquery.py" 2>&1 | Out-File -LiteralPath $LogPath -Encoding utf8 -Append
    if ($LASTEXITCODE -ne 0) {
        throw "Protective-case CAD generation failed with exit code $LASTEXITCODE"
    }
    "__PROTECTIVE_CASE_GENERATION_DONE__" | Add-Content -LiteralPath $LogPath -Encoding UTF8

    & python "hardware\enclosure\validate_protective_case.py" --check-artifacts 2>&1 | Out-File -LiteralPath $LogPath -Encoding utf8 -Append
    if ($LASTEXITCODE -ne 0) {
        throw "Protective-case validation failed with exit code $LASTEXITCODE"
    }
    "__PROTECTIVE_CASE_VALIDATION_DONE__" | Add-Content -LiteralPath $LogPath -Encoding UTF8
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
