from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parent
ARTIFACT_ROOT = ROOT.parent.parent / "release-artifacts" / "enclosure"
PARAMS = ROOT / "enclosure_params.json"
PDF_OUT = ARTIFACT_ROOT / "bottom_layout_a4_1to1.pdf"

MM_TO_PT = 72.0 / 25.4
A4_LANDSCAPE_MM = (297.0, 210.0)
COMPONENT_PILLAR_D = 5.0
COMPONENT_PILLAR_BASE_D = 7.0
COMPONENT_PILLAR_HOLE_D = 2.65
NEOTRELLIS_PILLAR_BASE_D = 7.0
NEOTRELLIS_PILLAR_HOLE_D = 2.70
FACEPLATE_INSERT_D = 7.2


def pt(mm: float) -> float:
    return mm * MM_TO_PT


def esc(text: str) -> str:
    return text.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")


class PdfCanvas:
    def __init__(self, width_mm: float, height_mm: float) -> None:
        self.width_pt = pt(width_mm)
        self.height_pt = pt(height_mm)
        self.commands: list[str] = []

    def line_width(self, width_mm: float) -> None:
        self.commands.append(f"{pt(width_mm):.3f} w")

    def stroke_rgb(self, r: float, g: float, b: float) -> None:
        self.commands.append(f"{r:.3f} {g:.3f} {b:.3f} RG")

    def fill_rgb(self, r: float, g: float, b: float) -> None:
        self.commands.append(f"{r:.3f} {g:.3f} {b:.3f} rg")

    def line(self, x0: float, y0: float, x1: float, y1: float) -> None:
        self.commands.append(f"{pt(x0):.3f} {pt(y0):.3f} m {pt(x1):.3f} {pt(y1):.3f} l S")

    def arrow(self, x0: float, y0: float, x1: float, y1: float, label: str) -> None:
        self.line(x0, y0, x1, y1)
        dx = x1 - x0
        dy = y1 - y0
        length = (dx * dx + dy * dy) ** 0.5
        if length == 0.0:
            return
        ux = dx / length
        uy = dy / length
        px = -uy
        py = ux
        head_len = 3.0
        head_w = 1.8
        self.line(x1, y1, x1 - ux * head_len + px * head_w, y1 - uy * head_len + py * head_w)
        self.line(x1, y1, x1 - ux * head_len - px * head_w, y1 - uy * head_len - py * head_w)
        self.text(x1 + px * 2.0, y1 + py * 2.0, label, 7.0)

    def circle(self, x: float, y: float, radius: float, fill: bool = False) -> None:
        k = 0.5522847498
        cx = pt(x)
        cy = pt(y)
        r = pt(radius)
        c = r * k
        op = "B" if fill else "S"
        self.commands.append(
            " ".join(
                [
                    f"{cx + r:.3f} {cy:.3f} m",
                    f"{cx + r:.3f} {cy + c:.3f} {cx + c:.3f} {cy + r:.3f} {cx:.3f} {cy + r:.3f} c",
                    f"{cx - c:.3f} {cy + r:.3f} {cx - r:.3f} {cy + c:.3f} {cx - r:.3f} {cy:.3f} c",
                    f"{cx - r:.3f} {cy - c:.3f} {cx - c:.3f} {cy - r:.3f} {cx:.3f} {cy - r:.3f} c",
                    f"{cx + c:.3f} {cy - r:.3f} {cx + r:.3f} {cy - c:.3f} {cx + r:.3f} {cy:.3f} c",
                    op,
                ]
            )
        )

    def rounded_rect(self, x: float, y: float, w: float, h: float, r: float) -> None:
        k = 0.5522847498
        x0 = pt(x)
        y0 = pt(y)
        x1 = pt(x + w)
        y1 = pt(y + h)
        rp = pt(r)
        c = rp * k
        self.commands.append(
            " ".join(
                [
                    f"{x0 + rp:.3f} {y0:.3f} m",
                    f"{x1 - rp:.3f} {y0:.3f} l",
                    f"{x1 - rp + c:.3f} {y0:.3f} {x1:.3f} {y0 + rp - c:.3f} {x1:.3f} {y0 + rp:.3f} c",
                    f"{x1:.3f} {y1 - rp:.3f} l",
                    f"{x1:.3f} {y1 - rp + c:.3f} {x1 - rp + c:.3f} {y1:.3f} {x1 - rp:.3f} {y1:.3f} c",
                    f"{x0 + rp:.3f} {y1:.3f} l",
                    f"{x0 + rp - c:.3f} {y1:.3f} {x0:.3f} {y1 - rp + c:.3f} {x0:.3f} {y1 - rp:.3f} c",
                    f"{x0:.3f} {y0 + rp:.3f} l",
                    f"{x0:.3f} {y0 + rp - c:.3f} {x0 + rp - c:.3f} {y0:.3f} {x0 + rp:.3f} {y0:.3f} c S",
                ]
            )
        )

    def text(self, x: float, y: float, text: str, size: float = 6.0) -> None:
        self.commands.append(f"BT /F1 {size:.1f} Tf {pt(x):.3f} {pt(y):.3f} Td ({esc(text)}) Tj ET")

    def save(self, path: Path) -> None:
        stream = "\n".join(self.commands).encode("ascii")
        objects = [
            b"<< /Type /Catalog /Pages 2 0 R >>",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {self.width_pt:.3f} {self.height_pt:.3f}] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".encode(
                "ascii"
            ),
            b"<< /Length " + str(len(stream)).encode("ascii") + b" >>\nstream\n" + stream + b"\nendstream",
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        ]
        data = bytearray(b"%PDF-1.4\n")
        offsets = [0]
        for index, obj in enumerate(objects, start=1):
            offsets.append(len(data))
            data.extend(f"{index} 0 obj\n".encode("ascii"))
            data.extend(obj)
            data.extend(b"\nendobj\n")
        xref = len(data)
        data.extend(f"xref\n0 {len(objects) + 1}\n0000000000 65535 f \n".encode("ascii"))
        for offset in offsets[1:]:
            data.extend(f"{offset:010d} 00000 n \n".encode("ascii"))
        data.extend(f"trailer << /Size {len(objects) + 1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode("ascii"))
        path.write_bytes(data)


def draw(params: dict) -> PdfCanvas:
    page_w, page_h = A4_LANDSCAPE_MM
    case_w, case_h = params["case_size_v21"]
    origin_x = (page_w - case_w) / 2.0
    origin_y = (page_h - case_h) / 2.0
    c = PdfCanvas(page_w, page_h)
    c.stroke_rgb(0.0, 0.0, 0.0)
    c.line_width(0.25)
    c.rounded_rect(origin_x, origin_y, case_w, case_h, params["corner_r"])
    c.text(10, page_h - 10, "Cell Symphony bottom layout, 1:1 scale on A4 landscape. Print at 100%, no fit-to-page.", 8)
    c.text(10, page_h - 18, "Printed directions: north is up on this page, east is right.", 7)
    c.text(10, page_h - 26, "Check coords: OLED W x=52.65, OLED E x=89.75, OLED S y=90.95, POWER y=65.85/50.61, NEOKEY N y=64.31", 6)
    c.text(origin_x, origin_y - 8, f"Case outline {case_w:.1f} x {case_h:.1f} mm", 6)
    c.line_width(0.35)
    c.arrow(origin_x + case_w + 6.0, origin_y + 20.0, origin_x + case_w + 6.0, origin_y + 38.0, "N")
    c.arrow(origin_x + case_w + 6.0, origin_y + 20.0, origin_x + case_w + 24.0, origin_y + 20.0, "E")
    c.text(origin_x + case_w + 2.0, origin_y + 13.0, "S", 7)
    c.text(origin_x + case_w - 2.0, origin_y + 18.0, "W", 7)

    c.stroke_rgb(0.65, 0.65, 0.65)
    c.line_width(0.12)
    for x in range(0, int(case_w) + 1, 10):
        c.line(origin_x + x, origin_y, origin_x + x, origin_y + case_h)
    for y in range(0, int(case_h) + 1, 10):
        c.line(origin_x, origin_y + y, origin_x + case_w, origin_y + y)

    for spec in params["faceplate_insert_pillars_v22"]:
        x, y = spec["pos"]
        c.stroke_rgb(0.45, 0.45, 0.45)
        c.line_width(0.2)
        c.circle(origin_x + x, origin_y + y, FACEPLATE_INSERT_D / 2.0)
        c.text(origin_x + x + 2.5, origin_y + y + 2.5, spec["name"], 4.5)

    for spec in params["bottom_component_support_pillars_v22"]:
        x, y = spec["pos"]
        component = spec["component"]
        base_d = NEOTRELLIS_PILLAR_BASE_D if component == "neotrellis" else COMPONENT_PILLAR_BASE_D
        hole_d = NEOTRELLIS_PILLAR_HOLE_D if component == "neotrellis" else COMPONENT_PILLAR_HOLE_D
        if component == "neotrellis":
            c.stroke_rgb(0.0, 0.25, 0.85)
        elif component in {"neokey", "oled", "power"}:
            c.stroke_rgb(0.8, 0.15, 0.0)
        else:
            c.stroke_rgb(0.0, 0.45, 0.0)
        c.line_width(0.28)
        c.circle(origin_x + x, origin_y + y, base_d / 2.0)
        c.line_width(0.16)
        c.circle(origin_x + x, origin_y + y, hole_d / 2.0)
        c.line_width(0.1)
        c.line(origin_x + x - 2.0, origin_y + y, origin_x + x + 2.0, origin_y + y)
        c.line(origin_x + x, origin_y + y - 2.0, origin_x + x, origin_y + y + 2.0)
        c.text(origin_x + x + 2.5, origin_y + y + 2.5, spec["name"], 4.2)
    return c


def main() -> None:
    params = json.loads(PARAMS.read_text())
    ARTIFACT_ROOT.mkdir(parents=True, exist_ok=True)
    draw(params).save(PDF_OUT)
    print(PDF_OUT)


if __name__ == "__main__":
    main()
