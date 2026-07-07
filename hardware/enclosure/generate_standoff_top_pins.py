from __future__ import annotations

from pathlib import Path

import cadquery as cq


ROOT = Path(__file__).resolve().parent
ARTIFACT_ROOT = ROOT.parent.parent / "release-artifacts" / "enclosure"
THIN_BASE_STL_OUT = ARTIFACT_ROOT / "standoff_top_pin_thin_base.stl"
THIN_BASE_STEP_OUT = ARTIFACT_ROOT / "standoff_top_pin_thin_base.step"

BASE_D = 5.0
THIN_BASE_H = 0.5
PIN_BOTTOM_D = 2.6
PIN_TOP_D = 2.48
PIN_H = 6.0


def build_top_pin(base_h: float = THIN_BASE_H) -> cq.Workplane:
    base = cq.Workplane("XY").circle(BASE_D / 2.0).extrude(base_h)
    pin = (
        cq.Workplane("XY", origin=(0, 0, base_h))
        .circle(PIN_BOTTOM_D / 2.0)
        .workplane(offset=PIN_H)
        .circle(PIN_TOP_D / 2.0)
        .loft()
    )
    return base.union(pin).clean()


def main() -> None:
    top_pin = build_top_pin()
    ARTIFACT_ROOT.mkdir(parents=True, exist_ok=True)
    cq.exporters.export(top_pin, str(THIN_BASE_STEP_OUT))
    cq.exporters.export(top_pin, str(THIN_BASE_STL_OUT), tolerance=0.04, angularTolerance=0.08)
    print(f"wrote {THIN_BASE_STEP_OUT}")
    print(f"wrote {THIN_BASE_STL_OUT}")


if __name__ == "__main__":
    main()
