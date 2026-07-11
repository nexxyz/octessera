from __future__ import annotations

from pathlib import Path

import cadquery as cq


ROOT = Path(__file__).resolve().parent
ARTIFACT_ROOT = ROOT.parent.parent / "release-artifacts" / "enclosure"
STEP_ROOT = ARTIFACT_ROOT / "step"
STL_ROOT = ARTIFACT_ROOT / "stl"

STANDOFF_D = 5.0
PIN_D = 2.65
PIN_H = 5.0
SOCKET_D = 2.85
SOCKET_DEPTH = 6.0


def build_standoff_pillar(standoff_h: float) -> cq.Workplane:
    standoff = cq.Workplane("XY").circle(STANDOFF_D / 2.0).extrude(standoff_h)
    pin = cq.Workplane("XY", origin=(0.0, 0.0, standoff_h)).circle(PIN_D / 2.0).extrude(PIN_H)
    socket = cq.Workplane("XY", origin=(0.0, 0.0, -0.1)).circle(SOCKET_D / 2.0).extrude(SOCKET_DEPTH + 0.1)
    return standoff.union(pin).cut(socket).clean()


def export_pillar(name: str, model: cq.Workplane) -> None:
    STEP_ROOT.mkdir(parents=True, exist_ok=True)
    STL_ROOT.mkdir(parents=True, exist_ok=True)
    for old_name in ["standoff_pillar_9mm"]:
        for suffix in ["step", "stl"]:
            old_path = (STEP_ROOT if suffix == "step" else STL_ROOT) / f"{old_name}.{suffix}"
            if old_path.exists():
                old_path.unlink()
    step_path = STEP_ROOT / f"{name}.step"
    stl_path = STL_ROOT / f"{name}.stl"
    cq.exporters.export(model, str(step_path))
    cq.exporters.export(model, str(stl_path), tolerance=0.04, angularTolerance=0.08)
    print(f"wrote {step_path}")
    print(f"wrote {stl_path}")


def is_valid_solid(model: cq.Workplane) -> bool:
    shape = model.findSolid()
    return bool(getattr(shape, "isValid")())


def main() -> None:
    pillars = {
        "standoff_pillar_9_5mm": build_standoff_pillar(9.5),
        "standoff_pillar_10mm": build_standoff_pillar(10.0),
    }
    for name, model in pillars.items():
        if not is_valid_solid(model):
            raise SystemExit(f"{name} invalid")
        export_pillar(name, model)


if __name__ == "__main__":
    main()
