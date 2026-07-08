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
