from __future__ import annotations

import cadquery as cq


def rect_prism(x0: float, y0: float, x1: float, y1: float, z0: float, z1: float) -> cq.Workplane:
    return (
        cq.Workplane("XY")
        .rect(x1 - x0, y1 - y0)
        .extrude(z1 - z0)
        .translate(((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0))
    )


def rounded_rect_prism(
    x0: float, y0: float, x1: float, y1: float, radius: float, z0: float, z1: float
) -> cq.Workplane:
    width = x1 - x0
    depth = y1 - y0
    sketch = cq.Sketch().rect(width, depth).vertices().fillet(radius)
    return (
        cq.Workplane("XY")
        .placeSketch(sketch)
        .extrude(z1 - z0)
        .translate(((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0))
    )


def northwest_rounded_rect_prism(
    x0: float, y0: float, x1: float, y1: float, radius: float, z0: float, z1: float
) -> cq.Workplane:
    arc_offset = radius * (1.0 - 2.0**-0.5)
    return (
        cq.Workplane("XY")
        .moveTo(x0, y0)
        .lineTo(x1, y0)
        .lineTo(x1, y1)
        .lineTo(x0 + radius, y1)
        .threePointArc((x0 + arc_offset, y1 - arc_offset), (x0, y1 - radius))
        .close()
        .extrude(z1 - z0)
        .translate((0, 0, z0))
    )


def neokey_support_block(
    params: dict,
    seat_bounds: tuple[float, float, float, float],
    bottom_z: float,
    tier1_top_z: float,
    top_z: float,
) -> cq.Workplane:
    seat_x0, seat_y0, seat_x1, seat_y1 = seat_bounds
    lower = rect_prism(seat_x0, seat_y0, seat_x1, seat_y1, bottom_z, tier1_top_z)
    upper = northwest_rounded_rect_prism(
        seat_x0,
        seat_y0,
        seat_x1,
        seat_y1,
        params["key_cutout_r"],
        tier1_top_z,
        top_z,
    )
    return lower.union(upper).clean()


def neokey_south_filler(
    seat_bounds: tuple[float, float, float, float], bottom_z: float, top_z: float
) -> cq.Workplane:
    seat_x0, seat_y0, seat_x1, _ = seat_bounds
    return rect_prism(seat_x0, 0.0, seat_x1, seat_y0 + 0.25, bottom_z, top_z)


def neokey_deck_cap(
    params: dict,
    seat_bounds: tuple[float, float, float, float],
    bottom_z: float,
    top_z: float,
) -> cq.Workplane:
    seat_x0, seat_y0, seat_x1, seat_y1 = seat_bounds
    support_cap = northwest_rounded_rect_prism(
        seat_x0,
        seat_y0,
        seat_x1,
        seat_y1,
        params["key_cutout_r"],
        bottom_z,
        top_z,
    )
    filler_cap = rect_prism(seat_x0, 0.0, seat_x1, seat_y0 + 0.25, bottom_z, top_z)
    return support_cap.union(filler_cap).clean()


def neokey_raised_cap(
    params: dict,
    seat_bounds: tuple[float, float, float, float],
    bottom_z: float,
    top_z: float,
) -> cq.Workplane:
    seat_x0, _, seat_x1, seat_y1 = seat_bounds
    return rounded_rect_prism(
        seat_x0,
        0.0,
        seat_x1,
        seat_y1,
        params["key_cutout_r"],
        bottom_z,
        top_z,
    )
