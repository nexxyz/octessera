from __future__ import annotations


SLOPE_PROFILE_STEPS = 12
SOUTH_SHOULDER_SAMPLES = 36
SOUTH_SHOULDER_PLAN_WIDTH = 8.5
SOUTH_ROOF_LOW_EAVE_WIDTH = 0.0
SOUTH_ROOF_LOW_WALL_BAND = 3.0

PI_BLOCK_WEST_X = 0.0
PI_BLOCK_EAST_X = 105.0
PI_BLOCK_SOUTH_Y = 0.0
PI_BLOCK_NORTH_Y = 38.35
NEOKEY_SLOPE_SOUTH_Y = 88.0
NEOKEY_SLOPE_LOW_X = 95.0
CASE_NORTH_Y = 140.0


def _sample_range(start: float, end: float, samples: int) -> list[float]:
    return [start + (end - start) * index / samples for index in range(samples + 1)]


def south_edge_samples() -> tuple[list[tuple[float, float]], list[tuple[float, float]]]:
    south_y = PI_BLOCK_NORTH_Y
    north_y = CASE_NORTH_Y
    west_x = PI_BLOCK_WEST_X
    east_x = PI_BLOCK_EAST_X

    lower_segment = [
        ((x, south_y), (x, south_y - SOUTH_SHOULDER_PLAN_WIDTH))
        for x in _sample_range(west_x, east_x, SOUTH_SHOULDER_SAMPLES)
    ]
    protected_segment = [
        ((NEOKEY_SLOPE_LOW_X, y), (NEOKEY_SLOPE_LOW_X + SOUTH_SHOULDER_PLAN_WIDTH, y))
        for y in _sample_range(NEOKEY_SLOPE_SOUTH_Y, north_y, 12)
    ]
    pairs = lower_segment + protected_segment
    low = [low_point for low_point, _ in pairs]
    high = [high_point for _, high_point in pairs]
    return high, low


def load_guidance_slots() -> list[tuple[tuple[float, float], tuple[float, float]]]:
    slot_count = 5
    slot_spacing = 10.0
    slot_length = 17.2
    center_x = (PI_BLOCK_WEST_X + PI_BLOCK_EAST_X) / 2.0 - 8.0
    center_y = (PI_BLOCK_SOUTH_Y + PI_BLOCK_NORTH_Y) / 2.0 - 2.5
    first_x = center_x - slot_spacing * (slot_count - 1) / 2.0
    y0 = center_y - slot_length / 2.0
    y1 = center_y + slot_length / 2.0
    return [((first_x + index * slot_spacing, y0), (first_x + index * slot_spacing, y1)) for index in range(slot_count)]


def extend_low_edge_outward(
    low: list[tuple[float, float]], high: list[tuple[float, float]], distance: float
) -> list[tuple[float, float]]:
    outer = []
    for low_point, high_point in zip(low, high):
        low_x, low_y = low_point
        high_x, high_y = high_point
        dx = low_x - high_x
        dy = low_y - high_y
        length = (dx * dx + dy * dy) ** 0.5
        if length == 0.0:
            outer.append(low_point)
        else:
            outer.append((low_x + dx / length * distance, low_y + dy / length * distance))
    return outer


def south_roof_outer_samples() -> list[tuple[float, float]]:
    high, low = south_edge_samples()
    return extend_low_edge_outward(low, high, SOUTH_ROOF_LOW_EAVE_WIDTH)
