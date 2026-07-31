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

(
  declare -A fake_groups=([audio]='audio:x:29:' [i2c]='i2c:x:100:' [gpio]='gpio:x:997:')
  groupadd_calls=()
  runtime_groups=
  getent() {
    [[ "$1" == group && -n "${fake_groups[$2]+present}" ]] || return 1
    printf '%s\n' "${fake_groups[$2]}"
  }
  groupadd() {
    [[ "$1" == --system ]] || return 1
    fake_groups[$2]="$2:x:998:"
    groupadd_calls+=("$2")
  }
  usermod() { runtime_groups="$*"; }
  octessera_configure_runtime_hardware_groups
  [[ "${groupadd_calls[*]}" == spi && "$runtime_groups" == '--groups audio,i2c,spi,gpio octessera-runtime' ]] || {
    echo 'Missing Orange hardware groups were not created or assigned exactly.' >&2
    exit 1
  }
)
(
  declare -A fake_groups=([audio]='audio:x:29:' [i2c]='i2c:x:100:' [spi]='spi:x:not-a-gid:' [gpio]='gpio:x:997:')
  getent() {
    [[ "$1" == group && -n "${fake_groups[$2]+present}" ]] || return 1
    printf '%s\n' "${fake_groups[$2]}"
  }
  usermod() { :; }
  if octessera_configure_runtime_hardware_groups; then
    echo 'Malformed existing Orange hardware group was accepted.' >&2
    exit 1
  fi
)

printf 'Orange image mode and runtime bundle tests passed\n'
