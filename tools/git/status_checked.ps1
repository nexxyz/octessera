$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)

Push-Location $projectRoot
try {
    & git status --short
    if ($LASTEXITCODE -ne 0) {
        throw "git status failed with exit code $LASTEXITCODE"
    }
    "__GIT_STATUS_DONE__"
}
finally {
    Pop-Location
}
