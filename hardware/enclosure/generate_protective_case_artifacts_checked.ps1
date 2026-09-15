$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)

Push-Location $projectRoot
try {
    & python "hardware\enclosure\generate_protective_case_cadquery.py"
    if ($LASTEXITCODE -ne 0) {
        throw "Protective-case CAD generation failed with exit code $LASTEXITCODE"
    }
    "__PROTECTIVE_CASE_GENERATION_DONE__"

    & python "hardware\enclosure\validate_protective_case.py" --check-artifacts
    if ($LASTEXITCODE -ne 0) {
        throw "Protective-case validation failed with exit code $LASTEXITCODE"
    }
    "__PROTECTIVE_CASE_VALIDATION_DONE__"
}
finally {
    Pop-Location
}
