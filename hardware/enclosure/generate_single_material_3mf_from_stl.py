from __future__ import annotations

import struct
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parent
ARTIFACT_ROOT = ROOT.parent.parent / "release-artifacts" / "enclosure"
STL_ROOT = ARTIFACT_ROOT / "stl"
THREEMF_ROOT = ARTIFACT_ROOT / "3mf-multicolor"


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


def mesh_xml(triangles: list[tuple[tuple[float, float, float], tuple[float, float, float], tuple[float, float, float]]]) -> str:
    vertices_xml = []
    triangles_xml = []
    for index, triangle in enumerate(triangles):
        vertex_offset = index * 3
        for x, y, z in triangle:
            vertices_xml.append(f'<vertex x="{x:.6f}" y="{y:.6f}" z="{z:.6f}" />')
        triangles_xml.append(
            f'<triangle v1="{vertex_offset}" v2="{vertex_offset + 1}" v3="{vertex_offset + 2}" />'
        )
    return "<mesh><vertices>" + "".join(vertices_xml) + "</vertices><triangles>" + "".join(triangles_xml) + "</triangles></mesh>"


def model_xml(path: Path) -> str:
    return f'''<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xml:lang="en-US" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <metadata name="Title">{path.stem}</metadata>
  <resources>
    <object id="1" type="model">{mesh_xml(read_binary_stl_triangles(path))}</object>
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
        write_3mf(stl_path)


if __name__ == "__main__":
    main()
