$ErrorActionPreference = "Stop"
$repoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")
$top3mfs = @(
    (Join-Path $repoRoot "release-artifacts\enclosure\3mf-multicolor\case_top_two_level_multicolor.3mf"),
    (Join-Path $repoRoot "release-artifacts\enclosure\3mf-multicolor\case_top_two_level_orange_pi_multicolor.3mf")
)

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

multi.THREEMF_ROOT.mkdir(parents=True, exist_ok=True)
params = json.loads(enclosure.PARAMS.read_text())
for variant_params, filename in [
    (params, 'case_top_two_level_multicolor.3mf'),
    (enclosure.orange_pi_top_params(params), 'case_top_two_level_orange_pi_multicolor.3mf'),
]:
    body, branding = multi.enclosure_top_variant(variant_params)
    out = multi.THREEMF_ROOT / filename
    multi.write_3mf(out, body, branding)

    with zipfile.ZipFile(out) as package:
        config = package.read('Metadata/model_settings.config').decode()

    has_extruder2 = 'extruder" value="2' in config
    print(f'wrote {out}')
    print(f'body_valid={body.val().isValid()} body_solids={len(body.solids().vals())}')
    print(f'branding_solids={len(branding.solids().vals())} branding_zmax={branding.val().BoundingBox().zmax:.3f}')
    print(f'extruder2={has_extruder2}')
print('__BRANDED_3MF_DONE__')
'@

$script | python -

if (-not $?) {
    throw "branded 3MF generation failed"
}

foreach ($top3mf in $top3mfs) {
    if (-not (Test-Path -LiteralPath $top3mf)) {
        throw "missing branded top 3MF: $top3mf"
    }
}

Write-Output "__BRANDED_TOP_ARTIFACTS_DONE__"
