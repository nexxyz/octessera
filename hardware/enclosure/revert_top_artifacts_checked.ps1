$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$artifacts = @(
    "release-artifacts\enclosure\step\case_top_two_level_cadquery.step",
    "release-artifacts\enclosure\stl\case_top_two_level_cadquery.stl"
)

Push-Location $projectRoot
try {
    & git checkout -- @artifacts
    if ($LASTEXITCODE -ne 0) {
        throw "top artifact checkout failed with exit code $LASTEXITCODE"
    }
    "__ENCLOSURE_TOP_ARTIFACT_CHECKOUT_DONE__"
    & git status --short
    if ($LASTEXITCODE -ne 0) {
        throw "git status failed with exit code $LASTEXITCODE"
    }
    "__GIT_STATUS_DONE__"
}
finally {
    Pop-Location
}
