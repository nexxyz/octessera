from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parent
ARTIFACT_ROOT = ROOT.parent.parent / "release-artifacts" / "enclosure"
PARAMS = ROOT / "enclosure_params.json"
STEP_OUT = ARTIFACT_ROOT / "step" / "case_top_two_level_cadquery_raspberry-pi-zero-2w.step"
STL_OUT = ARTIFACT_ROOT / "stl" / "case_top_two_level_cadquery_raspberry-pi-zero-2w.stl"
ORANGE_PI_STEP_OUT = ARTIFACT_ROOT / "step" / "case_top_two_level_cadquery_orange-pi-zero-2w.step"
ORANGE_PI_STL_OUT = ARTIFACT_ROOT / "stl" / "case_top_two_level_cadquery_orange-pi-zero-2w.stl"


def load_params() -> dict:
    return json.loads(PARAMS.read_text())
