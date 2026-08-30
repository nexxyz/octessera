#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fixture="$(mktemp -d)"
trap 'rm -rf "$fixture"' EXIT

bash "$root/tools/pi-image/stage-musical-assets.sh" "$fixture/root"
mkdir -p "$fixture/rootfs/home/pi/samples" "$fixture/rootfs/usr/share/octessera/samples"
mkdir -p "$fixture/rootfs/etc"
printf '%s\n' 'root:x:0:0:root:/root:/bin/sh' 'pi:x:1000:1000:Pi:/home/pi:/bin/bash' > "$fixture/rootfs/etc/passwd"
printf '%s\n' 'root:x:0:' 'pi:x:1000:' > "$fixture/rootfs/etc/group"
printf 'user sentinel\n' > "$fixture/rootfs/home/pi/samples/user-sample.wav"
bash "$root/tools/pi-image/install-musical-assets.sh" "$fixture/root" "$fixture/rootfs"
chown -R 1000:1000 "$fixture/rootfs/home/pi/samples"
python3 "$root/tools/pi-image/verify-rpi-samples.py" --root "$fixture/rootfs" --repository-root "$root"
test ! -e "$fixture/rootfs/usr/share/octessera/samples/files"
python3 - "$root" "$fixture/root" <<'PY'
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
stage = pathlib.Path(sys.argv[2])
sys.path.insert(0, str(root / "tools/samples"))
from sample_library import read_manifest, verify_media_tree, verify_metadata_tree, verify_manifest

records = read_manifest(root / "samples/MANIFEST.tsv")
verify_media_tree(stage / "home/pi/samples", records, ("sd-card",))
verify_metadata_tree(stage / "usr/share/octessera/samples", root / "samples")
verify_manifest(stage / "usr/share/octessera/samples/MANIFEST.tsv", records)
if not (stage / "home/pi/samples/sd-card").is_dir():
    raise SystemExit("Raspberry SD-card mount subtree is missing")
if (stage / "home/pi/samples/sd-card").is_symlink():
    raise SystemExit("Raspberry SD-card mount subtree is symlinked")
if "OCTESSERA_PI_SAMPLES_DIR=/home/pi/samples" not in (root / "tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera.service").read_text(encoding="utf-8"):
    raise SystemExit("Raspberry runtime sample root is not /home/pi/samples")
PY
printf 'Raspberry musical asset staging passed\n'
