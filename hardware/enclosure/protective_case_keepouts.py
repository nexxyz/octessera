from __future__ import annotations

from dataclasses import dataclass

try:
    from .protective_case_geometry import (
        FACE_DOWN_TRANSPORT_TRANSLATION,
        face_down_transport_feature_xy,
        face_down_transport_xy,
        load_source_parameters,
    )
except ImportError:
    from protective_case_geometry import (
        FACE_DOWN_TRANSPORT_TRANSLATION,
        face_down_transport_feature_xy,
        face_down_transport_xy,
        load_source_parameters,
    )


OLED_SCREEN_CUTOUT_X_SHIFT = -0.5
OLED_SCREEN_CUTOUT_Y_SHIFT = -0.3
NEOKEY_PANEL_X_OFFSET = -0.25
NEOKEY_PANEL_Y_OFFSET = -1.0
NEOTRELLIS_CASE_ORIGIN = (124.75, 17.5)


@dataclass(frozen=True)
class ContactKeepout:
    name: str
    bounds: tuple[float, float, float, float]


def _bounds_for_points(
    points: tuple[tuple[float, float], ...], half_width: float, half_depth: float
) -> tuple[float, float, float, float]:
    return (
        min(point[0] for point in points) - half_width,
        min(point[1] for point in points) - half_depth,
        max(point[0] for point in points) + half_width,
        max(point[1] for point in points) + half_depth,
    )


def _expand_bounds(
    bounds: tuple[float, float, float, float], expansion_x: float, expansion_y: float
) -> tuple[float, float, float, float]:
    return (
        bounds[0] - expansion_x,
        bounds[1] - expansion_y,
        bounds[2] + expansion_x,
        bounds[3] + expansion_y,
    )


def control_contact_keepouts(params: dict) -> tuple[ContactKeepout, ...]:
    source = load_source_parameters()
    source_features = source["features_local"]
    expansion_x, expansion_y = params["device"]["control_keepout_expansion"]
    keepouts = []
    screen_x, screen_y = face_down_transport_feature_xy(
        source,
        source_features["oled_screen_center"],
        OLED_SCREEN_CUTOUT_X_SHIFT,
        OLED_SCREEN_CUTOUT_Y_SHIFT,
    )
    screen_width, screen_depth = source["screen_cutout"]
    keepouts.append(
        ContactKeepout(
            "OLED opening",
            _expand_bounds(
                (
                    screen_x - screen_width / 2.0,
                    screen_y - screen_depth / 2.0,
                    screen_x + screen_width / 2.0,
                    screen_y + screen_depth / 2.0,
                ),
                expansion_x,
                expansion_y,
            ),
        )
    )
    for name, point in source_features["encoders"].items():
        x, y = face_down_transport_feature_xy(source, point)
        radius = source["encoder_crater_flat_d"][name] / 2.0 + source["encoder_crater_slope_w"]
        keepouts.append(
            ContactKeepout(
                f"encoder {name} cap",
                _expand_bounds((x - radius, y - radius, x + radius, y + radius), expansion_x, expansion_y),
            )
        )
    key_centers = tuple(
        face_down_transport_feature_xy(
            source,
            point,
            NEOKEY_PANEL_X_OFFSET,
            NEOKEY_PANEL_Y_OFFSET,
        )
        for point in source_features["neokey_key_centers"]
    )
    key_width, key_depth = source["key_cutout"]
    keepouts.append(
        ContactKeepout(
            "NeoKey cutout field",
            _expand_bounds(
                _bounds_for_points(key_centers, key_width / 2.0, key_depth / 2.0),
                expansion_x,
                expansion_y,
            ),
        )
    )
    pitch = source["neotrellis_pitch"]
    opening = source["neotrellis_button_cutout"]
    trellis_centers = tuple(
        face_down_transport_xy(
            (
                FACE_DOWN_TRANSPORT_TRANSLATION[0] + NEOTRELLIS_CASE_ORIGIN[0] + column * pitch,
                FACE_DOWN_TRANSPORT_TRANSLATION[1] - (NEOTRELLIS_CASE_ORIGIN[1] + row * pitch),
            )
        )
        for row in range(8)
        for column in range(8)
    )
    keepouts.append(
        ContactKeepout(
            "NeoTrellis opening field",
            _expand_bounds(
                _bounds_for_points(trellis_centers, opening / 2.0, opening / 2.0),
                expansion_x,
                expansion_y,
            ),
        )
    )
    return tuple(keepouts)
