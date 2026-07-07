from __future__ import annotations

import json
import math
import sys

import generate_bottom_plate_cadquery as bottom


def rounded_outline_margin(x: float, y: float, width: float, depth: float, radius: float) -> float:
    if radius <= x <= width - radius and 0.0 <= y <= depth:
        return min(y, depth - y)
    if radius <= y <= depth - radius and 0.0 <= x <= width:
        return min(x, width - x)
    cx = radius if x < radius else width - radius
    cy = radius if y < radius else depth - radius
    return radius - math.hypot(x - cx, y - cy)


def expected_counterbore_radius(spec: dict) -> float:
    diameter = bottom.CORNER_COUNTERBORE_D if bottom.is_corner_insert(spec) else bottom.COUNTERBORE_D
    return diameter / 2.0


def validate_insert_margins(params: dict) -> None:
    width, depth = params["case_size_v21"]
    radius = params["corner_r"]
    for spec in params["faceplate_insert_pillars_v22"]:
        x, y = spec["pos"]
        margin = rounded_outline_margin(x, y, width, depth, radius)
        required = expected_counterbore_radius(spec)
        print(f"{spec['name']}_outline_margin={margin:.3f} required={required:.3f}")
        if margin < required:
            raise SystemExit(f"FAIL: {spec['name']} counterbore breaks rounded outline")


def validate_component_pillar_margins(params: dict) -> None:
    width, depth = params["case_size_v21"]
    radius = params["corner_r"]
    for spec in params["bottom_component_support_pillars_v22"]:
        x, y = spec["pos"]
        diameter = (
            bottom.NEOTRELLIS_PILLAR_BASE_D
            if spec["component"] == "neotrellis"
            else bottom.COMPONENT_PILLAR_BASE_D
        )
        required = diameter / 2.0
        margin = rounded_outline_margin(x, y, width, depth, radius)
        print(f"{spec['name']}_outline_margin={margin:.3f} required={required:.3f}")
        if margin < required:
            raise SystemExit(f"FAIL: {spec['name']} pillar breaks rounded outline")


def validate_component_pillar_screw_clearance(params: dict) -> None:
    for pillar in params["bottom_component_support_pillars_v22"]:
        px, py = pillar["pos"]
        pillar_diameter = (
            bottom.NEOTRELLIS_PILLAR_BASE_D
            if pillar["component"] == "neotrellis"
            else bottom.COMPONENT_PILLAR_BASE_D
        )
        required = pillar_diameter / 2.0 + max(bottom.COUNTERBORE_D, bottom.CORNER_COUNTERBORE_D) / 2.0 + 0.4
        for screw in params["faceplate_insert_pillars_v22"]:
            sx, sy = screw["pos"]
            clearance = math.hypot(px - sx, py - sy)
            if clearance < required:
                raise SystemExit(
                    f"FAIL: {pillar['name']} is too close to screw/counterbore {screw['name']} "
                    f"({clearance:.3f} < {required:.3f})"
                )


def main() -> None:
    params = json.loads(bottom.PARAMS.read_text())
    plate = bottom.build_bottom_plate(params)
    bbox = plate.val().BoundingBox()
    print(f"valid={plate.val().isValid()}")
    print(f"solids={len(plate.solids().vals())}")
    print(f"bbox=({bbox.xmin:.3f},{bbox.xmax:.3f}) ({bbox.ymin:.3f},{bbox.ymax:.3f}) ({bbox.zmin:.3f},{bbox.zmax:.3f})")
    print(f"insert_count={len(params['faceplate_insert_pillars_v22'])}")
    print(f"component_pillar_count={len(params['bottom_component_support_pillars_v22'])}")
    validate_insert_margins(params)
    validate_component_pillar_margins(params)
    validate_component_pillar_screw_clearance(params)
    if not plate.val().isValid() or len(plate.solids().vals()) != 1:
        raise SystemExit("FAIL: bottom plate is invalid")
    max_component_height = max(
        spec.get("height", bottom.COMPONENT_PILLAR_HEIGHT)
        for spec in params["bottom_component_support_pillars_v22"]
    )
    expected_zmax = bottom.BOTTOM_PLATE_THICKNESS + max_component_height
    if abs(bbox.zmin) > 0.02 or abs(bbox.zmax - expected_zmax) > 0.02:
        raise SystemExit("FAIL: unexpected bottom plate z bounds")
    print("PASS")


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        raise
