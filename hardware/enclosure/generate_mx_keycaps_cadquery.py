from __future__ import annotations

from pathlib import Path
import math

import cadquery as cq


ROOT = Path(__file__).resolve().parent
ARTIFACT_ROOT = ROOT.parent.parent / "release-artifacts" / "enclosure"
STEP_ROOT = ARTIFACT_ROOT / "step"
STL_ROOT = ARTIFACT_ROOT / "stl"

CAP_BOTTOM_W = 18.0
CAP_TOP_W = 15.0
CAP_HEIGHT = 9.0
CAP_CORNER_R = 2.0
TOP_THICKNESS = 1.6
WALL_THICKNESS = 1.1
STEM_BOSS_D = 6.2
STEM_BOSS_H = 7.7
MX_CROSS_ARM_LEN = 4.35
MX_CROSS_ARM_W = 1.35
MX_SOCKET_DEPTH = 5.2
ICON_SCALE = 0.598
FN_ICON_SCALE = 0.86
ICON_RAISE = 0.65


def rounded_rect_wire(width: float, depth: float, radius: float, z: float) -> cq.Wire:
    points = []
    segments = 8
    centers = [
        (width / 2.0 - radius, depth / 2.0 - radius, 0.0),
        (-width / 2.0 + radius, depth / 2.0 - radius, math.pi / 2.0),
        (-width / 2.0 + radius, -depth / 2.0 + radius, math.pi),
        (width / 2.0 - radius, -depth / 2.0 + radius, 3.0 * math.pi / 2.0),
    ]
    for cx, cy, start in centers:
        for index in range(segments + 1):
            angle = start + index * math.pi / 2.0 / segments
            points.append(cq.Vector(cx + radius * math.cos(angle), cy + radius * math.sin(angle), z))
    points.append(points[0])
    return cq.Wire.makePolygon(points)


def rounded_loft(bottom_w: float, top_w: float, height: float, radius: float) -> cq.Workplane:
    bottom = rounded_rect_wire(bottom_w, bottom_w, radius, 0.0)
    top = rounded_rect_wire(top_w, top_w, min(radius, top_w / 2.0 - 0.1), height)
    return cq.Workplane("XY").add(cq.Solid.makeLoft([bottom, top], ruled=True))


def mx_cross_cutter(depth: float) -> cq.Workplane:
    arm_a = cq.Workplane("XY").rect(MX_CROSS_ARM_LEN, MX_CROSS_ARM_W).extrude(depth + 0.2)
    arm_b = cq.Workplane("XY").rect(MX_CROSS_ARM_W, MX_CROSS_ARM_LEN).extrude(depth + 0.2)
    return arm_a.union(arm_b).translate((0.0, 0.0, -0.1))


def make_basic_mx_keycap() -> cq.Workplane:
    outer = rounded_loft(CAP_BOTTOM_W, CAP_TOP_W, CAP_HEIGHT, CAP_CORNER_R)
    inner_bottom_w = CAP_BOTTOM_W - 2.0 * WALL_THICKNESS
    inner_top_w = CAP_TOP_W - 2.0 * WALL_THICKNESS
    inner_h = CAP_HEIGHT - TOP_THICKNESS
    inner = rounded_loft(inner_bottom_w, inner_top_w, inner_h + 0.2, CAP_CORNER_R - 0.45).translate((0.0, 0.0, -0.1))
    shell = outer.cut(inner).clean()
    stem_boss = cq.Workplane("XY").circle(STEM_BOSS_D / 2.0).extrude(STEM_BOSS_H)
    cap = shell.union(stem_boss).clean()
    cap = cap.cut(mx_cross_cutter(MX_SOCKET_DEPTH)).clean()
    cap = cap.faces(">Z").fillet(0.45)
    cap = cap.faces("<Z").fillet(0.25)
    return cap.clean()


def icon_point(x: float, y: float, scale: float = ICON_SCALE) -> tuple[float, float]:
    return ((x - 12.0) * scale, (12.0 - y) * scale)


def polygon_icon(points: list[tuple[float, float]], scale: float = ICON_SCALE) -> cq.Workplane:
    return cq.Workplane("XY").polyline([icon_point(x, y, scale) for x, y in points]).close().extrude(ICON_RAISE).translate((0.0, 0.0, CAP_HEIGHT))


def bar_icon(x0: float, y0: float, x1: float, y1: float, width: float = 0.78, scale: float = ICON_SCALE) -> cq.Workplane:
    ax, ay = icon_point(x0, y0, scale)
    bx, by = icon_point(x1, y1, scale)
    dx = bx - ax
    dy = by - ay
    length = math.hypot(dx, dy)
    if length == 0.0:
        raise ValueError("zero-length icon bar")
    angle = math.degrees(math.atan2(dy, dx))
    return (
        cq.Workplane("XY")
        .rect(length, width)
        .extrude(ICON_RAISE)
        .rotate((0, 0, 0), (0, 0, 1), angle)
        .translate(((ax + bx) / 2.0, (ay + by) / 2.0, CAP_HEIGHT))
    )


def compound_keycap(body: cq.Workplane, icon_parts: list[cq.Workplane]) -> cq.Workplane:
    solids = [body.solids().vals()[0]]
    for part in icon_parts:
        solids.extend(part.solids().vals())
    return cq.Workplane("XY").add(cq.Compound.makeCompound(solids))


def back_icon() -> list[cq.Workplane]:
    return [
        polygon_icon(
            [
                (4.2, 11.4),
                (10.2, 5.8),
                (10.2, 9.1),
                (16.2, 9.1),
                (18.8, 9.8),
                (20.5, 11.8),
                (20.8, 13.9),
                (20.1, 16.4),
                (19.1, 18.6),
                (18.9, 16.4),
                (17.7, 14.5),
                (15.0, 13.7),
                (10.2, 13.7),
                (10.2, 17.0),
            ]
        )
    ]


def play_icon() -> list[cq.Workplane]:
    return [polygon_icon([(8.9, 5.6), (18.2, 12.0), (8.9, 18.4)])]


def shift_icon() -> list[cq.Workplane]:
    return [polygon_icon([(12.0, 4.6), (18.8, 11.4), (15.2, 11.4), (15.2, 18.8), (8.8, 18.8), (8.8, 11.4), (5.2, 11.4)])]


def layer_icon() -> list[cq.Workplane]:
    return [layer_rhombus(10.8), layer_rhombus(13.2)]


def layer_rhombus(center_y: float) -> cq.Workplane:
    outer = [(6.4, center_y), (12.0, center_y - 3.35), (17.6, center_y), (12.0, center_y + 3.35)]
    inner = [(9.0, center_y), (12.0, center_y - 1.6), (15.0, center_y), (12.0, center_y + 1.6)]
    outer_solid = polygon_icon(outer, scale=FN_ICON_SCALE)
    inner_cutter = (
        cq.Workplane("XY")
        .polyline([icon_point(x, y, FN_ICON_SCALE) for x, y in inner])
        .close()
        .extrude(ICON_RAISE + 0.4)
        .translate((0.0, 0.0, CAP_HEIGHT - 0.2))
    )
    return outer_solid.cut(inner_cutter).clean()


def make_labeled_keycap(icon_parts: list[cq.Workplane]) -> cq.Workplane:
    return compound_keycap(make_basic_mx_keycap(), icon_parts)


def export_keycap(name: str, model: cq.Workplane) -> None:
    STEP_ROOT.mkdir(parents=True, exist_ok=True)
    STL_ROOT.mkdir(parents=True, exist_ok=True)
    step_path = STEP_ROOT / f"{name}.step"
    stl_path = STL_ROOT / f"{name}.stl"
    cq.exporters.export(model, str(step_path))
    cq.exporters.export(model, str(stl_path), tolerance=0.04, angularTolerance=0.08)
    print(f"wrote {step_path}")
    print(f"wrote {stl_path}")


def main() -> None:
    caps = {
        "mx_keycap_back": make_labeled_keycap(back_icon()),
        "mx_keycap_play": make_labeled_keycap(play_icon()),
        "mx_keycap_shift": make_labeled_keycap(shift_icon()),
        "mx_keycap_fn_layer": make_labeled_keycap(layer_icon()),
    }
    for name, model in caps.items():
        solid_count = len(model.solids().vals())
        if solid_count < 2 or not model.val().isValid():
            raise SystemExit(f"{name} invalid: solids={solid_count} valid={model.val().isValid()}")
        export_keycap(name, model)


if __name__ == "__main__":
    main()
