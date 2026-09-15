from __future__ import annotations

from pathlib import Path
import zipfile
from xml.etree import ElementTree


BBox = tuple[float, float, float, float, float, float]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def xml_elements(element, name: str):
    return tuple(child for child in element.iter() if child.tag.rsplit("}", 1)[-1] == name)


def _bbox(vertices: list[tuple[float, float, float]]) -> BBox:
    return (
        min(vertex[0] for vertex in vertices),
        min(vertex[1] for vertex in vertices),
        min(vertex[2] for vertex in vertices),
        max(vertex[0] for vertex in vertices),
        max(vertex[1] for vertex in vertices),
        max(vertex[2] for vertex in vertices),
    )


def _read_model(path: Path):
    required_members = {"[Content_Types].xml", "_rels/.rels", "3D/3dmodel.model"}
    try:
        with zipfile.ZipFile(path) as package:
            require(package.testzip() is None, f"{path} has corrupt package data")
            missing = required_members - set(package.namelist())
            require(not missing, f"{path} is missing 3MF members: {sorted(missing)}")
            model = ElementTree.fromstring(package.read("3D/3dmodel.model"))
    except (zipfile.BadZipFile, ElementTree.ParseError, KeyError) as exc:
        raise ValueError(f"{path} is not a valid 3MF package") from exc
    require(model.tag.rsplit("}", 1)[-1] == "model", f"{path} has no model root")
    resources = xml_elements(model, "resources")
    builds = xml_elements(model, "build")
    require(len(resources) == 1 and len(builds) == 1, f"{path} must have one resources and build element")
    objects = xml_elements(resources[0], "object")
    items = xml_elements(builds[0], "item")
    require(bool(objects) and len(items) == 1, f"{path} must have model objects and exactly one build item")
    object_by_id = {int(obj.attrib["id"]): obj for obj in objects}
    require(len(object_by_id) == len(objects), f"{path} has duplicate object IDs")
    root_ids = [object_id for object_id, obj in object_by_id.items() if not xml_elements(obj, "mesh")]
    require(len(root_ids) == 1, f"{path} must have exactly one synthetic root object")
    root_id = root_ids[0]
    require(int(items[0].attrib["objectid"]) == root_id, f"{path} build item must reference root object {root_id}")
    require(len(xml_elements(builds[0], "item")) == 1, f"{path} has independent top-level build items")
    root_components = xml_elements(object_by_id[root_id], "component")
    mesh_object_ids = sorted(object_id for object_id, obj in object_by_id.items() if xml_elements(obj, "mesh"))
    require(mesh_object_ids == list(range(1, root_id)), f"{path} mesh object IDs must be contiguous before root {root_id}")
    require([int(component.attrib["objectid"]) for component in root_components] == mesh_object_ids, f"{path} root components must reference every mesh object exactly once")
    identity = "1 0 0 0 1 0 0 0 1 0 0 0"
    require(all(component.attrib.get("transform") == identity for component in root_components), f"{path} root component transforms must be identity")
    mesh_bboxes: dict[int, BBox] = {}
    for object_id in mesh_object_ids:
        vertices = [
            (float(vertex.attrib["x"]), float(vertex.attrib["y"]), float(vertex.attrib["z"]))
            for vertex in xml_elements(object_by_id[object_id], "vertex")
        ]
        require(bool(vertices), f"{path} mesh object {object_id} has no vertices")
        require(bool(xml_elements(object_by_id[object_id], "triangle")), f"{path} mesh object {object_id} has no triangles")
        mesh_bboxes[object_id] = _bbox(vertices)
    return mesh_bboxes, root_id


def validate_3mf_package(
    path: Path,
    expected_bbox: BBox,
    expected_mesh_bboxes: dict[int, BBox],
) -> dict[int, BBox]:
    mesh_bboxes, root_id = _read_model(path)
    require(set(mesh_bboxes) == set(expected_mesh_bboxes), f"{path} has unexpected mesh object IDs: {sorted(mesh_bboxes)}")
    vertices = [
        (bbox[0], bbox[1], bbox[2])
        for bbox in mesh_bboxes.values()
    ] + [
        (bbox[3], bbox[4], bbox[5])
        for bbox in mesh_bboxes.values()
    ]
    actual_bbox = _bbox(vertices)
    require(all(abs(actual - expected) <= 0.05 for actual, expected in zip(actual_bbox, expected_bbox)), f"{path} mesh bbox={actual_bbox}, expected approximately {expected_bbox}")
    for object_id, actual in mesh_bboxes.items():
        expected = expected_mesh_bboxes[object_id]
        require(all(abs(measured - target) <= 0.05 for measured, target in zip(actual, expected)), f"{path} mesh object {object_id} bbox={actual}, expected approximately {expected}")
    print(f"{path.name}=valid_3mf root={root_id} mesh_objects={len(mesh_bboxes)} aggregate_bbox={actual_bbox}")
    return mesh_bboxes


def _settings_parts(path: Path) -> dict[int, tuple[str, str | None]]:
    try:
        with zipfile.ZipFile(path) as package:
            settings = ElementTree.fromstring(package.read("Metadata/model_settings.config"))
    except (zipfile.BadZipFile, ElementTree.ParseError, KeyError) as exc:
        raise ValueError(f"{path} has no valid model settings") from exc
    parts = {}
    for part in xml_elements(settings, "part"):
        part_id = int(part.attrib["id"])
        name = next((entry.attrib.get("value") for entry in xml_elements(part, "metadata") if entry.attrib.get("key") == "name"), None)
        extruder = next((entry.attrib.get("value") for entry in xml_elements(part, "metadata") if entry.attrib.get("key") == "extruder"), None)
        require(name is not None, f"{path} part {part_id} has no name")
        parts[part_id] = (name, extruder)
    return parts


def validate_model_parts(path: Path, expected: dict[int, tuple[str, str]]) -> None:
    actual = _settings_parts(path)
    require(actual == expected, f"{path} has unexpected model parts or extruders: {actual}")
    print(f"{path.name}=model_parts={len(actual)} ids_mesh_correlated settings=PASS")


def validate_deep_tub_debossed_package(path: Path, expected_bbox: BBox, expected_mesh_bboxes: dict[int, BBox]) -> None:
    validate_3mf_package(path, expected_bbox, expected_mesh_bboxes)
    validate_model_parts(path, {1: ("transport_case_deep_tub_debossed_logo", "1")})


def validate_deep_tub_multicolor_package(path: Path, expected_bbox: BBox, expected_mesh_bboxes: dict[int, BBox]) -> None:
    validate_3mf_package(path, expected_bbox, expected_mesh_bboxes)
    validate_model_parts(
        path,
        {
            1: ("transport_case_deep_tub_body", "1"),
            2: ("transport_case_deep_tub_logo", "2"),
            3: ("transport_case_deep_tub_device_orientation_guide", "2"),
        },
    )


def validate_shallow_lid_debossed_package(path: Path, expected_bbox: BBox, expected_mesh_bboxes: dict[int, BBox]) -> None:
    validate_3mf_package(path, expected_bbox, expected_mesh_bboxes)
    validate_model_parts(path, {1: ("transport_case_shallow_lid_debossed_branding", "1")})


def validate_shallow_lid_multicolor_package(path: Path, expected_bbox: BBox, expected_mesh_bboxes: dict[int, BBox]) -> None:
    validate_3mf_package(path, expected_bbox, expected_mesh_bboxes)
    validate_model_parts(
        path,
        {
            1: ("transport_case_shallow_lid_body", "1"),
            2: ("transport_case_shallow_lid_logo", "2"),
            3: ("transport_case_shallow_lid_wordmark", "2"),
        },
    )
