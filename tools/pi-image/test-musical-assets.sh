#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fixture="$(mktemp -d)"
trap 'rm -rf "$fixture"' EXIT

mkdir -p "$fixture/save-documents"
cargo run --locked --quiet -p playback-runtime --bin split_default_config -- \
    "$root/config/generated/pi/default.json" "$fixture/save-documents/pi"
for save_document in system.json default.patch.json; do
    test -f "$fixture/save-documents/pi/$save_document"
    test ! -L "$fixture/save-documents/pi/$save_document"
done
bash "$root/tools/pi-image/stage-musical-assets.sh" "$fixture/root"
mkdir -p "$fixture/rootfs/home/pi/samples" "$fixture/rootfs/usr/share/octessera/samples"
printf 'user sentinel\n' > "$fixture/rootfs/home/pi/samples/user-sample.wav"
bash "$root/tools/pi-image/install-musical-assets.sh" "$fixture/root" "$fixture/rootfs"
test ! -e "$fixture/rootfs/usr/share/octessera/samples/files"
python3 - "$root" "$fixture/root" "$fixture/rootfs" <<'PY'
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
stage = pathlib.Path(sys.argv[2])
sys.path.insert(0, str(root / "tools/samples"))
from sample_library import verify_media_tree, verify_metadata_tree

for tree in (stage, pathlib.Path(sys.argv[3])):
    verify_media_tree(tree / "home/pi/samples", root / "samples", ("sd-card",))
    verify_metadata_tree(tree / "usr/share/octessera/samples", root / "samples")
if not (stage / "home/pi/samples/sd-card").is_dir():
    raise SystemExit("Raspberry SD-card mount subtree is missing")
if (stage / "home/pi/samples/sd-card").is_symlink():
    raise SystemExit("Raspberry SD-card mount subtree is symlinked")
if "OCTESSERA_PI_SAMPLES_DIR=/home/pi/samples" not in (root / "tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera.service").read_text(encoding="utf-8"):
    raise SystemExit("Raspberry runtime sample root is not /home/pi/samples")
PY
python3 - "$root" "$fixture/save-documents/pi" <<'PY'
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
documents = pathlib.Path(sys.argv[2])
stage = (root / "tools/pi-image/stage4-octessera/02-setup-service/00-run.sh").read_text(encoding="utf-8")
expected = {
    "system.json": '"kind": "octessera.system"',
    "default.patch.json": '"kind": "octessera.patch"',
}
for name, kind in expected.items():
    source = documents / name
    target = pathlib.Path("home/pi/presets") / name
    if source.is_symlink() or not source.is_file():
        raise SystemExit(f"Projected Raspberry save document is missing or symlinked: {source}")
    if kind not in source.read_text(encoding="utf-8"):
        raise SystemExit(f"Projected Raspberry save document has the wrong kind: {name}")
    installed = documents.parent / "staged" / target
    installed.parent.mkdir(parents=True, exist_ok=True)
    installed.write_bytes(source.read_bytes())
    if installed.read_bytes() != source.read_bytes():
        raise SystemExit(f"Staged Raspberry save document differs from projection: {target}")
if ('for save_document in system.json default.patch.json; do' not in stage
        or '"$save_documents/$save_document"' not in stage
        or '"$ROOTFS_DIR/home/pi/presets/$save_document"' not in stage):
    raise SystemExit("Raspberry stage copy contract is missing projected save documents")
if 'chmod 0644 /home/pi/presets/system.json /home/pi/presets/default.patch.json' not in stage:
    raise SystemExit("Raspberry stage does not set split save-document modes")
if 'config/generated/pi/default.json' in stage:
    raise SystemExit("Raspberry stage still treats the mixed default as an active source")
PY
printf 'Raspberry musical asset staging passed\n'
