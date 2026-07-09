param(
    [switch]$SkipReview
)

$ErrorActionPreference = "Stop"
$repoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")
$top3mf = Join-Path $repoRoot "release-artifacts\enclosure\3mf-multicolor\case_top_two_level_branded_multicolor.3mf"

& powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "generate_top_artifacts_checked.ps1")

if (-not $?) {
    throw "top STEP/STL generation failed"
}

$script = @'
from pathlib import Path
import json
import sys
import zipfile

sys.path.insert(0, 'hardware/enclosure')
import generate_multimaterial_caps_3mf as multi
import generate_two_level_enclosure_cadquery as enclosure

params = json.loads(enclosure.PARAMS.read_text())
body, branding = multi.enclosure_top_variant(params)
out = multi.THREEMF_ROOT / 'case_top_two_level_branded_multicolor.3mf'
multi.THREEMF_ROOT.mkdir(parents=True, exist_ok=True)
multi.write_3mf(out, body, branding)

with zipfile.ZipFile(out) as package:
    config = package.read('Metadata/model_settings.config').decode()

print(f'wrote {out}')
print(f'body_valid={body.val().isValid()} body_solids={len(body.solids().vals())}')
print(f'branding_solids={len(branding.solids().vals())} branding_zmax={branding.val().BoundingBox().zmax:.3f}')
print(f'extruder2={"extruder\" value=\"2" in config}')
print('__BRANDED_3MF_DONE__')
'@

$script | python -

if (-not $?) {
    throw "branded 3MF generation failed"
}

if (-not (Test-Path -LiteralPath $top3mf)) {
    throw "missing branded top 3MF: $top3mf"
}

if (-not $SkipReview) {
    python (Join-Path $PSScriptRoot "generate_wave_review_view.py")
    if (-not $?) {
        throw "review image generation failed"
    }
    Write-Output "__REVIEW_VIEW_DONE__"
}

Write-Output "__BRANDED_TOP_ARTIFACTS_DONE__"
