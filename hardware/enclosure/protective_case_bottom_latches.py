from __future__ import annotations

from typing import cast

import cadquery as cq

try:
    from .upstream import andy_wings_parametric_box as upstream
    from .protective_case_geometry import hinge_axis, hinge_centers, normalize_source_half, parametric_box_measures
    from .protective_case_mating_interface import deep_receiver_components, opening_prism, shallow_tongue_components, transformed_hinge_profile
except ImportError:
    import upstream.andy_wings_parametric_box as upstream
    from protective_case_geometry import hinge_axis, hinge_centers, normalize_source_half, parametric_box_measures
    from protective_case_mating_interface import deep_receiver_components, opening_prism, shallow_tongue_components, transformed_hinge_profile


TOLERANCE = 0.05
VOLUME_TOLERANCE = 1.0e-7
SEAM_Z = 54.85
ACTUATOR_VOLUME = 522.288830
CATCH_VOLUME = 80.593856
ACTUATOR_ROOT_FACE = 79.945371
CATCH_ROOT_FACE = 107.858996


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def _volume(model: cq.Workplane) -> float:
    return sum(solid.Volume() for solid in cast(list[cq.Shape], model.solids().vals()))


def _intersection_volume(first: cq.Workplane, second: cq.Workplane) -> float:
    return _volume(first.intersect(second))


def _shape_box(model: cq.Workplane) -> cq.BoundBox:
    return cast(cq.Shape, model.val()).BoundingBox()


def _prism(bounds: tuple[float, float, float, float], z_min: float, z_max: float) -> cq.Workplane:
    x_min, x_max, y_min, y_max = bounds
    return cq.Workplane("XY").box(x_max - x_min, y_max - y_min, z_max - z_min, centered=(False, False, False)).translate((x_min, y_min, z_min))


def seam_z(params: dict) -> float:
    return params["parametric_box"]["top_height"]


def wall_y(params: dict) -> float:
    box = params["parametric_box"]
    return params["wall_planes"]["north_y"] + box["thickness"]


def latch_centers(params: dict) -> tuple[float, ...]:
    translate_x = params["normalization"]["translate"][0]
    return tuple(translate_x + position for position in params["parametric_box"]["clip"]["positions_x"])


def _profile_kind(half: str) -> str:
    if half == "shallow":
        return "actuator"
    if half == "deep":
        return "catch"
    raise ValueError(f"unknown latch half {half}")


def _source_clip_profile(params: dict, kind: str) -> cq.Workplane:
    clip = upstream.Clip(parametric_box_measures(params, ()).clip)
    if kind == "actuator":
        return clip.top
    if kind == "catch":
        return clip.bottom
    raise ValueError(f"unknown latch profile {kind}")


def transformed_clip_profile(params: dict, center: float, kind: str, mirrored: bool = True) -> cq.Workplane:
    box = params["parametric_box"]
    source_x = center - params["normalization"]["translate"][0]
    relative_z = box["height"] - box["top_height"] - box["height"] / 2.0
    placed = _source_clip_profile(params, kind).translate((source_x, -box["depth"] / 2.0, relative_z - 2.0))
    placed = placed.translate((0.0, -box["depth"] / 2.0 - box["thickness"], 0.0))
    normalized = normalize_source_half(placed, params)
    return normalized.mirror("XY", (0.0, 0.0, seam_z(params))) if mirrored else normalized


def latch_profile_components(params: dict, half: str) -> tuple[tuple[str, cq.Workplane], ...]:
    kind = _profile_kind(half)
    return tuple(
        (f"{half}_latch_{kind}_{index + 1}", transformed_clip_profile(params, center, kind))
        for index, center in enumerate(latch_centers(params))
    )


def latch_bridge_components(params: dict, half: str) -> tuple[tuple[str, cq.Workplane], ...]:
    kind = _profile_kind(half)
    spec = params["bottom_latches"]
    bridge = spec["root_bridge"]
    dimensions = spec[kind]
    y_min = wall_y(params) - bridge["host_profile_overlap"]
    y_max = y_min + bridge["depth"]
    z_min, z_max = dimensions["z"]
    return tuple(
        (
            f"{half}_latch_{kind}_{index + 1}_root_bridge",
            _prism((center - dimensions["width"] / 2.0, center + dimensions["width"] / 2.0, y_min, y_max), z_min, z_max),
        )
        for index, center in enumerate(latch_centers(params))
    )


def bottom_latch_components(params: dict, half: str) -> tuple[tuple[str, cq.Workplane], ...]:
    profiles = latch_profile_components(params, half)
    bridges = latch_bridge_components(params, half)
    if len(profiles) != len(bridges):
        raise ValueError(f"{half} latch profile and root bridge counts differ")
    components = []
    for (name, profile), (_, bridge) in zip(profiles, bridges):
        _require(_intersection_volume(profile, bridge) > VOLUME_TOLERANCE, f"{name} root bridge does not overlap its profile")
        components.append((name, profile.union(bridge).clean()))
    return tuple(components)


def add_bottom_latches(params: dict, model: cq.Workplane, half: str) -> cq.Workplane:
    result = model
    profiles = latch_profile_components(params, half)
    bridges = latch_bridge_components(params, half)
    if len(profiles) != len(bridges):
        raise ValueError(f"{half} latch profile and root bridge counts differ")
    for (_, profile), (_, bridge) in zip(profiles, bridges):
        latch = profile.union(bridge).clean()
        _require(_intersection_volume(profile, bridge) > VOLUME_TOLERANCE, f"{half} latch root bridge does not overlap its profile")
        _require(_intersection_volume(result, bridge) > VOLUME_TOLERANCE, f"{half} latch root bridge does not overlap its host")
        result = result.union(latch).clean()
    return result.clean()


def validate_source_box_clip_ownership(source_box) -> None:
    _require(tuple(source_box.measures.clip.positions_x) == (), "protective-case source box owns Clip solids")


def _valid_single(model: cq.Workplane, label: str) -> None:
    shape = cast(cq.Shape, model.val())
    _require(shape.isValid() and len(model.solids().vals()) == 1, f"{label} must be one valid solid")


def _assert_bbox(model: cq.Workplane, expected: tuple[float, float, float, float, float, float], label: str) -> None:
    box = _shape_box(model)
    actual = (box.xmin, box.xmax, box.ymin, box.ymax, box.zmin, box.zmax)
    _require(all(abs(measured - target) <= TOLERANCE for measured, target in zip(actual, expected)), f"{label} bbox changed: {actual}")


def _root_face_area(profile: cq.Workplane, y: float) -> float:
    faces = [
        cast(cq.Face, face)
        for face in profile.faces().vals()
        if abs(cast(cq.Face, face).BoundingBox().ymin - y) <= TOLERANCE and abs(cast(cq.Face, face).BoundingBox().ymax - y) <= TOLERANCE
    ]
    _require(len(faces) == 1 and faces[0].geomType() == "PLANE", "latch root face is not one plane")
    return faces[0].Area()


def _union(components: tuple[tuple[str, cq.Workplane], ...]) -> cq.Workplane:
    result = components[0][1]
    for _, component in components[1:]:
        result = result.union(component).clean()
    return result


def _validate_profiles(params: dict) -> None:
    expected = {
        "shallow": ((62.8, 82.8, 148.4, 151.4, 46.035410, 60.85), ACTUATOR_VOLUME, ACTUATOR_ROOT_FACE, 18),
        "deep": ((60.8, 84.8, 148.4, 149.834972, 47.35, 51.85), CATCH_VOLUME, CATCH_ROOT_FACE, 6),
    }
    for half, (bbox, expected_volume, expected_root_face, expected_fillet_count) in expected.items():
        components = latch_profile_components(params, half)
        _require(len(components) == 2, f"{half} latch count changed")
        for index, (_, profile) in enumerate(components):
            _valid_single(profile, f"{half} latch profile {index + 1}")
            shift = 110.0 if index else 0.0
            shifted_bbox = (bbox[0] + shift, bbox[1] + shift, bbox[2], bbox[3], bbox[4], bbox[5])
            _assert_bbox(profile, shifted_bbox, f"{half} latch profile {index + 1}")
            _require(abs(_volume(profile) - expected_volume) <= TOLERANCE, f"{half} latch profile volume changed")
            circles = [edge for edge in cast(list[cq.Shape], profile.edges().vals()) if edge.geomType() == "CIRCLE"]
            _require(len(circles) == expected_fillet_count and all(abs(getattr(edge, "radius")() - 0.3) <= TOLERANCE for edge in circles), f"{half} latch source fillets changed")
            _require(abs(_root_face_area(profile, wall_y(params)) - expected_root_face) <= TOLERANCE, f"{half} latch root face changed")
            unmirrored = transformed_clip_profile(params, latch_centers(params)[index], _profile_kind(half), mirrored=False)
            unmirrored_box = _shape_box(unmirrored)
            mirrored_box = _shape_box(profile)
            _require(
                abs(mirrored_box.xmin - unmirrored_box.xmin) <= TOLERANCE
                and abs(mirrored_box.xmax - unmirrored_box.xmax) <= TOLERANCE
                and abs(mirrored_box.ymin - unmirrored_box.ymin) <= TOLERANCE
                and abs(mirrored_box.ymax - unmirrored_box.ymax) <= TOLERANCE
                and abs(mirrored_box.zmin - (2.0 * seam_z(params) - unmirrored_box.zmax)) <= TOLERANCE
                and abs(mirrored_box.zmax - (2.0 * seam_z(params) - unmirrored_box.zmin)) <= TOLERANCE,
                f"{half} latch profile mirror changed",
            )


def _validate_bridges(params: dict, corrected_deep: cq.Workplane, corrected_shallow: cq.Workplane) -> None:
    spec = params["bottom_latches"]
    bridge = spec["root_bridge"]
    _require(abs(bridge["depth"] - 2.0 * bridge["host_profile_overlap"]) <= TOLERANCE, "latch bridge overlap is not symmetric")
    for half, host, expected in (("shallow", corrected_shallow, (19.4, 56.95, 60.70, 7.275)), ("deep", corrected_deep, (23.4, 47.60, 51.60, 9.36))):
        kind = _profile_kind(half)
        dimensions = spec[kind]
        _require(tuple(dimensions["z"]) == expected[1:3] and dimensions["width"] == expected[0], f"{half} latch bridge dimensions changed")
        profiles = latch_profile_components(params, half)
        bridges = latch_bridge_components(params, half)
        _require(len(bridges) == 2, f"{half} latch root bridge count changed")
        for index, ((_, profile), (_, root)) in enumerate(zip(profiles, bridges)):
            center = latch_centers(params)[index]
            _assert_bbox(root, (center - expected[0] / 2.0, center + expected[0] / 2.0, wall_y(params) - bridge["host_profile_overlap"], wall_y(params) + bridge["host_profile_overlap"], expected[1], expected[2]), f"{half} latch root bridge {index + 1}")
            _require(abs(_volume(root) - expected[0] * bridge["depth"] * (expected[2] - expected[1])) <= TOLERANCE, f"{half} latch root bridge volume changed")
            _require(abs(_intersection_volume(root, host) - expected[3]) <= TOLERANCE, f"{half} latch host overlap changed")
            _require(abs(_intersection_volume(root, profile) - expected[3]) <= TOLERANCE, f"{half} latch profile overlap changed")
            _require(_intersection_volume(root, host.union(profile)) >= _volume(root) - TOLERANCE, f"{half} latch root bridge is visible")


def _validate_clearances(params: dict) -> None:
    _require(hinge_centers(params) == (63.9, 191.7), "hinge centers changed")
    opening = opening_prism(params)
    mating_parts = {"shallow": shallow_tongue_components(params), "deep": deep_receiver_components(params)}
    for half in ("shallow", "deep"):
        for _, component in latch_profile_components(params, half) + latch_bridge_components(params, half):
            _require(_intersection_volume(component, opening) <= VOLUME_TOLERANCE, f"{half} latch intersects mating opening")
            _require(all(_intersection_volume(component, mating) <= VOLUME_TOLERANCE for _, mating in mating_parts[half]), f"{half} latch intersects mating rails")
            _require(
                all(_intersection_volume(component, transformed_hinge_profile(params, center, half)) <= VOLUME_TOLERANCE for center in hinge_centers(params)),
                f"{half} latch intersects hinge geometry",
            )


def _canonical_device(params: dict) -> cq.Workplane:
    try:
        from .protective_case_device_fit import build_canonical_face_down_device
    except ImportError:
        from protective_case_device_fit import build_canonical_face_down_device
    return build_canonical_face_down_device()


def _validate_sweep(
    params: dict,
    corrected_deep: cq.Workplane,
    corrected_shallow: cq.Workplane,
    canonical_device: cq.Workplane,
) -> None:
    deep_profiles = latch_profile_components(params, "deep")
    shallow_profiles = latch_profile_components(params, "shallow")
    deep_bridges = latch_bridge_components(params, "deep")
    shallow_bridges = latch_bridge_components(params, "shallow")
    deep_latches = _union(deep_profiles + deep_bridges)
    shallow_latches = _union(shallow_profiles + shallow_bridges)
    axis_y, axis_z = hinge_axis(params)

    def collision(angle: float, moving: cq.Workplane = shallow_latches, stationary: cq.Workplane = deep_latches) -> float:
        return _intersection_volume(moving.rotate((0.0, axis_y, axis_z), (1.0, axis_y, axis_z), angle), stationary)

    dense = {round(index / 100.0, 2): collision(index / 100.0) for index in range(0, 201)}
    _require(dense[0.0] <= VOLUME_TOLERANCE, "latches collide at closed 0 degrees")
    _require(all(dense[angle] > VOLUME_TOLERANCE for angle in (0.01, 0.55, 1.0, 1.45)), "latch collision band lost engagement")
    _require(all(dense[angle] <= VOLUME_TOLERANCE for angle in (1.46, 1.5, 2.0)), "latches do not release at 1.46 degrees")
    max_angle, max_collision = max(dense.items(), key=lambda item: item[1])
    _require(abs(max_angle - 0.55) <= 1.0e-9 and abs(max_collision - 39.169681) <= TOLERANCE, "latch collision maximum changed")
    _require(abs(dense[1.0] - 16.690446) <= TOLERANCE, "one-degree latch engagement changed")

    integer = {angle: collision(angle) for angle in range(181)}
    _require(all(integer[angle] <= VOLUME_TOLERANCE for angle in (0, *range(2, 181))), "integer latch sweep has an unexpected collision")
    _require(abs(integer[1] - 16.690446) <= TOLERANCE, "integer one-degree latch engagement changed")
    for angle in (0.55, 1.0):
        moving_actuators = _union(shallow_profiles + shallow_bridges).rotate((0.0, axis_y, axis_z), (1.0, axis_y, axis_z), angle)
        moving_shell = corrected_shallow.rotate((0.0, axis_y, axis_z), (1.0, axis_y, axis_z), angle)
        intentional = _intersection_volume(moving_actuators, deep_latches)
        _require(abs(collision(angle) - intentional) <= VOLUME_TOLERANCE, f"latch collision has a non-actuator/catch component at {angle} degrees")
        _require(intentional > 0.0, f"actuator/catch collision missing at {angle} degrees")
        _require(_intersection_volume(moving_actuators, corrected_deep) <= VOLUME_TOLERANCE, "actuator/deep shell collision is not zero")
        _require(_intersection_volume(moving_shell, deep_latches) <= VOLUME_TOLERANCE, "moving lid shell/catch collision is not zero")
        _require(_intersection_volume(moving_shell, corrected_deep) <= VOLUME_TOLERANCE, "no-latch shell collision is not zero")

    final_deep = add_bottom_latches(params, corrected_deep, "deep")
    final_shallow = add_bottom_latches(params, corrected_shallow, "shallow")
    assembly_collisions = []
    lid_device_collisions = []
    no_latch_collisions = []
    for angle in range(181):
        moving_no_latch = corrected_shallow.rotate((0.0, axis_y, axis_z), (1.0, axis_y, axis_z), angle)
        moving_lid = final_shallow.rotate((0.0, axis_y, axis_z), (1.0, axis_y, axis_z), angle)
        no_latch_collisions.append(_intersection_volume(moving_no_latch, corrected_deep))
        assembly_collisions.append(_intersection_volume(moving_lid, final_deep))
        lid_device_collisions.append(_intersection_volume(moving_lid, canonical_device))
    _require(all(collision <= VOLUME_TOLERANCE for collision in no_latch_collisions), "corrected shell sweep has a collision")
    _require(assembly_collisions[0] <= VOLUME_TOLERANCE, "final latch owners collide at closed 0 degrees")
    _require(abs(assembly_collisions[1] - 16.690446) <= TOLERANCE, "final latch owner one-degree engagement changed")
    _require(all(collision <= VOLUME_TOLERANCE for collision in assembly_collisions[2:]), "final latch owner sweep has a collision after 2 degrees")
    _require(all(collision <= VOLUME_TOLERANCE for collision in lid_device_collisions), "moving final lid collides with canonical device")
    print(f"bottom_latches=centers={latch_centers(params)} shallow_actuator=522.288830mm3 deep_catch=80.593856mm3 bridge_overlap=0.10mm collision_band=0.01..1.45deg max=39.169681mm3@0.55deg one_degree=16.690446mm3 release=1.46deg PASS")
    print(f"bottom_latch_assembly_sweep=corrected_shell_max={max(no_latch_collisions):.6f}mm3 final_owners=0:{assembly_collisions[0]:.6f}mm3/1:{assembly_collisions[1]:.6f}mm3/2..180:max={max(assembly_collisions[2:]):.6f}mm3 lid_device_0..180:max={max(lid_device_collisions):.6f}mm3 PASS")


def validate_bottom_latches(
    params: dict,
    corrected_deep: cq.Workplane,
    corrected_shallow: cq.Workplane,
    canonical_device: cq.Workplane | None = None,
) -> None:
    _require(seam_z(params) == SEAM_Z, "latch seam changed")
    _require(latch_centers(params) == (72.8, 182.8), "latch centers changed")
    _require(abs(wall_y(params) - 148.4) <= TOLERANCE, "latch wall plane changed")
    _validate_profiles(params)
    _validate_bridges(params, corrected_deep, corrected_shallow)
    _validate_clearances(params)
    _validate_sweep(params, corrected_deep, corrected_shallow, _canonical_device(params) if canonical_device is None else canonical_device)
