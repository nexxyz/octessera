#!/usr/bin/env bash
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

root="$work/root"
fixtures="$work/fixtures"
mockbin="$work/bin"
mkdir -p "$root/releases/1.0.0" "$fixtures" "$mockbin"

cat > "$mockbin/jq" <<'EOF'
#!/usr/bin/env python3
import json, sys

args = sys.argv[1:]
raw = False
exit_on_empty = False
compact = False
vars = {}
pos = []
i = 0
while i < len(args):
    arg = args[i]
    if arg.startswith("-") and arg != "--arg":
        raw = raw or "r" in arg
        exit_on_empty = exit_on_empty or "e" in arg
        compact = compact or "c" in arg
        i += 1
    elif arg == "--arg":
        vars[args[i + 1]] = args[i + 2]
        i += 3
    else:
        pos.append(arg)
        i += 1

expr, file_path = pos[-2:]
with open(file_path, encoding="utf-8") as f:
    data = json.load(f)

missing = object()
value = missing
if expr == ".tag_name":
    value = data.get("tag_name", missing)
elif expr == ".assets[] | select(.name == $name) | .browser_download_url":
    for asset in data.get("assets", []):
        if asset.get("name") == vars.get("name"):
            value = asset.get("browser_download_url", missing)
            break
elif expr == ".next // empty":
    value = data.get("next") or missing
elif expr == ".previous // empty":
    value = data.get("previous") or missing
elif expr == ".current // .active // empty":
    value = data.get("current") or data.get("active") or missing
elif expr == "{tag, version, arch, binary, platforms}":
    value = {key: data.get(key) for key in ["tag", "version", "arch", "binary", "platforms"]}
elif expr == ".":
    value = data
else:
    raise SystemExit(f"unsupported jq expression: {expr}")

if value is missing:
    raise SystemExit(1 if exit_on_empty else 0)
if raw and isinstance(value, str):
    print(value)
else:
    print(json.dumps(value, separators=(",", ":") if compact else None))
EOF
chmod 0755 "$mockbin/jq"

cat > "$root/releases/1.0.0/octessera-pi" <<'EOF'
#!/usr/bin/env bash
echo old
EOF
chmod 0755 "$root/releases/1.0.0/octessera-pi"
cat > "$root/releases/1.0.0/update-manifest.json" <<'EOF'
{"schema_version":1,"tag":"v1.0.0","version":"1.0.0","arch":"aarch64-unknown-linux-gnu","binary":"octessera-pi","platforms":["linux-aarch64-device"]}
EOF
ln -sfn "$root/releases/1.0.0" "$root/current"
ln -sfn "$root/current/octessera-pi" "$work/octessera-pi"

make_release() {
  local version="$1" bad_sum="${2:-}"
  VERSION="$version" FIXTURES="$fixtures" python3 - <<'PY'
import json, os, pathlib, stat, zipfile
version = os.environ["VERSION"]
fixtures = pathlib.Path(os.environ["FIXTURES"])
binary = fixtures / "octessera-pi"
binary.write_text("#!/usr/bin/env bash\necho new\n", encoding="utf-8")
manifest = fixtures / "octessera-device-release.json"
manifest.write_text(json.dumps({"schema_version": 1, "tag": "v" + version, "version": version, "arch": "aarch64-unknown-linux-gnu", "binary": "octessera-pi", "platforms": ["linux-aarch64-device"]}), encoding="utf-8")
with zipfile.ZipFile(fixtures / f"octessera-{version}-device-aarch64.zip", "w") as zf:
    info = zipfile.ZipInfo("octessera-pi")
    info.external_attr = (stat.S_IFREG | 0o755) << 16
    zf.writestr(info, binary.read_bytes())
    zf.write(manifest, "octessera-device-release.json")
PY
  if [[ "$bad_sum" == bad ]]; then
    printf '%064d  octessera-%s-device-aarch64.zip\n' 0 "$version" > "$fixtures/SHA256SUMS-device.txt"
  else
    (cd "$fixtures" && sha256sum "octessera-$version-device-aarch64.zip" > SHA256SUMS-device.txt)
  fi
  cat > "$fixtures/release-$version.json" <<EOF
{"tag_name":"v$version","assets":[{"name":"octessera-$version-device-aarch64.zip","browser_download_url":"https://github.com/nexxyz/octessera/releases/download/v$version/octessera-$version-device-aarch64.zip"},{"name":"SHA256SUMS-device.txt","browser_download_url":"https://github.com/nexxyz/octessera/releases/download/v$version/SHA256SUMS-device.txt"}]}
EOF
}

make_release 1.0.1
cat > "$mockbin/curl" <<EOF
#!/usr/bin/env bash
set -euo pipefail
out=""
url=""
while (( \$# )); do
  case "\$1" in
    --output) out="\$2"; shift 2 ;;
    http*) url="\$1"; shift ;;
    *) shift ;;
  esac
done
case "\$url" in
  *api.github.com*/releases/tags/v*) version="\${url##*/v}"; cp "$fixtures/release-\$version.json" "\$out" ;;
  *SHA256SUMS-device.txt) cp "$fixtures/SHA256SUMS-device.txt" "\$out" ;;
  *device-aarch64.zip) file="\${url##*/}"; cp "$fixtures/\$file" "\$out" ;;
  *) echo "unexpected curl URL: \$url" >&2; exit 1 ;;
esac
EOF
chmod 0755 "$mockbin/curl"

env PATH="$mockbin:$PATH" OCTESSERA_UPDATE_ROOT="$root" OCTESSERA_UPDATE_BIN_LINK="$work/octessera-pi" OCTESSERA_UPDATE_LOCK="$work/lock" "$here/octessera-update" apply v1.0.1 >/dev/null
[[ "$(readlink -f "$root/current")" == "$root/releases/1.0.1" ]]
env PATH="$mockbin:$PATH" OCTESSERA_UPDATE_ROOT="$root" OCTESSERA_UPDATE_BIN_LINK="$work/octessera-pi" OCTESSERA_UPDATE_LOCK="$work/lock" "$here/octessera-update" rollback >/dev/null
[[ "$(readlink -f "$root/current")" == "$root/releases/1.0.0" ]]

make_release 1.0.2 bad
if env PATH="$mockbin:$PATH" OCTESSERA_UPDATE_ROOT="$root" OCTESSERA_UPDATE_BIN_LINK="$work/octessera-pi" OCTESSERA_UPDATE_LOCK="$work/lock" "$here/octessera-update" apply v1.0.2 >/dev/null 2>&1; then
  echo "checksum rejection failed" >&2
  exit 1
fi
if env OCTESSERA_UPDATE_ROOT="$root" OCTESSERA_UPDATE_LOCK="$work/lock" "$here/octessera-update" check v1.0.1 extra >/dev/null 2>&1; then
  echo "extra argument rejection failed" >&2
  exit 1
fi

echo "device updater smoke test passed"
