from __future__ import annotations

import argparse
import json
import zipfile
from collections import Counter
from dataclasses import dataclass
from html import escape
from pathlib import Path
from typing import cast

import cadquery as cq

import generate_two_level_enclosure_cadquery as enclosure
import generate_encoder_caps_cadquery as enc
import generate_mx_keycaps_cadquery as mx


ROOT = Path(__file__).resolve().parent
ARTIFACT_ROOT = ROOT.parent.parent / "release-artifacts" / "enclosure"
THREEMF_ROOT = ARTIFACT_ROOT / "3mf-multicolor"
TOLERANCE = 0.04
FLUSH_DEPTH = 0.65


@dataclass(frozen=True)
class ModelPart:
    name: str
    model: cq.Workplane
    extruder: int


def shape_value(model: cq.Workplane) -> cq.Shape:
    return cast(cq.Shape, model.val())


def vertex_key(vertex: cq.Vector) -> tuple[int, int, int]:
    return (
        round(vertex.x * 1_000_000),
        round(vertex.y * 1_000_000),
        round(vertex.z * 1_000_000),
    )


def triangle_area_sq(
    vertices: list[cq.Vector],
    triangle: tuple[int, int, int],
) -> float:
    a, b, c = (vertices[index] for index in triangle)
    ab = (b.x - a.x, b.y - a.y, b.z - a.z)
    ac = (c.x - a.x, c.y - a.y, c.z - a.z)
    cross = (
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    )
    return cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]


def add_indexed_mesh(
    mesh_vertices: list[cq.Vector],
    mesh_triangles: list[tuple[int, int, int]],
    vertices: list[cq.Vector],
    vertex_indices: dict[tuple[int, int, int], int],
    triangles: list[tuple[int, int, int]],
) -> None:
    for triangle in mesh_triangles:
        if triangle_area_sq(mesh_vertices, triangle) < 1e-18:
            continue
        indices = []
        for source_index in triangle:
            vertex = mesh_vertices[source_index]
            key = vertex_key(vertex)
            if key not in vertex_indices:
                vertex_indices[key] = len(vertices)
                vertices.append(vertex)
            indices.append(vertex_indices[key])
        if len(set(indices)) == 3:
            triangles.append((indices[0], indices[1], indices[2]))


def assert_manifold_edges(name: str, triangles: list[tuple[int, int, int]]) -> None:
    edge_counts: Counter[tuple[int, int]] = Counter()
    for a, b, c in triangles:
        for edge in ((a, b), (b, c), (c, a)):
            low, high = sorted(edge)
            edge_counts[(low, high)] += 1
    bad_edges = [edge for edge, count in edge_counts.items() if count != 2]
    if bad_edges:
        raise ValueError(f"{name} has {len(bad_edges)} non-manifold 3MF edges after indexing")


def translated(parts: list[cq.Workplane], z: float) -> list[cq.Workplane]:
    return [part.translate((0.0, 0.0, z)) for part in parts]


def cut_parts(body: cq.Workplane, parts: list[cq.Workplane]) -> cq.Workplane:
    result = body
    for part in parts:
        result = result.cut(part)
    return result.clean()


def compound(parts: list[cq.Workplane]) -> cq.Workplane:
    solids = []
    for part in parts:
        solids.extend(part.solids().vals())
    return cq.Workplane("XY").add(cq.Compound.makeCompound(solids))


def mesh_xml(name: str, model: cq.Workplane) -> tuple[str, int]:
    vertices: list[cq.Vector] = []
    vertex_indices: dict[tuple[int, int, int], int] = {}
    triangles: list[tuple[int, int, int]] = []
    for solid in cast(list[cq.Shape], model.solids().vals()):
        mesh_vertices, mesh_triangles = solid.tessellate(TOLERANCE)
        add_indexed_mesh(mesh_vertices, mesh_triangles, vertices, vertex_indices, triangles)
    assert_manifold_edges(name, triangles)
    vertices_xml = [f'<vertex x="{vertex.x:.6f}" y="{vertex.y:.6f}" z="{vertex.z:.6f}" />' for vertex in vertices]
    triangles_xml = [f'<triangle v1="{a}" v2="{b}" v3="{c}" />' for a, b, c in triangles]
    return (
        "<mesh><vertices>"
        + "".join(vertices_xml)
        + "</vertices><triangles>"
        + "".join(triangles_xml)
        + "</triangles></mesh>"
    ), len(triangles_xml)


def object_resources_xml(parts: list[ModelPart]) -> tuple[str, list[int]]:
    meshes = []
    face_counts = []
    for index, part in enumerate(parts, start=1):
        mesh, face_count = mesh_xml(part.name, part.model)
        meshes.append(f'<object id="{index}" p:UUID="000000{index:02d}-81cb-4c03-9d28-80fed5dfa1dc" type="model">{mesh}</object>')
        face_counts.append(face_count)
    return "".join(meshes), face_counts


def root_model_xml(parts: list[ModelPart], object_resources: str) -> str:
    root_object_id = len(parts) + 1
    components = "".join(
        f'<component objectid="{index}" p:UUID="000000{index:02d}-b206-40ff-9872-83e8017abed1" transform="1 0 0 0 1 0 0 0 1 0 0 0"/>'
        for index in range(1, len(parts) + 1)
    )
    return f'''<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02" xmlns:BambuStudio="http://schemas.bambulab.com/package/2021" xmlns:p="http://schemas.microsoft.com/3dmanufacturing/production/2015/06" requiredextensions="p">
  <metadata name="BambuStudio:3mfVersion">1</metadata>
  <resources>
    {object_resources}
    <object id="{root_object_id}" p:UUID="000000{root_object_id:02d}-61cb-4c03-9d28-80fed5dfa1dc" type="model">
      <components>{components}</components>
    </object>
  </resources>
  <build p:UUID="00000004-22b5-4d84-8835-1976022ea369">
    <item objectid="{root_object_id}" p:UUID="00000005-b1ec-4553-aec9-835e5b724bb4" printable="1"/>
  </build>
</model>
'''


def part_settings_xml(parts: list[ModelPart], face_counts: list[int]) -> str:
    return "".join(
        f'''    <part id="{index}" subtype="normal_part">
      <metadata key="name" value="{escape(part.name)}"/>
      <metadata key="matrix" value="1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1"/>
      <metadata key="extruder" value="{part.extruder}"/>
      <mesh_stat face_count="{face_count}" edges_fixed="0" degenerate_facets="0" facets_removed="0" facets_reversed="0" backwards_edges="0"/>
    </part>
'''
        for index, (part, face_count) in enumerate(zip(parts, face_counts), start=1)
    )


def model_settings_xml(name: str, parts: list[ModelPart], face_counts: list[int]) -> str:
    root_object_id = len(parts) + 1
    part_settings = "".join(
        f'''    <part id="{index}" subtype="normal_part">
      <metadata key="name" value="{escape(part.name)}"/>
      <metadata key="matrix" value="1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1"/>
      <metadata key="extruder" value="{part.extruder}"/>
      <mesh_stat face_count="{face_count}" edges_fixed="0" degenerate_facets="0" facets_removed="0" facets_reversed="0" backwards_edges="0"/>
    </part>
'''
        for index, (part, face_count) in enumerate(zip(parts, face_counts), start=1)
    )
    return f'''<?xml version="1.0" encoding="UTF-8"?>
<config>
  <object id="{root_object_id}">
    <metadata key="name" value="{escape(name)}"/>
    <metadata key="extruder" value="1"/>
    <metadata face_count="{sum(face_counts)}"/>
{part_settings}  </object>
</config>
'''


def write_3mf_parts(path: Path, parts: list[ModelPart]) -> None:
    object_resources, face_counts = object_resources_xml(parts)
    model_xml = root_model_xml(parts, object_resources)
    model_settings = model_settings_xml(path.stem, parts, face_counts)
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as package:
        package.writestr(
            "[Content_Types].xml",
            '''<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml" />
  <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml" />
  <Default Extension="config" ContentType="application/octet-stream" />
</Types>
''',
        )
        package.writestr(
            "_rels/.rels",
            '''<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Target="/3D/3dmodel.model" Id="rel0" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel" />
</Relationships>
''',
        )
        package.writestr("3D/3dmodel.model", model_xml)
        package.writestr("Metadata/model_settings.config", model_settings)


def write_3mf(path: Path, body: cq.Workplane, marking: cq.Workplane) -> None:
    write_3mf_parts(path, [ModelPart(f"{path.stem}_body", body, 1), ModelPart(f"{path.stem}_marking", marking, 2)])


def mx_variant(name: str, icon_parts: list[cq.Workplane]) -> tuple[cq.Workplane, cq.Workplane]:
    flush_icon = translated(icon_parts, -mx.ICON_RAISE)
    body = cut_parts(mx.make_basic_mx_keycap(), flush_icon)
    return body, compound(flush_icon)


def encoder_dot_variant(name: str) -> tuple[cq.Workplane, cq.Workplane]:
    if name == "main":
        body = enc.make_wide_knurled_cap()
        top_z = shape_value(body).BoundingBox().zmax
        cutter = enc.perimeter_dot_marking(
            enc.MAIN_MARK_DOT_COUNT,
            enc.MAIN_MARK_DOT_ORBIT_R,
            enc.AUX_MARK_DOT_R,
            FLUSH_DEPTH + 0.08,
            top_z - FLUSH_DEPTH,
        )
        label = enc.perimeter_dot_marking(
            enc.MAIN_MARK_DOT_COUNT,
            enc.MAIN_MARK_DOT_ORBIT_R,
            enc.AUX_MARK_DOT_R,
            FLUSH_DEPTH,
            top_z - FLUSH_DEPTH,
        )
    else:
        body = enc.make_aux_cap_body()
        top_z = shape_value(body).BoundingBox().zmax
        count = int(name)
        cutter = enc.dot_marking(
            count,
            enc.AUX_MARK_DOT_R,
            enc.AUX_MARK_DOT_SPACING,
            FLUSH_DEPTH + 0.08,
            top_z - FLUSH_DEPTH,
        )
        label = enc.dot_marking(
            count,
            enc.AUX_MARK_DOT_R,
            enc.AUX_MARK_DOT_SPACING,
            FLUSH_DEPTH,
            top_z - FLUSH_DEPTH,
        )
    return cut_parts(body, [cutter]), label


def enclosure_top_variant(params: dict) -> tuple[cq.Workplane, cq.Workplane]:
    body, marking_parts = enclosure_top_part_variant(params)
    marking_solids = cast(list[cq.Shape], [solid for part in marking_parts for solid in part.model.solids().vals()])
    flush_branding = cq.Workplane("XY").add(cq.Compound.makeCompound(marking_solids))
    return body, flush_branding


def enclosure_top_part_variant(params: dict) -> tuple[cq.Workplane, list[ModelPart]]:
    body = enclosure.build_body_model(params)
    model_bottom_z = shape_value(body).BoundingBox().zmin
    top_branding = enclosure.build_flush_top_branding_marking()
    port_marking_cutters = enclosure.build_flush_port_marking_cutters(params, model_bottom_z)
    for cutter in [top_branding, top_branding.translate((0.0, 0.0, 0.08)), port_marking_cutters]:
        for solid in cutter.solids().vals():
            body = body.cut(cq.Workplane("XY").add(solid)).clean()
    marking_parts = [
        *[ModelPart(name, part, 2) for name, part in enclosure.build_flush_top_branding_parts()],
        *[ModelPart(name, part, 2) for name, part in enclosure.build_flush_port_marking_parts(params, model_bottom_z)],
    ]
    return body, marking_parts


def write_case_top() -> None:
    params = json.loads(enclosure.PARAMS.read_text())
    case_body, marking_parts = enclosure_top_part_variant(params)
    case_filename = "case_top_two_level_multicolor.3mf"
    write_3mf_parts(THREEMF_ROOT / case_filename, [ModelPart("case_top_body", case_body, 1), *marking_parts])
    print(f"wrote {THREEMF_ROOT / case_filename}")


def write_mx_caps() -> None:
    mx_caps = {
        "mx_keycap_back_multicolor_flush.3mf": mx.back_icon(),
        "mx_keycap_play_multicolor_flush.3mf": mx.play_icon(),
        "mx_keycap_shift_multicolor_flush.3mf": mx.shift_icon(),
        "mx_keycap_fn_layer_multicolor_flush.3mf": mx.layer_icon(),
    }
    for filename, icon_parts in mx_caps.items():
        body, marking = mx_variant(filename, icon_parts)
        write_3mf(THREEMF_ROOT / filename, body, marking)
        print(f"wrote {THREEMF_ROOT / filename}")


def write_encoder_caps() -> None:
    encoder_dot_caps = {
        "encoder_cap_main_knurled_dots_multicolor_flush.3mf": "main",
        "encoder_cap_aux1_ribbed_dot_multicolor_flush.3mf": "1",
        "encoder_cap_aux2_ribbed_dots_multicolor_flush.3mf": "2",
        "encoder_cap_aux3_ribbed_dots_multicolor_flush.3mf": "3",
    }
    for filename, label in encoder_dot_caps.items():
        body, marking = encoder_dot_variant(label)
        write_3mf(THREEMF_ROOT / filename, body, marking)
        print(f"wrote {THREEMF_ROOT / filename}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--only", choices=("all", "case", "mx", "encoder"), default="all")
    args = parser.parse_args()
    THREEMF_ROOT.mkdir(parents=True, exist_ok=True)
    if args.only in ("all", "case"):
        write_case_top()
    if args.only in ("all", "mx"):
        write_mx_caps()
    if args.only in ("all", "encoder"):
        write_encoder_caps()


if __name__ == "__main__":
    main()
