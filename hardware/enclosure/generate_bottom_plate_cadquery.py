from __future__ import annotations

import json
from pathlib import Path

import cadquery as cq


ROOT = Path(__file__).resolve().parent
ARTIFACT_ROOT = ROOT.parent.parent / "release-artifacts" / "enclosure"
PARAMS = ROOT / "enclosure_params.json"
STEP_OUT = ARTIFACT_ROOT / "step" / "case_bottom_plate_cadquery.step"
STL_OUT = ARTIFACT_ROOT / "stl" / "case_bottom_plate_cadquery.stl"
BOTTOM_PLATE_THICKNESS = 3.2
GUIDE_WALL_OFFSET = 3.15
GUIDE_WALL_THICKNESS = 1.15
GUIDE_WALL_HEIGHT = 1.6
COMPONENT_PILLAR_D = 5.0
COMPONENT_PILLAR_BASE_D = 7.0
COMPONENT_PILLAR_HOLE_D = 2.85
COMPONENT_PILLAR_HEIGHT = 5.0
NEOTRELLIS_PILLAR_BASE_D = 7.0
NEOTRELLIS_PILLAR_HOLE_D = 2.70
SCREW_CLEARANCE_D = 3.4
COUNTERBORE_D = 6.4
CORNER_COUNTERBORE_D = 5.8
COUNTERBORE_DEPTH = 1.8
EPS = 0.15


def rounded_plate(width: float, depth: float, radius: float, thickness: float) -> cq.Workplane:
    sketch = cq.Sketch().rect(width, depth).vertices().fillet(radius)
    return (
        cq.Workplane("XY")
        .placeSketch(sketch)
        .extrude(thickness)
        .translate((width / 2.0, depth / 2.0, 0.0))
    )


def is_corner_insert(spec: dict) -> bool:
    return spec["name"] in {"NW", "NE", "SW", "SE"}


def guide_wall(params: dict) -> cq.Workplane:
    width, depth = params["case_size_v21"]
    radius = params["corner_r"]
    outer_w = width - 2.0 * GUIDE_WALL_OFFSET
    outer_d = depth - 2.0 * GUIDE_WALL_OFFSET
    inner_w = outer_w - 2.0 * GUIDE_WALL_THICKNESS
    inner_d = outer_d - 2.0 * GUIDE_WALL_THICKNESS
    outer_r = max(radius - GUIDE_WALL_OFFSET, 0.1)
    inner_r = max(outer_r - GUIDE_WALL_THICKNESS, 0.1)
    outer = rounded_plate(outer_w, outer_d, outer_r, GUIDE_WALL_HEIGHT).translate(
        (GUIDE_WALL_OFFSET, GUIDE_WALL_OFFSET, BOTTOM_PLATE_THICKNESS)
    )
    inner = rounded_plate(inner_w, inner_d, inner_r, GUIDE_WALL_HEIGHT + 2 * EPS).translate(
        (
            GUIDE_WALL_OFFSET + GUIDE_WALL_THICKNESS,
            GUIDE_WALL_OFFSET + GUIDE_WALL_THICKNESS,
            BOTTOM_PLATE_THICKNESS - EPS,
        )
    )
    wall = outer.cut(inner).clean()
    return add_screw_keepouts(wall, params).clean()


def add_screw_keepouts(model: cq.Workplane, params: dict) -> cq.Workplane:
    keepout_r = max(COUNTERBORE_D, CORNER_COUNTERBORE_D) / 2.0 + 0.4
    for spec in params["faceplate_insert_pillars_v22"]:
        x, y = spec["pos"]
        cutter = (
            cq.Workplane("XY")
            .circle(keepout_r)
            .extrude(GUIDE_WALL_HEIGHT + 2 * EPS)
            .translate((x, y, BOTTOM_PLATE_THICKNESS - EPS))
        )
        model = model.cut(cutter)
    return model.clean()


def component_support_pillars(params: dict) -> cq.Workplane:
    pillars = cq.Workplane("XY")
    for spec in params["bottom_component_support_pillars_v22"]:
        x, y = spec["pos"]
        height = spec.get("height", COMPONENT_PILLAR_HEIGHT)
        base_d = (
            NEOTRELLIS_PILLAR_BASE_D
            if spec["component"] == "neotrellis"
            else COMPONENT_PILLAR_BASE_D
        )
        hole_d = (
            NEOTRELLIS_PILLAR_HOLE_D
            if spec["component"] == "neotrellis"
            else COMPONENT_PILLAR_HOLE_D
        )
        pillar = (
            cq.Workplane("XY")
            .circle(base_d / 2.0)
            .workplane(offset=height)
            .circle(COMPONENT_PILLAR_D / 2.0)
            .loft()
            .translate((x, y, BOTTOM_PLATE_THICKNESS))
        )
        hole = (
            cq.Workplane("XY")
            .circle(hole_d / 2.0)
            .extrude(height + 2 * EPS)
            .translate((x, y, BOTTOM_PLATE_THICKNESS - EPS))
        )
        pillars = pillars.union(pillar.cut(hole))
    return pillars.clean()


def add_screw_holes(plate: cq.Workplane, params: dict) -> cq.Workplane:
    clearance_r = SCREW_CLEARANCE_D / 2.0
    for spec in params["faceplate_insert_pillars_v22"]:
        x, y = spec["pos"]
        counterbore_d = CORNER_COUNTERBORE_D if is_corner_insert(spec) else COUNTERBORE_D
        through_hole = (
            cq.Workplane("XY")
            .circle(clearance_r)
            .extrude(BOTTOM_PLATE_THICKNESS + 2 * EPS)
            .translate((x, y, -EPS))
        )
        counterbore = (
            cq.Workplane("XY")
            .circle(counterbore_d / 2.0)
            .extrude(COUNTERBORE_DEPTH + EPS)
            .translate((x, y, -EPS))
        )
        plate = plate.cut(through_hole).cut(counterbore)
    return plate.clean()


def build_bottom_plate(params: dict) -> cq.Workplane:
    width, depth = params["case_size_v21"]
    plate = rounded_plate(width, depth, params["corner_r"], BOTTOM_PLATE_THICKNESS)
    plate = plate.union(guide_wall(params)).clean()
    plate = plate.union(component_support_pillars(params)).clean()
    return add_screw_holes(plate, params)


def main() -> None:
    params = json.loads(PARAMS.read_text())
    plate = build_bottom_plate(params)
    STEP_OUT.parent.mkdir(parents=True, exist_ok=True)
    STL_OUT.parent.mkdir(parents=True, exist_ok=True)
    cq.exporters.export(plate, str(STEP_OUT))
    cq.exporters.export(plate, str(STL_OUT), tolerance=0.08, angularTolerance=0.12)
    print(f"wrote {STEP_OUT}")
    print(f"wrote {STL_OUT}")


if __name__ == "__main__":
    main()
