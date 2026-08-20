from __future__ import annotations

from pathlib import Path
from typing import cast

import cadquery as cq

from top_body_assembly import build_body_model
from top_branding_variants import build_branding_marking


def build_branded_export_model(params: dict) -> cq.Workplane:
    body = build_body_model(params)
    branding = build_branding_marking(params, cast(cq.Shape, body.val()).BoundingBox().zmin)
    solids = cast(list[cq.Shape], [*body.solids().vals(), *branding.solids().vals()])
    return cq.Workplane("XY").add(cq.Compound.makeCompound(solids))


def export_top_variant(params: dict, step_out: Path, stl_out: Path) -> None:
    model = build_branded_export_model(params)
    step_out.parent.mkdir(parents=True, exist_ok=True)
    stl_out.parent.mkdir(parents=True, exist_ok=True)
    cq.exporters.export(model, str(step_out))
    cq.exporters.export(model, str(stl_out), tolerance=0.08, angularTolerance=0.12)
    print(f"wrote {step_out}")
    print(f"wrote {stl_out}")
