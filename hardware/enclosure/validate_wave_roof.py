from __future__ import annotations

import json
import math
import sys

import cadquery as cq
from OCP.BRep import BRep_Tool
from OCP.TopAbs import TopAbs_VERTEX
from OCP.TopExp import TopExp_Explorer
from OCP.TopoDS import TopoDS

import generate_two_level_enclosure_cadquery as cad
import top_wave_geometry as wave


MAX_LOW_WALL_SHIFT_MM = 0.25
MIN_BOTTOM_FOOTPRINT_MM = cad.SOUTH_ROOF_LOW_WALL_BAND - 0.2
ROOF_SAMPLE_TOLERANCE = 0.12
ROOF_SAMPLES = [
    ("Pi block roof west", 19.0, 18.0, cad.HIGH_Z - 0.2),
    ("Pi block roof east", 85.0, 18.0, cad.HIGH_Z - 0.2),
    ("tier-1 between craters", 45.0, 65.0, cad.LOW_Z - 0.2),
    ("tier-1 east of craters", 80.0, 80.0, cad.LOW_Z - 0.2),
    ("NeoTrellis northwest roof", 132.0, 130.0, cad.HIGH_Z - 0.2),
    ("NeoTrellis northeast roof", 240.0, 130.0, cad.HIGH_Z - 0.2),
    ("Pi block north slope", 55.0, 33.0, cad.HIGH_Z - 1.0),
    ("Pi block west wall", 1.0, 5.0, cad.LOW_Z + 1.0),
    ("NeoKey roof north extension", 115.0, 130.0, cad.HIGH_Z - 0.2),
    ("NeoKey north slope", 104.0, 120.0, cad.HIGH_Z - 1.0),
]


def solid_contains_point(solid, point: cq.Vector, tolerance: float) -> bool:
    return solid.isInside(point, tolerance)


def section_z_bounds(model: cq.Workplane, x: float, y: float) -> tuple[float, float]:
    probe = cq.Workplane("XY").box(0.001, 0.001, 10.0).translate((x, y, 13.0))
    pieces = model.intersect(probe).solids().vals()
    if not pieces:
        raise ValueError(f"section is empty at x={x}, y={y}")
    return (
        min(piece.BoundingBox().zmin for piece in pieces),
        max(piece.BoundingBox().zmax for piece in pieces),
    )


def check_lower_wave_blend(model: cq.Workplane) -> None:
    profile_points, profile_edges = wave.lower_wave_top_profile(52.5, wave.PI_BLOCK_NORTH_Y)
    blend_edge = profile_edges[0]
    deck_tangent = cq.Vector(0, -1, 0)
    next_segment_tangent = profile_points[5] - profile_points[4]
    for name, actual, expected in [
        ("deck tangent", blend_edge.tangentAt(0), deck_tangent),
        ("retained roof tangent", blend_edge.tangentAt(1), next_segment_tangent),
    ]:
        angle = math.degrees(math.acos(max(-1.0, min(1.0, actual.normalized().dot(expected.normalized())))))
        print(f"{name}_angle_deg={angle:.6f}")
        if angle > 0.1:
            raise ValueError(f"Bezier blend is not tangent to {name}: {angle:.6f} degrees")

    low_end = section_z_bounds(model, 52.5, 38.35)
    blend_end = section_z_bounds(model, 52.5, 35.5166666667)
    high_end = section_z_bounds(model, 52.5, 29.85)
    blend_height = profile_points[4].z
    for name, measured, expected in [
        ("deck endpoint top", low_end[1], wave.LOW_Z),
        ("blend endpoint top", blend_end[1], blend_height),
        ("retained roof endpoint top", high_end[1], wave.HIGH_Z),
        ("retained roof endpoint underside", high_end[0], wave.HIGH_UNDERSIDE_Z),
    ]:
        print(f"{name}_z_mm={measured:.6f} expected={expected:.6f}")
        if abs(measured - expected) > 0.001:
            raise ValueError(f"{name} differs from expected by more than 0.001 mm")
    blend_length = 4.0 * wave.SOUTH_SHOULDER_PLAN_WIDTH / wave.SLOPE_PROFILE_STEPS
    blend_sections = [
        section_z_bounds(model, 52.5, 38.35 - blend_length * index / 4.0)
        for index in range(5)
    ]
    minimum_blend_thickness = min(top - bottom for bottom, top in blend_sections)
    print(f"minimum_blend_thickness_mm={minimum_blend_thickness:.6f}")
    if minimum_blend_thickness < 3.0:
        raise ValueError("lower-wave blend has less than 3 mm vertical thickness")

    before_endpoint = section_z_bounds(model, 52.5, 38.349)[1]
    after_endpoint = section_z_bounds(model, 52.5, 38.351)[1]
    if abs(before_endpoint - after_endpoint) > 0.001:
        raise ValueError("lower-wave blend has a Z step at the deck endpoint")

    west_inner = section_z_bounds(model, 2.85, 36.75208333)[1]
    west_outer = section_z_bounds(model, 2.95, 36.75208333)[1]
    wall_cap_delta = abs(west_inner - west_outer)
    print(f"west_wall_cap_delta_mm={wall_cap_delta:.6f}")
    if wall_cap_delta > 0.001:
        raise ValueError("west wave wall cap does not match the roof across its boundary")


def wire_vertices(wire) -> list[tuple[float, float, float]]:
    vertices = []
    seen = set()
    explorer = TopExp_Explorer(wire.wrapped, TopAbs_VERTEX)
    while explorer.More():
        point = BRep_Tool.Pnt_s(TopoDS.Vertex_s(explorer.Current()))
        key = (round(point.X(), 4), round(point.Y(), 4), round(point.Z(), 4))
        if key not in seen:
            seen.add(key)
            vertices.append((point.X(), point.Y(), point.Z()))
        explorer.Next()
    return vertices


def profile_rows() -> list[tuple[int, float, float, float, float]]:
    high, low = cad.south_edge_samples()
    rows = []
    for index, (low_point, high_point) in enumerate(zip(low, high)):
        vector_x = high_point[0] - low_point[0]
        vector_y = high_point[1] - low_point[1]
        vector_length = math.hypot(vector_x, vector_y)
        if vector_length == 0.0:
            continue
        unit_x = vector_x / vector_length
        unit_y = vector_y / vector_length
        values = []
        for x, y, z in wire_vertices(cad.shoulder_profile_wire(low_point, high_point)):
            section_distance = (x - low_point[0]) * unit_x + (y - low_point[1]) * unit_y
            values.append((section_distance, z))
        low_top = [s for s, z in values if abs(z - cad.LOW_Z) <= 0.02]
        low_bottom = [s for s, z in values if abs(z - cad.UNDERSIDE_Z) <= 0.02]
        if not low_top or not low_bottom:
            raise ValueError(f"profile {index} is missing low top or bottom vertices")
        shift = abs(min(low_bottom, key=abs) - min(low_top, key=abs))
        bottom_width = max(low_bottom) - min(low_bottom)
        rows.append((index, low_point[0], low_point[1], shift, bottom_width))
    return rows


def main() -> None:
    rows = profile_rows()
    worst_shift = max(rows, key=lambda row: row[3])
    narrowest_bottom = min(rows, key=lambda row: row[4])
    params = json.loads(cad.PARAMS.read_text())
    model = cad.build_model(params)
    solids = model.solids().vals()
    missing_samples = [
        name
        for name, x, y, z in ROOF_SAMPLES
        if not any(
            solid_contains_point(solid, cq.Vector(x, y, z), ROOF_SAMPLE_TOLERANCE)
            for solid in solids
        )
    ]
    print(f"worst_vertical_shift_mm={worst_shift[3]:.3f} at index={worst_shift[0]}")
    print(f"min_bottom_footprint_mm={narrowest_bottom[4]:.3f} at index={narrowest_bottom[0]}")
    slot_count = len(cad.load_guidance_slots())
    print(f"slots={slot_count}")
    print(f"valid={model.val().isValid()}")
    print(f"solids={len(model.solids().vals())}")
    check_lower_wave_blend(model)
    if missing_samples:
        raise SystemExit(f"FAIL: missing roof samples: {', '.join(missing_samples)}")
    if worst_shift[3] > MAX_LOW_WALL_SHIFT_MM:
        raise SystemExit("FAIL: low roof wall is slanted")
    if narrowest_bottom[4] < MIN_BOTTOM_FOOTPRINT_MM:
        raise SystemExit("FAIL: low roof wall bottom footprint is too narrow")
    if not model.val().isValid() or len(model.solids().vals()) != 1:
        raise SystemExit("FAIL: generated model is invalid")
    if slot_count == 0:
        raise SystemExit("FAIL: no ventilation slot guides found")
    print("PASS")


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        raise
