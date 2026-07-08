from __future__ import annotations

import math
from pathlib import Path

import cadquery as cq


ROOT = Path(__file__).resolve().parent
ARTIFACT_ROOT = ROOT.parent.parent / "release-artifacts" / "enclosure"
STEP_ROOT = ARTIFACT_ROOT / "step"
STL_ROOT = ARTIFACT_ROOT / "stl"

SHAFT_BORE_D = 6.15
SHAFT_FLAT_TO_ARC_MM = 4.65
BORE_CLEARANCE_Z = 0.2
AUX_MARK_DOT_R = 1.2
AUX_MARK_DOT_SPACING = 3.0
MAIN_MARK_DOT_COUNT = 11
MAIN_MARK_DOT_ORBIT_R = 7.35


def polar_points(radius: float, count: int, phase: float = 0.0) -> list[tuple[float, float]]:
    return [
        (
            radius * math.cos(2.0 * math.pi * index / count + phase),
            radius * math.sin(2.0 * math.pi * index / count + phase),
        )
        for index in range(count)
    ]


def d_bore_profile(radius: float, flat_to_arc: float) -> list[tuple[float, float]]:
    flat_y = radius - flat_to_arc
    start = math.asin(flat_y / radius)
    end = math.pi - start
    samples = 42
    return [
        (radius * math.cos(start + (end - start) * index / samples), radius * math.sin(start + (end - start) * index / samples))
        for index in range(samples + 1)
    ]


def d_bore_cutter(depth: float) -> cq.Workplane:
    radius = SHAFT_BORE_D / 2.0
    points = d_bore_profile(radius, SHAFT_FLAT_TO_ARC_MM)
    return (
        cq.Workplane("XY")
        .polyline(points)
        .close()
        .extrude(depth + BORE_CLEARANCE_Z)
        .translate((0.0, 0.0, -BORE_CLEARANCE_Z))
    )


def diagonal_groove(angle: float, z: float, handedness: int, radius: float) -> cq.Workplane:
    radial = cq.Vector(math.cos(angle), math.sin(angle), 0.0)
    tangent = cq.Vector(-math.sin(angle), math.cos(angle), 0.0)
    origin = radial.multiply(radius + 0.08) + cq.Vector(0.0, 0.0, z)
    plane = cq.Plane(origin, tangent, radial)
    length = 4.6
    width = 0.52
    tilt = handedness * math.radians(38.0)
    ux = math.cos(tilt)
    uy = math.sin(tilt)
    vx = -uy
    vy = ux
    points = [
        (ux * length / 2.0 + vx * width / 2.0, uy * length / 2.0 + vy * width / 2.0),
        (-ux * length / 2.0 + vx * width / 2.0, -uy * length / 2.0 + vy * width / 2.0),
        (-ux * length / 2.0 - vx * width / 2.0, -uy * length / 2.0 - vy * width / 2.0),
        (ux * length / 2.0 - vx * width / 2.0, uy * length / 2.0 - vy * width / 2.0),
    ]
    return cq.Workplane(plane).polyline(points).close().extrude(-0.55)


def add_diamond_knurl_cuts(body: cq.Workplane, radius: float, height: float) -> cq.Workplane:
    for row, z in enumerate([2.0, 4.0, 6.0, 8.0, 10.0]):
        offset = (row % 2) * math.pi / 18.0
        for index in range(18):
            angle = 2.0 * math.pi * index / 18.0 + offset
            body = body.cut(diagonal_groove(angle, z, 1, radius))
            body = body.cut(diagonal_groove(angle, z, -1, radius))
    return body


def make_wide_knurled_cap() -> cq.Workplane:
    height = 12.5
    radius = 10.0
    body = cq.Workplane("XY").circle(radius).extrude(height)
    body = add_diamond_knurl_cuts(body, radius, height)
    body = body.faces(">Z").fillet(0.7)
    body = body.faces("<Z").fillet(0.45)
    body = body.cut(d_bore_cutter(height - 1.4))
    return body.clean()


def dot_marking(count: int, radius: float, spacing: float, depth: float, z: float) -> cq.Workplane:
    dots = cq.Workplane("XY")
    start_x = -spacing * (count - 1) / 2.0
    for index in range(count):
        dot = cq.Workplane("XY").circle(radius).extrude(depth).translate((start_x + index * spacing, 0.0, z))
        dots = dots.union(dot)
    return dots.clean()


def perimeter_dot_marking(count: int, orbit_radius: float, dot_radius: float, depth: float, z: float) -> cq.Workplane:
    dots = cq.Workplane("XY")
    for index in range(count):
        angle = 2.0 * math.pi * index / count + math.pi / 2.0
        x = orbit_radius * math.cos(angle)
        y = orbit_radius * math.sin(angle)
        dot = cq.Workplane("XY").circle(dot_radius).extrude(depth).translate((x, y, z))
        dots = dots.union(dot)
    return dots.clean()


def add_separate_marking(body: cq.Workplane, marking: cq.Workplane) -> cq.Workplane:
    solids = [body.solids().vals()[0], *marking.solids().vals()]
    compound = cq.Compound.makeCompound(solids)
    return cq.Workplane("XY").add(compound)


def make_main_cap() -> cq.Workplane:
    return add_separate_marking(
        make_wide_knurled_cap(),
        perimeter_dot_marking(MAIN_MARK_DOT_COUNT, MAIN_MARK_DOT_ORBIT_R, AUX_MARK_DOT_R, 0.65, 12.5),
    )


def make_aux_cap_body() -> cq.Workplane:
    height = 10.5
    flange_h = 2.0
    flange_r = 8.4
    body_r = 6.0
    body = (
        cq.Workplane("XY")
        .circle(flange_r)
        .workplane(offset=flange_h)
        .circle(body_r)
        .loft(combine=True)
        .union(cq.Workplane("XY").circle(body_r).extrude(height - flange_h).translate((0, 0, flange_h - 0.05)))
        .clean()
    )
    body = body.faces(">Z").fillet(0.45)
    body = body.faces("<Z").fillet(0.45)
    for index in range(14):
        angle = 2.0 * math.pi * index / 14
        radial = cq.Vector(math.cos(angle), math.sin(angle), 0.0)
        tangent = cq.Vector(-math.sin(angle), math.cos(angle), 0.0)
        plane = cq.Plane(radial.multiply(body_r + 0.04) + cq.Vector(0, 0, 5.8), tangent, radial)
        groove = cq.Workplane(plane).rect(1.05, 5.7).extrude(-0.65)
        body = body.cut(groove)
    body = body.cut(d_bore_cutter(height - 1.2))
    return body.clean()


def make_aux_cap(dot_count: int) -> cq.Workplane:
    return add_separate_marking(
        make_aux_cap_body(),
        dot_marking(dot_count, AUX_MARK_DOT_R, AUX_MARK_DOT_SPACING, 0.65, 10.5),
    )


def export_cap(name: str, model: cq.Workplane) -> None:
    STEP_ROOT.mkdir(parents=True, exist_ok=True)
    STL_ROOT.mkdir(parents=True, exist_ok=True)
    for old_name in [
        "encoder_cap_wide_knurled",
        "encoder_cap_low_cone",
    ]:
        for suffix in ["step", "stl"]:
            old_path = (STEP_ROOT if suffix == "step" else STL_ROOT) / f"{old_name}.{suffix}"
            if old_path.exists():
                old_path.unlink()
    step_path = STEP_ROOT / f"{name}.step"
    stl_path = STL_ROOT / f"{name}.stl"
    cq.exporters.export(model, str(step_path))
    cq.exporters.export(model, str(stl_path), tolerance=0.04, angularTolerance=0.08)
    print(f"wrote {step_path}")
    print(f"wrote {stl_path}")


def main() -> None:
    caps = {
        "encoder_cap_main_knurled_dots": make_main_cap(),
        "encoder_cap_aux1_ribbed_dot": make_aux_cap(1),
        "encoder_cap_aux2_ribbed_dots": make_aux_cap(2),
        "encoder_cap_aux3_ribbed_dots": make_aux_cap(3),
    }
    for name, model in caps.items():
        solid_count = len(model.solids().vals())
        if solid_count < 2 or not model.val().isValid():
            raise SystemExit(f"{name} invalid: solids={solid_count} valid={model.val().isValid()}")
        export_cap(name, model)


if __name__ == "__main__":
    main()
