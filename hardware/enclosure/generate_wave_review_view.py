from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path

try:
    import generate_two_level_enclosure_cadquery as model
except ModuleNotFoundError:
    from hardware.enclosure import generate_two_level_enclosure_cadquery as model


ROOT = Path(__file__).resolve().parent
ARTIFACT_ROOT = ROOT.parent.parent / "release-artifacts" / "enclosure" / "review"
SVG_OUT = ARTIFACT_ROOT / "current_wave_top_view.svg"
PNG_OUT = ARTIFACT_ROOT / "current_wave_top_view.png"

SCALE = 6.0
MARGIN = 30.0
PI_BOARD_X0 = 8.5
PI_BOARD_Y0 = 6.5
PI_BOARD_W = 65.0
PI_BOARD_H = 30.0
PI_HOTSPOT_X = PI_BOARD_X0 + 35.0
PI_HOTSPOT_Y = PI_BOARD_Y0 + 16.5
PI_HOTSPOT_R = 7.0


def svg_x(x: float) -> float:
    return MARGIN + x * SCALE


def svg_y(y: float, case_h: float) -> float:
    return MARGIN + (case_h - y) * SCALE


def rect(case_h: float, x0: float, y0: float, x1: float, y1: float, stroke: str, fill: str = "none", width: float = 1.0, dash: str = "") -> str:
    return (
        f'<rect x="{svg_x(x0):.2f}" y="{svg_y(y1, case_h):.2f}" '
        f'width="{(x1 - x0) * SCALE:.2f}" height="{(y1 - y0) * SCALE:.2f}" '
        f'fill="{fill}" stroke="{stroke}" stroke-width="{width}" {dash}/>'
    )


def circle(case_h: float, x: float, y: float, r: float, stroke: str, fill: str = "none", width: float = 1.0) -> str:
    return f'<circle cx="{svg_x(x):.2f}" cy="{svg_y(y, case_h):.2f}" r="{r * SCALE:.2f}" fill="{fill}" stroke="{stroke}" stroke-width="{width}"/>'


def text(case_h: float, x: float, y: float, label: str, size: int = 14) -> str:
    return f'<text x="{svg_x(x):.2f}" y="{svg_y(y, case_h):.2f}" font-size="{size}" fill="#111">{label}</text>'


def line(case_h: float, x0: float, y0: float, x1: float, y1: float, stroke: str, width: float = 1.0, dash: str = "") -> str:
    return f'<line x1="{svg_x(x0):.2f}" y1="{svg_y(y0, case_h):.2f}" x2="{svg_x(x1):.2f}" y2="{svg_y(y1, case_h):.2f}" stroke="{stroke}" stroke-width="{width}" {dash}/>'


def main() -> None:
    params = json.loads(model.PARAMS.read_text())
    case_w, case_h = params["case_size_v21"]
    image_w = case_w * SCALE + 2.0 * MARGIN
    image_h = case_h * SCALE + 2.0 * MARGIN

    high, low = model.south_edge_samples()
    key_centers = model.neokey_key_centers(params)
    neokey_seat_x0, _, _, neokey_seat_top_y = model.neokey_seat_bounds(params, key_centers)
    lower_wave_top_y = model.first_y_at_x(low, neokey_seat_x0)

    def wave_polygon(y0: float, y1: float) -> str:
        pairs = model.trimmed_curve_pairs(low, high, y0, y1)
        high_points = [high_point for _, high_point in pairs]
        low_points = [low_point for low_point, _ in pairs]
        points = high_points + list(reversed(low_points))
        return " ".join(f"{svg_x(x):.2f},{svg_y(y, case_h):.2f}" for x, y in points)

    def wave_flat_polygon(y0: float, y1: float, right_x: float = model.EXTENDED_SLOPE_RIGHT_X) -> str:
        pairs = model.trimmed_curve_pairs(low, high, y0, y1)
        high_points = [high_point for _, high_point in pairs]
        points = [
            (right_x, y0),
            (right_x, y1),
            *reversed(high_points),
        ]
        return " ".join(f"{svg_x(x):.2f},{svg_y(y, case_h):.2f}" for x, y in points)

    def polygon(points: list[tuple[float, float]]) -> str:
        return " ".join(f"{svg_x(x):.2f},{svg_y(y, case_h):.2f}" for x, y in points)

    upper_wave_poly = wave_polygon(neokey_seat_top_y, case_h)
    lower_wave_poly = wave_polygon(0.0, lower_wave_top_y)
    lower_wave_flat_poly = wave_flat_polygon(0.0, lower_wave_top_y, model.LOWER_TO_TIER2_RAMP_START_X)
    ramp_poly = polygon(
        [
            (model.LOWER_TO_TIER2_RAMP_START_X, 0.0),
            (model.LOWER_TO_TIER2_RAMP_END_X, 0.0),
            (model.LOWER_TO_TIER2_RAMP_END_X, lower_wave_top_y),
            (model.LOWER_TO_TIER2_RAMP_START_X, lower_wave_top_y),
        ]
    )
    high_region = [(case_w, 0.0), (case_w, case_h), (model.EXTENDED_SLOPE_RIGHT_X, case_h), *reversed(high), (case_w, 0.0)]
    high_poly = " ".join(f"{svg_x(x):.2f},{svg_y(y, case_h):.2f}" for x, y in high_region)

    lines = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{image_w:.0f}" height="{image_h:.0f}" viewBox="0 0 {image_w:.0f} {image_h:.0f}">',
        '<rect width="100%" height="100%" fill="white"/>',
        rect(case_h, 0, 0, case_w, case_h, "#777", width=2.0),
        f'<polygon points="{high_poly}" fill="rgba(70,120,255,0.18)" stroke="#4678ff" stroke-width="1"/>',
        f'<polygon points="{ramp_poly}" fill="rgba(70,120,255,0.10)" stroke="#4678ff" stroke-width="1"/>',
        f'<polygon points="{lower_wave_flat_poly}" fill="rgba(70,120,255,0.18)" stroke="#4678ff" stroke-width="1"/>',
        f'<polygon points="{upper_wave_poly}" fill="rgba(0,0,0,0.65)" stroke="black" stroke-width="2"/>',
        f'<polygon points="{lower_wave_poly}" fill="rgba(0,0,0,0.65)" stroke="black" stroke-width="2"/>',
        '<polyline points="' + " ".join(f"{svg_x(x):.2f},{svg_y(y, case_h):.2f}" for x, y in high) + '" fill="none" stroke="#164cff" stroke-width="2"/>',
        '<polyline points="' + " ".join(f"{svg_x(x):.2f},{svg_y(y, case_h):.2f}" for x, y in low) + '" fill="none" stroke="#111" stroke-width="2"/>',
        rect(case_h, 8.5, 6.5, 8.5 + 107.25, 6.5 + 127.0, "#1f77b4", width=2.0, dash='stroke-dasharray="6 3"'),
        rect(case_h, PI_BOARD_X0, PI_BOARD_Y0, PI_BOARD_X0 + PI_BOARD_W, PI_BOARD_Y0 + PI_BOARD_H, "#0aa", "rgba(0,180,180,0.06)", 2.0, dash='stroke-dasharray="4 3"'),
        circle(case_h, PI_HOTSPOT_X, PI_HOTSPOT_Y, PI_HOTSPOT_R, "#0aa", "rgba(0,180,180,0.12)", 2.0),
        text(case_h, PI_HOTSPOT_X + 4.0, PI_HOTSPOT_Y, "Approx Pi SoC/hotspot", 12),
    ]

    for start, end in model.load_guidance_slots():
        lines.append(
            f'<line x1="{svg_x(start[0]):.2f}" y1="{svg_y(start[1], case_h):.2f}" '
            f'x2="{svg_x(end[0]):.2f}" y2="{svg_y(end[1], case_h):.2f}" '
            'stroke="#333" stroke-width="12" stroke-linecap="round"/>'
        )

    for port in params["ports_v21"]:
        if port["label"] in {"Pi mini-HDMI", "Pi USB data"}:
            lines.append(rect(case_h, port["a"], -4.0, port["b"], 0.0, "#0aa", "rgba(0,180,180,0.18)", 1.0))
            lines.append(text(case_h, port["a"], 3.0, port["label"], 11))

    oled = params["features_local"]["oled_board_bbox"]
    oled_x0, oled_y0 = model.local_to_case(params, [oled[0], oled[3]])
    oled_x1, oled_y1 = model.local_to_case(params, [oled[2], oled[1]])
    lines.append(rect(case_h, oled_x0, oled_y0, oled_x1, oled_y1, "red", "rgba(255,0,0,0.05)", 2.0))

    screen_x, screen_y = model.local_to_case(params, params["features_local"]["oled_screen_center"])
    screen_w, screen_h = params["screen_cutout"]
    lines.append(rect(case_h, screen_x - screen_w / 2.0, screen_y - screen_h / 2.0, screen_x + screen_w / 2.0, screen_y + screen_h / 2.0, "red", width=2.0))

    encoder_labels = {"SW1": "SW1", "SW2": "SW2", "SW3": "SW3", "SW4": "AUX3"}
    for name, point in params["features_local"]["encoders"].items():
        x, y = model.local_to_case(params, point)
        flat_r = params["encoder_crater_flat_d"][name] / 2.0
        outer_r = flat_r + params["encoder_crater_slope_w"]
        hole_r = params["encoder_hole_d"] / 2.0
        lines.append(circle(case_h, x, y, outer_r, "#777", width=1.5))
        lines.append(circle(case_h, x, y, flat_r, "#111", width=2.0))
        lines.append(circle(case_h, x, y, hole_r, "#111", width=1.0))
        lines.append(text(case_h, x + 5.0, y - 2.0, encoder_labels[name]))

    pitch = params["neotrellis_pitch"]
    cutout = params["neotrellis_button_cutout"]
    for row in range(8):
        for col in range(8):
            x = 124.75 + col * pitch
            y = 17.5 + row * pitch
            lines.append(rect(case_h, x - cutout / 2.0, y - cutout / 2.0, x + cutout / 2.0, y + cutout / 2.0, "orange", width=0.8))

    lines.extend(
        [
            '<text x="40" y="22" font-size="18" fill="#111">Current generated top view: black = shoulder slope, blue = high flat tier</text>',
            '<text x="40" y="44" font-size="13" fill="#555">Blue dashed = main PCB keepout; red = OLED; orange = NeoTrellis; AUX3 label marks SW4 crater.</text>',
            '<text x="40" y="62" font-size="13" fill="#555">Cyan = approximate Pi Zero 2 W board and SoC/hotspot target. Gray encoder rings include sloped crater radius.</text>',
            "</svg>",
        ]
    )
    ARTIFACT_ROOT.mkdir(parents=True, exist_ok=True)
    SVG_OUT.write_text("\n".join(lines), encoding="utf-8")
    print(SVG_OUT)
    if shutil.which("magick"):
        subprocess.run(["magick", str(SVG_OUT), str(PNG_OUT)], check=True)
        print(PNG_OUT)


if __name__ == "__main__":
    main()
