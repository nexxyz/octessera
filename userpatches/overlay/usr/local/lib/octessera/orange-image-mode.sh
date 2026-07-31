#!/usr/bin/env bash

# shellcheck disable=SC2034

octessera_require_regular_file() {
  local path="$1"
  [[ -f "$path" && ! -L "$path" ]] || {
    echo "Expected a regular, non-symlink file: $path" >&2
    return 1
  }
}

octessera_require_exact_bundle_entries() {
  local bundle="$1"
  local entries
  local entry
  entries="$(find -P "$bundle" -mindepth 1 -maxdepth 1 -printf '%f\n' | LC_ALL=C sort)"
  [[ "$entries" == $'SHA256SUMS\noctessera-pi\noctessera-runtime.json' ]] || {
    echo "Production runtime bundle has unexpected entries: $bundle" >&2
    return 1
  }
  while IFS= read -r entry; do
    [[ -n "$entry" ]] || continue
    octessera_require_regular_file "$bundle/$entry" || return 1
  done <<< "$entries"
}

octessera_require_elf64_aarch64() {
  local binary="$1"
  if ! python3 - "$binary" <<'PY'
import sys
from pathlib import Path

header = Path(sys.argv[1]).read_bytes()[:20]
if len(header) != 20 or header[:7] != b"\x7fELF\x02\x01\x01" or header[18:20] != b"\xb7\x00":
    raise SystemExit(1)
PY
  then
    echo "Production runtime is not an ELF64 AArch64 binary: $binary" >&2
    return 1
  fi
}

octessera_validate_production_bundle() {
  local overlay_root="$1"
  local bundle="$overlay_root/usr/local/lib/octessera/production-runtime"
  local metadata="$bundle/octessera-runtime.json"
  local binary="$bundle/octessera-pi"
  local sums="$bundle/SHA256SUMS"
  local metadata_hash
  local manifest
  local manifest_hash
  local actual_hash
  local version
  local parent

  for parent in "$overlay_root/usr" "$overlay_root/usr/local" "$overlay_root/usr/local/lib" "$overlay_root/usr/local/lib/octessera"; do
    [[ -d "$parent" && ! -L "$parent" ]] || {
      echo "Production runtime bundle has an unsafe parent: $parent" >&2
      return 1
    }
  done
  [[ -d "$bundle" && ! -L "$bundle" ]] || {
    echo "Production image requires a staged Orange runtime bundle: $bundle" >&2
    return 1
  }
  octessera_require_exact_bundle_entries "$bundle" || return 1
  jq -e 'type == "object" and ((keys | sort) == ["artifact_kind", "binary_sha256", "name", "profile", "runtime_ready", "version"]) and .name == "octessera-pi" and .profile == "orange-pi-zero-2w" and (.version | type == "string" and test("^[A-Za-z0-9][A-Za-z0-9._+-]{0,63}$")) and .artifact_kind == "production-runtime" and .runtime_ready == true and (.binary_sha256 | type == "string" and test("^[a-f0-9]{64}$"))' "$metadata" >/dev/null || {
    echo "Production runtime metadata is not exact: $metadata" >&2
    return 1
  }
  version="$(jq -r '.version' "$metadata")"
  metadata_hash="$(jq -r '.binary_sha256' "$metadata")"
  manifest="$(cat -- "$sums")"
  [[ "$manifest" =~ ^([a-f0-9]{64})[[:space:]][[:space:]]octessera-pi$ ]] || {
    echo "Production runtime SHA256SUMS must contain one exact binary entry." >&2
    return 1
  }
  manifest_hash="${BASH_REMATCH[1]}"
  actual_hash="$(sha256sum "$binary" | awk '{ print $1 }')"
  [[ "$actual_hash" == "$metadata_hash" && "$actual_hash" == "$manifest_hash" ]] || {
    echo "Production runtime binary hash does not match metadata and SHA256SUMS." >&2
    return 1
  }
  octessera_require_elf64_aarch64 "$binary" || return 1
  OCTESSERA_RUNTIME_VERSION="$version"
  OCTESSERA_RUNTIME_BINARY_SHA256="$actual_hash"
  OCTESSERA_RUNTIME_MANIFEST_SHA256="$(sha256sum "$sums" | awk '{ print $1 }')"
  OCTESSERA_RUNTIME_METADATA_SHA256="$(sha256sum "$metadata" | awk '{ print $1 }')"
}

octessera_require_diagnostic_updater_overlay() {
  local overlay_root="$1"
  local updater_file
  [[ "$OCTESSERA_IMAGE_MODE" == diagnostic ]] || return 0
  for updater_file in \
    usr/local/sbin/octessera-update \
    usr/local/sbin/octessera-update-guard \
    usr/local/sbin/octessera-update-recovery \
    usr/local/lib/octessera/updater_protocol.py \
    usr/local/lib/octessera/updater_state.py \
    usr/local/lib/octessera/updater_assets.py \
    usr/local/lib/octessera/updater_guard.py \
    usr/local/lib/octessera/updater_cli.py \
    etc/systemd/system/octessera-update-guard.service \
    etc/systemd/system/octessera-update-recovery.service; do
    [[ -f "$overlay_root/$updater_file" && ! -L "$overlay_root/$updater_file" ]] || {
      echo "Missing diagnostic updater overlay: $updater_file" >&2
      return 1
    }
  done
}

octessera_configure_runtime_account() {
  local runtime_passwd
  local runtime_group
  local runtime_gid
  local runtime_group_gid
  local runtime_group_name
  local sudoers_file

  reject_runtime_sudoers_file() {
    local candidate="$1"
    if grep -Eq '(^|[^[:alnum:]_-])octessera-runtime([^[:alnum:]_-]|$)' "$candidate"; then
      echo "Production runtime account must not appear in sudoers: $candidate" >&2
      return 1
    fi
  }

  if id octessera-runtime >/dev/null 2>&1; then
    runtime_passwd="$(getent passwd octessera-runtime)"
    IFS=: read -r _ _ _ runtime_gid _ runtime_home runtime_shell <<< "$runtime_passwd"
    [[ "$runtime_home" == /nonexistent && "$runtime_shell" == /usr/sbin/nologin ]] || {
      echo "Existing octessera-runtime account is not a locked system account." >&2
      return 1
    }
    runtime_group="$(getent group octessera-runtime || true)"
    IFS=: read -r _ _ runtime_group_gid _ <<< "$runtime_group"
    [[ -n "$runtime_group" && "${runtime_group%%:*}" == octessera-runtime && "$runtime_gid" == "$runtime_group_gid" ]] || {
      echo "Existing octessera-runtime primary group is invalid." >&2
      return 1
    }
  else
    useradd --system --user-group --home-dir /nonexistent --shell /usr/sbin/nologin octessera-runtime
  fi
  passwd -l octessera-runtime >/dev/null
  for runtime_group_name in audio i2c spi gpio; do
    getent group "$runtime_group_name" >/dev/null || {
      echo "Production runtime requires existing group: $runtime_group_name" >&2
      return 1
    }
  done
  usermod --shell /usr/sbin/nologin --home /nonexistent --groups audio,i2c,spi,gpio octessera-runtime
  if [[ -f /etc/sudoers && ! -L /etc/sudoers ]]; then
    reject_runtime_sudoers_file /etc/sudoers || return 1
  fi
  if [[ -d /etc/sudoers.d && ! -L /etc/sudoers.d ]]; then
    while IFS= read -r -d '' sudoers_file; do
      reject_runtime_sudoers_file "$sudoers_file" || return 1
    done < <(find -P /etc/sudoers.d -type f -print0)
  fi
  install -d -m 0755 -o octessera-runtime -g octessera-runtime /var/lib/octessera/presets /var/lib/octessera/samples
  chown octessera-runtime:octessera-runtime /var/lib/octessera/presets /var/lib/octessera/samples
  chmod 0755 /var/lib/octessera/presets /var/lib/octessera/samples
}

octessera_load_image_contract() {
  local overlay_root="$1"
  local contract="$overlay_root/etc/octessera/image-contract.json"

  octessera_require_regular_file "$contract" || return 1
  command -v jq >/dev/null 2>&1 || {
    echo "jq is required to validate the Orange image contract." >&2
    return 1
  }
  jq -e 'type == "object" and ((keys | sort) == ["image_kind", "runtime_enabled_default", "schema_version"]) and .schema_version == 1 and (.image_kind == "diagnostic" or .image_kind == "production") and (.runtime_enabled_default == (.image_kind == "production"))' "$contract" >/dev/null || {
    echo "Orange image contract must explicitly select diagnostic or production." >&2
    return 1
  }
  OCTESSERA_IMAGE_MODE="$(jq -r '.image_kind' "$contract")"
  OCTESSERA_RUNTIME_ENABLED_DEFAULT="$(jq -r '.runtime_enabled_default | tostring' "$contract")"
  OCTESSERA_IMAGE_CONTRACT_SHA256="$(sha256sum "$contract" | awk '{ print $1 }')"
  export OCTESSERA_IMAGE_MODE OCTESSERA_RUNTIME_ENABLED_DEFAULT OCTESSERA_IMAGE_CONTRACT_SHA256
  if [[ "$OCTESSERA_IMAGE_MODE" == production ]]; then
    [[ -f "$overlay_root/etc/systemd/system/octessera.service" && ! -L "$overlay_root/etc/systemd/system/octessera.service" ]] || {
      echo "Production image contract requires the Orange octessera.service template." >&2
      return 1
    }
    octessera_validate_production_bundle "$overlay_root" || return 1
  elif [[ -e "$overlay_root/usr/local/lib/octessera/production-runtime" || -L "$overlay_root/usr/local/lib/octessera/production-runtime" ]]; then
    echo "Diagnostic image contract must not stage an Orange runtime bundle." >&2
    return 1
  else
    OCTESSERA_RUNTIME_VERSION=none
    OCTESSERA_RUNTIME_BINARY_SHA256=none
    OCTESSERA_RUNTIME_MANIFEST_SHA256=none
    OCTESSERA_RUNTIME_METADATA_SHA256=none
  fi
  export OCTESSERA_RUNTIME_VERSION OCTESSERA_RUNTIME_BINARY_SHA256 OCTESSERA_RUNTIME_MANIFEST_SHA256 OCTESSERA_RUNTIME_METADATA_SHA256
}

octessera_atomic_symlink() {
  local target="$1"
  local destination="$2"
  local temporary="${destination}.tmp.$$"
  [[ ! -e "$destination" && ! -L "$destination" ]] || {
    echo "Refusing to replace an existing managed link: $destination" >&2
    return 1
  }
  rm -f -- "$temporary"
  ln -s -- "$target" "$temporary"
  mv -f -- "$temporary" "$destination"
}

octessera_install_production_runtime() {
  local overlay_root="$1"
  local bundle="$overlay_root/usr/local/lib/octessera/production-runtime"
  local release_root=/opt/octessera/releases
  local release_dir="$release_root/$OCTESSERA_RUNTIME_VERSION"
  local temporary
  local path

  for path in /opt/octessera /opt/octessera/releases /usr/local/bin; do
    [[ ! -L "$path" ]] || {
      echo "Refusing a symlinked runtime install parent: $path" >&2
      return 1
    }
  done
  [[ ! -e "$release_dir" && ! -L "$release_dir" ]] || {
    echo "Refusing to replace an existing Orange runtime release: $release_dir" >&2
    return 1
  }
  [[ ! -e /opt/octessera/current && ! -L /opt/octessera/current ]] || {
    echo "Refusing to replace an existing Orange runtime current link." >&2
    return 1
  }
  [[ ! -e /usr/local/bin/octessera-pi && ! -L /usr/local/bin/octessera-pi ]] || {
    echo "Refusing to replace an existing Orange runtime binary link." >&2
    return 1
  }
  install -d -m 0755 -o root -g root /opt/octessera "$release_root" /usr/local/bin || return 1
  temporary="$(mktemp -d "$release_root/.${OCTESSERA_RUNTIME_VERSION}.XXXXXX")" || return 1
  if ! install -m 0555 -o root -g root "$bundle/octessera-pi" "$temporary/octessera-pi" || \
    ! install -m 0444 -o root -g root "$bundle/octessera-runtime.json" "$temporary/octessera-runtime.json" || \
    ! install -m 0444 -o root -g root "$bundle/SHA256SUMS" "$temporary/SHA256SUMS" || \
    ! chmod 0555 "$temporary" || ! mv -f -- "$temporary" "$release_dir"; then
    rm -rf -- "$temporary"
    return 1
  fi
  octessera_atomic_symlink "$release_dir" /opt/octessera/current || return 1
  octessera_atomic_symlink /opt/octessera/current/octessera-pi /usr/local/bin/octessera-pi || return 1
}
