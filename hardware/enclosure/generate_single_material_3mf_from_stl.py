from __future__ import annotations

import struct
import zipfile
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parent
ARTIFACT_ROOT = ROOT.parent.parent / "release-artifacts" / "enclosure"
STL_ROOT = ARTIFACT_ROOT / "stl"
THREEMF_ROOT = ARTIFACT_ROOT / "3mf-multicolor"
SKIPPED_STEMS = {"case_top_two_level_cadquery"}


def read_binary_stl_triangles(path: Path) -> list[tuple[tuple[float, float, float], tuple[float, float, float], tuple[float, float, float]]]:
    data = path.read_bytes()
    if len(data) < 84:
        raise ValueError(f"{path} is too small to be a binary STL")
    triangle_count = struct.unpack_from("<I", data, 80)[0]
    expected_size = 84 + triangle_count * 50
    if expected_size != len(data):
        raise ValueError(f"{path} is not a binary STL with expected size")
    triangles = []
    offset = 84
    for _ in range(triangle_count):
        values = struct.unpack_from("<12fH", data, offset)
        triangles.append((values[3:6], values[6:9], values[9:12]))
        offset += 50
    return triangles


def vertex_key(vertex: tuple[float, float, float]) -> tuple[int, int, int]:
    return (
        round(vertex[0] * 1_000_000),
        round(vertex[1] * 1_000_000),
        round(vertex[2] * 1_000_000),
    )


def triangle_area_sq(triangle: tuple[tuple[float, float, float], tuple[float, float, float], tuple[float, float, float]]) -> float:
    a, b, c = triangle
    ab = (b[0] - a[0], b[1] - a[1], b[2] - a[2])
    ac = (c[0] - a[0], c[1] - a[1], c[2] - a[2])
    cross = (
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    )
    return cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]


def indexed_mesh(
    triangles: list[tuple[tuple[float, float, float], tuple[float, float, float], tuple[float, float, float]]],
) -> tuple[list[tuple[float, float, float]], list[tuple[int, int, int]]]:
    vertices: list[tuple[float, float, float]] = []
    vertex_indices: dict[tuple[int, int, int], int] = {}
    indexed_triangles: list[tuple[int, int, int]] = []
    for triangle in triangles:
        if triangle_area_sq(triangle) < 1e-18:
            continue
        indices = []
        for vertex in triangle:
            key = vertex_key(vertex)
            if key not in vertex_indices:
                vertex_indices[key] = len(vertices)
                vertices.append(vertex)
            indices.append(vertex_indices[key])
        if len(set(indices)) == 3:
            indexed_triangles.append((indices[0], indices[1], indices[2]))
    return vertices, indexed_triangles


def assert_manifold_edges(path: Path, triangles: list[tuple[int, int, int]]) -> None:
    edge_counts: Counter[tuple[int, int]] = Counter()
    for a, b, c in triangles:
        for edge in ((a, b), (b, c), (c, a)):
            low, high = sorted(edge)
            edge_counts[(low, high)] += 1
    bad_edges = [edge for edge, count in edge_counts.items() if count != 2]
    if bad_edges:
        raise ValueError(f"{path} has {len(bad_edges)} non-manifold 3MF edges after indexing")


def mesh_xml(path: Path) -> str:
    vertices, triangles = indexed_mesh(read_binary_stl_triangles(path))
    assert_manifold_edges(path, triangles)
    vertices_xml = [f'<vertex x="{x:.6f}" y="{y:.6f}" z="{z:.6f}" />' for x, y, z in vertices]
    triangles_xml = [f'<triangle v1="{a}" v2="{b}" v3="{c}" />' for a, b, c in triangles]
    return "<mesh><vertices>" + "".join(vertices_xml) + "</vertices><triangles>" + "".join(triangles_xml) + "</triangles></mesh>"


def model_xml(path: Path) -> str:
    return f'''<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <metadata name="Title">{path.stem}</metadata>
  <resources>
    <object id="1" type="model">{mesh_xml(path)}</object>
  </resources>
  <build>
    <item objectid="1" />
  </build>
</model>
'''


def write_3mf(stl_path: Path) -> None:
    out_path = THREEMF_ROOT / f"{stl_path.stem}.3mf"
    THREEMF_ROOT.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(out_path, "w", compression=zipfile.ZIP_DEFLATED) as package:
        package.writestr(
            "[Content_Types].xml",
            '''<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml" />
  <Default Extension="model" ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml" />
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
        package.writestr("3D/3dmodel.model", model_xml(stl_path))
    print(f"wrote {out_path}")


def main() -> None:
    for stl_path in sorted(STL_ROOT.glob("case_*.stl")) + sorted(STL_ROOT.glob("standoff_*.stl")):
        if stl_path.stem not in SKIPPED_STEMS:
            write_3mf(stl_path)


if __name__ == "__main__":
    main()
