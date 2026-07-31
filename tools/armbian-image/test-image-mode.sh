#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=userpatches/overlay/usr/local/lib/octessera/orange-image-mode.sh
source "$root/userpatches/overlay/usr/local/lib/octessera/orange-image-mode.sh"

octessera_load_image_contract "$root/userpatches/overlay"
[[ "$OCTESSERA_IMAGE_MODE" == diagnostic && "$OCTESSERA_RUNTIME_ENABLED_DEFAULT" == false ]] || {
  echo "The checked-in Orange contract must be diagnostic and runtime-disabled." >&2
  exit 1
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
overlay="$work/overlay"
bundle="$overlay/usr/local/lib/octessera/production-runtime"
mkdir -p "$overlay/etc/octessera" "$overlay/etc/systemd/system" "$bundle"
cp "$root/userpatches/overlay/etc/systemd/system/octessera.service" "$overlay/etc/systemd/system/octessera.service"
cat > "$overlay/etc/octessera/image-contract.json" <<'EOF'
{
  "schema_version": 1,
  "image_kind": "production",
  "runtime_enabled_default": true
}
EOF
printf '\177ELF\002\001\001\000\000\000\000\000\000\000\000\000\002\000\267\000' > "$bundle/octessera-pi"
binary_hash="$(sha256sum "$bundle/octessera-pi" | awk '{ print $1 }')"
cat > "$bundle/octessera-runtime.json" <<EOF
{"name":"octessera-pi","profile":"orange-pi-zero-2w","version":"1.2.3","artifact_kind":"production-runtime","runtime_ready":true,"binary_sha256":"$binary_hash"}
EOF
printf '%s  octessera-pi\n' "$binary_hash" > "$bundle/SHA256SUMS"
octessera_load_image_contract "$overlay"
[[ "$OCTESSERA_IMAGE_MODE" == production && "$OCTESSERA_RUNTIME_ENABLED_DEFAULT" == true && "$OCTESSERA_RUNTIME_VERSION" == 1.2.3 ]] || {
  echo "The production Orange contract did not load its hash-bound bundle." >&2
  exit 1
}

printf '%s\n' unexpected > "$bundle/unexpected"
if octessera_validate_production_bundle "$overlay"; then
  echo "Production runtime accepted an unexpected bundle entry." >&2
  exit 1
fi
rm -f "$bundle/unexpected"
mkfifo "$bundle/unexpected.fifo"
if octessera_validate_production_bundle "$overlay"; then
  echo "Production runtime accepted a special bundle entry." >&2
  exit 1
fi
rm -f "$bundle/unexpected.fifo"
if ln -s octessera-pi "$bundle/unexpected-link" 2>/dev/null; then
  if octessera_validate_production_bundle "$overlay"; then
    echo "Production runtime accepted a symlink bundle entry." >&2
    exit 1
  fi
  rm -f "$bundle/unexpected-link"
fi

printf 'Orange image mode and runtime bundle tests passed\n'
