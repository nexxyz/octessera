$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)

Push-Location $projectRoot
try {
    & python "hardware\enclosure\generate_two_level_enclosure_cadquery.py"
    if ($LASTEXITCODE -ne 0) {
        throw "CAD generation failed with exit code $LASTEXITCODE"
    }
    "__CAD_GENERATION_DONE__"

    & python "hardware\enclosure\validate_wave_roof.py"
    if ($LASTEXITCODE -ne 0) {
        throw "Wave roof validation failed with exit code $LASTEXITCODE"
    }
    "__CAD_VALIDATION_DONE__"
}
finally {
    Pop-Location
}
