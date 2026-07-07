from __future__ import annotations

import cadquery as cq

from wave_guidance import south_edge_samples


SKIRT_BOTTOM_Z = -10.0


def plain_rect_prism(x0: float, y0: float, x1: float, y1: float, z0: float, z1: float) -> cq.Workplane:
    return (
        cq.Workplane("XY")
        .rect(x1 - x0, y1 - y0)
        .extrude(z1 - z0)
        .translate(((x0 + x1) / 2.0, (y0 + y1) / 2.0, z0))
    )


def rounded_wall_shell(params: dict, rounded_plate, z0: float, z1: float) -> cq.Workplane:
    width, depth = params["case_size_v21"]
    wall = params["wall"]
    radius = params["corner_r"]
    outer = rounded_plate(width, depth, radius, z0, z1 - z0)
    inner = rounded_plate(width - 2.0 * wall, depth - 2.0 * wall, max(radius - wall, 0.1), z0 - 0.1, z1 - z0 + 0.2)
    return outer.cut(inner.translate((wall, wall, 0.0))).clean()


def quarter_circle_ease(t: float) -> float:
    t = max(0.0, min(1.0, t))
    return 1.0 - (1.0 - t * t) ** 0.5


def x_profile_wall(
    x_values: list[float],
    y0: float,
    y1: float,
    base_z: float,
    top_z_at_x,
) -> cq.Workplane:
    wires = []
    for x in x_values:
        z = max(base_z + 0.05, top_z_at_x(x))
        wires.append(
            cq.Wire.makePolygon(
                [
                    cq.Vector(x, y0, base_z),
                    cq.Vector(x, y1, base_z),
                    cq.Vector(x, y1, z),
                    cq.Vector(x, y0, z),
                    cq.Vector(x, y0, base_z),
                ]
            )
        )
    return cq.Workplane("XY").add(cq.Solid.makeLoft(wires, ruled=True))


def quarter_circle_x_wall(
    x0: float,
    x1: float,
    y0: float,
    y1: float,
    base_z: float,
    low_top_z: float,
    high_top_z: float,
    samples: int = 64,
) -> cq.Workplane:
    def top_z_at_x(x: float) -> float:
        t = (x - x0) / (x1 - x0)
        return low_top_z + (high_top_z - low_top_z) * quarter_circle_ease(t)

    return x_profile_wall(sample_range(x0, x1, samples), y0, y1, base_z, top_z_at_x)


def sample_range(x0: float, x1: float, steps: int) -> list[float]:
    return [x0 + (x1 - x0) * index / steps for index in range(steps + 1)]


def unique_sorted(values: list[float]) -> list[float]:
    return sorted({round(value, 5) for value in values})


def interpolate(x: float, x0: float, x1: float, z0: float, z1: float) -> float:
    if x1 == x0:
        return z1
    return z0 + (z1 - z0) * ((x - x0) / (x1 - x0))


def perimeter_wall_skirts(
    params: dict,
    rounded_plate,
    extended_slope_right_x: float,
    lower_to_tier2_ramp_start_x: float,
    lower_to_tier2_ramp_end_x: float,
    low_z: float,
    lower_wave_high_z: float,
    high_z: float,
) -> cq.Workplane:
    width, depth = params["case_size_v21"]
    wall = params["wall"]
    footprint = rounded_plate(width, depth, params["corner_r"], SKIRT_BOTTOM_Z, high_z - SKIRT_BOTTOM_Z)
    low_shell = rounded_wall_shell(params, rounded_plate, SKIRT_BOTTOM_Z, low_z)
    high_edge, low_edge = south_edge_samples()
    south_low_x = low_edge[0][0]
    south_high_x = high_edge[0][0]
    north_low_x = low_edge[-1][0]
    north_high_x = high_edge[-1][0]
    high_extensions = (
        plain_rect_prism(width - wall, 0.0, width, depth, low_z, high_z)
        .union(plain_rect_prism(extended_slope_right_x, depth - wall, width, depth, low_z, high_z))
        .union(plain_rect_prism(extended_slope_right_x, 0.0, width, wall, low_z, high_z))
        .intersect(footprint)
        .clean()
    )
    south_profile_x = unique_sorted([
        *sample_range(south_low_x, south_high_x, 8),
        south_high_x,
        lower_to_tier2_ramp_start_x,
    ])

    def south_top_z(x: float) -> float:
        raised_corner_z = low_z + 2.0
        if x <= south_high_x:
            return interpolate(x, south_low_x, south_high_x, raised_corner_z, lower_wave_high_z)
        return lower_wave_high_z

    south_ramp = x_profile_wall(
        south_profile_x,
        0.0,
        wall,
        low_z,
        south_top_z,
    ).intersect(footprint).clean()
    overlap = 0.08
    south_transition = quarter_circle_x_wall(
        lower_to_tier2_ramp_start_x - overlap,
        lower_to_tier2_ramp_end_x + overlap,
        0.0,
        wall,
        low_z,
        lower_wave_high_z,
        high_z,
    ).intersect(footprint).clean()

    def north_top_z(x: float) -> float:
        if x <= north_low_x:
            return low_z
        if x <= north_high_x:
            return interpolate(x, north_low_x, north_high_x, low_z, high_z)
        return high_z

    north_ramp = x_profile_wall(
        sample_range(north_low_x, extended_slope_right_x, 24),
        depth - wall,
        depth,
        low_z,
        north_top_z,
    )
    return low_shell.union(south_ramp).union(south_transition).union(north_ramp).union(high_extensions).clean()
