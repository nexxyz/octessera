from __future__ import annotations

from pathlib import Path

import cadquery as cq


ROOT = Path(__file__).resolve().parent
ARTIFACT_ROOT = ROOT.parent.parent / "release-artifacts" / "enclosure"

PLUG_STL_OUT = ARTIFACT_ROOT / "stl" / "friction_insert_expanding_plug_flat_sides.stl"
PLUG_STEP_OUT = ARTIFACT_ROOT / "step" / "friction_insert_expanding_plug_flat_sides.step"
PIN_STL_OUT = ARTIFACT_ROOT / "stl" / "friction_insert_spreading_pin_headed.stl"
PIN_STEP_OUT = ARTIFACT_ROOT / "step" / "friction_insert_spreading_pin_headed.step"

INSERT_HOLE_D = 4.6
BOTTOM_SCREW_CLEARANCE_D = 3.4

PLUG_OD = 4.78
PLUG_FLAT_WIDTH = 4.28
PLUG_LENGTH = 7.4
PLUG_BORE_BOTTOM_D = 2.70
PLUG_BORE_TOP_D = 3.40
PLUG_SLIT_WIDTH = 0.58
PLUG_SLIT_DEPTH = 4.9
PLUG_SLIT_COUNT = 4
PLUG_TOP_RING_H = 0.5

PIN_DIAMETER_SCALE = 0.85
PIN_LENGTH_SCALE = 0.80

PIN_SHAFT_D = 2.40 * PIN_DIAMETER_SCALE
PIN_LENGTH = 5.2 * PIN_LENGTH_SCALE
PIN_HEAD_D = 5.0
PIN_HEAD_H = 0.75
PIN_TIP_BALL_D = 3.00 * PIN_DIAMETER_SCALE
PIN_TIP_NECK_D = 2.30 * PIN_DIAMETER_SCALE


def export_part(part: cq.Workplane, step_out: Path, stl_out: Path) -> None:
    step_out.parent.mkdir(parents=True, exist_ok=True)
    stl_out.parent.mkdir(parents=True, exist_ok=True)
    cq.exporters.export(part, str(step_out))
    cq.exporters.export(part, str(stl_out), tolerance=0.035, angularTolerance=0.08)
    print(f"wrote {step_out}")
    print(f"wrote {stl_out}")


def flat_sided_cylinder(outer_d: float, flat_width: float, length: float) -> cq.Workplane:
    cylinder = cq.Workplane("XY").circle(outer_d / 2.0).extrude(length)
    top_cut = (
        cq.Workplane("XY")
        .box(outer_d + 1.0, outer_d, length + 0.4)
        .translate((0, flat_width / 2.0 + outer_d / 2.0, length / 2.0))
    )
    bottom_cut = (
        cq.Workplane("XY")
        .box(outer_d + 1.0, outer_d, length + 0.4)
        .translate((0, -(flat_width / 2.0 + outer_d / 2.0), length / 2.0))
    )
    return cylinder.cut(top_cut).cut(bottom_cut).clean()


def tapered_bore(bottom_d: float, top_d: float, length: float) -> cq.Workplane:
    return (
        cq.Workplane("XY")
        .circle(bottom_d / 2.0)
        .workplane(offset=length)
        .circle(top_d / 2.0)
        .loft()
    )


def build_expanding_plug() -> cq.Workplane:
    plug = flat_sided_cylinder(PLUG_OD, PLUG_FLAT_WIDTH, PLUG_LENGTH)
    bore = tapered_bore(PLUG_BORE_BOTTOM_D, PLUG_BORE_TOP_D, PLUG_LENGTH)
    plug = plug.cut(bore)

    slit_top_z = PLUG_LENGTH - PLUG_TOP_RING_H
    for index in range(PLUG_SLIT_COUNT):
        angle = index * 90
        slit = (
            cq.Workplane("XY")
            .box(PLUG_SLIT_WIDTH, PLUG_OD + 0.8, PLUG_SLIT_DEPTH)
            .translate((0, PLUG_OD / 2.0, slit_top_z - PLUG_SLIT_DEPTH / 2.0))
            .rotate((0, 0, 0), (0, 0, 1), angle)
        )
        plug = plug.cut(slit)
    return plug.rotate((0, 0, 0), (1, 0, 0), 90).translate((0, PLUG_LENGTH / 2.0, PLUG_FLAT_WIDTH / 2.0)).clean()


def build_spreading_pin() -> cq.Workplane:
    head = cq.Workplane("XY").circle(PIN_HEAD_D / 2.0).extrude(PIN_HEAD_H)
    shaft = (
        cq.Workplane("XY", origin=(0, 0, PIN_HEAD_H))
        .circle(PIN_SHAFT_D / 2.0)
        .workplane(offset=PIN_LENGTH)
        .circle(PIN_TIP_NECK_D / 2.0)
        .loft()
    )
    ball = cq.Workplane("XY").sphere(PIN_TIP_BALL_D / 2.0).translate((0, 0, PIN_HEAD_H + PIN_LENGTH))
    pin = head.union(shaft).union(ball)
    pin = pin.faces("<Z").edges().chamfer(0.18)
    return pin.clean()


def main() -> None:
    plug = build_expanding_plug()
    pin = build_spreading_pin()
    export_part(plug, PLUG_STEP_OUT, PLUG_STL_OUT)
    export_part(pin, PIN_STEP_OUT, PIN_STL_OUT)
    print("__FRICTION_INSERT_PLUG_PIN_DONE__")


if __name__ == "__main__":
    main()
