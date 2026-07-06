from __future__ import annotations

import cadquery as cq


PILLAR_BOTTOM_Z = 0.0
INSERT_HOLE_DEPTH = 6.0


def add_faceplate_insert_pillars(model: cq.Workplane, params: dict) -> cq.Workplane:
    hole_r = params["insert_hole_d"] / 2.0
    for spec in params["faceplate_insert_pillars_v22"]:
        x, y = spec["pos"]
        target_z = spec["target_z"]
        boss_r = spec["boss_od"] / 2.0
        pillar = (
            cq.Workplane("XY")
            .circle(boss_r)
            .extrude(target_z - PILLAR_BOTTOM_Z)
            .translate((x, y, PILLAR_BOTTOM_Z))
        )
        insert_hole = (
            cq.Workplane("XY")
            .circle(hole_r)
            .extrude(INSERT_HOLE_DEPTH + 0.1)
            .translate((x, y, PILLAR_BOTTOM_Z - 0.05))
        )
        model = model.union(pillar).cut(insert_hole)
    return model.clean()
