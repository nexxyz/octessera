from __future__ import annotations

import json
from pathlib import Path

import generate_two_level_enclosure_cadquery as model


ROOT = Path(__file__).resolve().parent
SVG_OUT = ROOT / "wave_curve_template.svg"

SCALE = 6.0
MARGIN = 30.0


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


def circle(case_h: float, x: float, y: float, radius: float, stroke: str, fill: str = "none", width: float = 1.0) -> str:
    return f'<circle cx="{svg_x(x):.2f}" cy="{svg_y(y, case_h):.2f}" r="{radius * SCALE:.2f}" fill="{fill}" stroke="{stroke}" stroke-width="{width}"/>'


def text(case_h: float, x: float, y: float, label: str, size: int = 12, color: str = "#222") -> str:
    return f'<text x="{svg_x(x):.2f}" y="{svg_y(y, case_h):.2f}" font-size="{size}" fill="{color}">{label}</text>'


def main() -> None:
    params = json.loads(model.PARAMS.read_text())
    case_w, case_h = params["case_size_v21"]
    image_w = case_w * SCALE + 2.0 * MARGIN
    image_h = case_h * SCALE + 2.0 * MARGIN
    lines = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{image_w:.0f}" height="{image_h:.0f}" viewBox="0 0 {image_w:.0f} {image_h:.0f}">',
        '<rect width="100%" height="100%" fill="white"/>',
    ]

    for x in range(0, int(case_w) + 1, 10):
        lines.append(f'<line x1="{svg_x(x):.2f}" y1="{svg_y(0, case_h):.2f}" x2="{svg_x(x):.2f}" y2="{svg_y(case_h, case_h):.2f}" stroke="#eee" stroke-width="1"/>')
    for y in range(0, int(case_h) + 1, 10):
        lines.append(f'<line x1="{svg_x(0):.2f}" y1="{svg_y(y, case_h):.2f}" x2="{svg_x(case_w):.2f}" y2="{svg_y(y, case_h):.2f}" stroke="#eee" stroke-width="1"/>')

    lines.append(rect(case_h, 0, 0, case_w, case_h, "#777", width=2.0))
    lines.append(rect(case_h, 8.5, 6.5, 8.5 + 107.25, 6.5 + 127.0, "#1f77b4", width=2.0, dash='stroke-dasharray="6 3"'))

    oled = params["features_local"]["oled_board_bbox"]
    oled_x0, oled_y0 = model.local_to_case(params, [oled[0], oled[3]])
    oled_x1, oled_y1 = model.local_to_case(params, [oled[2], oled[1]])
    lines.append(rect(case_h, oled_x0, oled_y0, oled_x1, oled_y1, "#e22", "rgba(255,0,0,0.04)", 2.0))
    screen_x, screen_y = model.local_to_case(params, params["features_local"]["oled_screen_center"])
    screen_w, screen_h = params["screen_cutout"]
    lines.append(rect(case_h, screen_x - screen_w / 2.0, screen_y - screen_h / 2.0, screen_x + screen_w / 2.0, screen_y + screen_h / 2.0, "#e22", width=2.0))
    lines.append(text(case_h, oled_x0, oled_y1 + 4.0, "OLED"))

    encoder_labels = {"SW1": "SW1", "SW2": "SW2", "SW3": "SW3", "SW4": "AUX3 / SW4"}
    for name, point in params["features_local"]["encoders"].items():
        x, y = model.local_to_case(params, point)
        flat_r = params["encoder_crater_flat_d"][name] / 2.0
        outer_r = flat_r + params["encoder_crater_slope_w"]
        lines.append(circle(case_h, x, y, outer_r, "#999", width=1.5))
        lines.append(circle(case_h, x, y, flat_r, "#111", width=2.0))
        lines.append(text(case_h, x + 5.0, y - 2.0, encoder_labels[name]))

    neo_bbox = params["features_local"]["neokey_bbox"]
    nk_x0, nk_y0 = model.local_to_case(params, [neo_bbox[0], neo_bbox[3]])
    nk_x1, nk_y1 = model.local_to_case(params, [neo_bbox[2], neo_bbox[1]])
    lines.append(rect(case_h, nk_x0, nk_y0, nk_x1, nk_y1, "#950095", "rgba(160,0,160,0.04)", 2.0, dash='stroke-dasharray="5 3"'))
    for point in params["features_local"]["neokey_key_centers"]:
        x, y = model.local_to_case(params, point)
        y += model.NEOKEY_PANEL_Y_OFFSET
        key_w, key_h = params["key_cutout"]
        lines.append(rect(case_h, x - key_w / 2.0, y - key_h / 2.0, x + key_w / 2.0, y + key_h / 2.0, "#950095", width=1.4))
    lines.append(text(case_h, nk_x0, nk_y1 + 4.0, "NeoKey"))

    pitch = params["neotrellis_pitch"]
    cutout = params["neotrellis_button_cutout"]
    for row in range(8):
        for col in range(8):
            x = 124.75 + col * pitch
            y = 22.5 + row * pitch
            lines.append(rect(case_h, x - cutout / 2.0, y - cutout / 2.0, x + cutout / 2.0, y + cutout / 2.0, "#e6a000", width=0.8))
    lines.append(rect(case_h, 118.0, 9.0, 238.0, 130.0, "#e6a000", "none", 2.0))
    lines.append(text(case_h, 124.0, 134.0, "NeoTrellis"))

    lines.append(text(case_h, 3.0, -5.0, "Draw desired wave area using two splines plus two connecting lines. Coordinates are CAD mm; X right, Y up.", 14))
    lines.append("</svg>")
    SVG_OUT.write_text("\n".join(lines), encoding="utf-8")
    print(SVG_OUT)


if __name__ == "__main__":
    main()
