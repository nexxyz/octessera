#!/usr/bin/env bash

octessera_resolve_boot_dtb() {
  local image_root="${1:-/}"
  local target_name=sun50i-h618-orangepi-zero2w.dtb
  local target_relative=allwinner/$target_name
  local boot_dir
  local fdtfile=
  local extlinux_fdt=
  local boot_reference
  local candidate
  local resolved
  local fdtfile_status=0
  local extlinux_status=0
  local -a candidates=()

  image_root="${image_root%/}"
  boot_dir="$image_root/boot"
  [[ -f "$boot_dir/armbianEnv.txt" ]] || { echo "Missing Armbian boot configuration." >&2; return 1; }

  fdtfile="$(awk '
    /^[[:space:]]*#/ {
      if ($0 ~ /(^|[^_[:alnum:]])fdtfile[[:space:]]*=/) bad = 1
      next
    }
    /^fdtfile=/ {
      count++
      value = substr($0, length("fdtfile=") + 1)
      if (value !~ /^[A-Za-z0-9._/-]+$/ || value ~ /(^|\/)\.\.?($|\/)/) bad = 1
      if (count == 1) print value
      next
    }
    /(^|[^_[:alnum:]])fdtfile[[:space:]]*=/ { bad = 1 }
    END { if (bad || count > 1) exit 2 }
  ' "$boot_dir/armbianEnv.txt")" || fdtfile_status=$?
  [[ "$fdtfile_status" == 0 ]] || { echo "Malformed or ambiguous fdtfile in $boot_dir/armbianEnv.txt." >&2; return 1; }

  if [[ -f "$boot_dir/extlinux/extlinux.conf" ]]; then
    extlinux_fdt="$(awk '
      /^[[:space:]]*#/ { next }
      /^[[:space:]]*[Ff][Dd][Tt][[:space:]]+/ {
        count++
        value = $2
        if (value !~ /^\/?[A-Za-z0-9._/-]+$/ || value ~ /(^|\/)\.\.?($|\/)/) bad = 1
        if (count == 1) print value
      }
      END { if (bad || count > 1) exit 2 }
    ' "$boot_dir/extlinux/extlinux.conf")" || extlinux_status=$?
    [[ "$extlinux_status" == 0 ]] || { echo "Malformed or ambiguous FDT entry in $boot_dir/extlinux/extlinux.conf." >&2; return 1; }
  fi

  add_candidate() {
    local candidate_path="$1"
    local candidate_resolved
    [[ -f "$candidate_path" ]] || return 0
    candidate_resolved="$(readlink -f -- "$candidate_path" 2>/dev/null || true)"
    case "$candidate_resolved" in
      "$image_root"/boot/dtb-*/allwinner/$target_name|"$image_root"/usr/lib/linux-image-*/allwinner/$target_name) ;;
      *)
        echo "Unexpected H618 DTB candidate: $candidate_path." >&2
        return 1
        ;;
    esac
    for resolved in "${candidates[@]}"; do
      [[ "$resolved" != "$candidate_resolved" ]] || return 0
    done
    candidates+=("$candidate_resolved")
  }

  add_reference() {
    local reference="$1"
    local before="${#candidates[@]}"
    local relative_reference
    if [[ "$reference" == /boot/* || "$reference" == /usr/lib/* ]]; then
      add_candidate "$image_root$reference" || return 1
    elif [[ "$reference" == /dtb/* ]]; then
      add_candidate "$boot_dir$reference" || return 1
    elif [[ "$reference" == /allwinner/* ]]; then
      add_candidate "$boot_dir/dtb$reference" || return 1
      if [[ "${#candidates[@]}" == "$before" ]]; then
        add_candidate "$boot_dir$reference" || return 1
      fi
    else
      relative_reference="${reference#/}"
      if [[ "$relative_reference" == allwinner/* ]]; then
        add_candidate "$boot_dir/dtb/$relative_reference" || return 1
        if [[ "${#candidates[@]}" == "$before" ]]; then
          add_candidate "$boot_dir/$relative_reference" || return 1
        fi
      elif [[ "$relative_reference" == dtb/* ]]; then
        add_candidate "$boot_dir/$relative_reference" || return 1
      elif [[ "$relative_reference" == "$target_name" ]]; then
        add_candidate "$boot_dir/dtb/$target_relative" || return 1
      else
        echo "Unsupported boot DTB reference: $reference." >&2
        return 1
      fi
    fi
    if [[ "${#candidates[@]}" == "$before" ]]; then
      for candidate in "$image_root"/boot/dtb-*/$target_relative "$image_root"/usr/lib/linux-image-*/$target_relative; do
        add_candidate "$candidate" || return 1
      done
    fi
    [[ "${#candidates[@]}" != "$before" ]] || {
      echo "Boot DTB reference does not resolve: $reference." >&2
      return 1
    }
  }

  if [[ -n "$fdtfile" ]]; then
    [[ "$(basename "$fdtfile")" == "$target_name" ]] || {
      echo "fdtfile does not select the H618 Orange Pi Zero 2W DTB." >&2
      return 1
    }
    add_reference "$fdtfile" || return 1
  fi
  if [[ -n "$extlinux_fdt" ]]; then
    [[ "$(basename "$extlinux_fdt")" == "$target_name" ]] || {
      echo "Extlinux FDT does not select the H618 Orange Pi Zero 2W DTB." >&2
      return 1
    }
    add_reference "$extlinux_fdt" || return 1
  fi

  if [[ -z "$fdtfile" && -z "$extlinux_fdt" ]]; then
    if [[ -d "$boot_dir/dtb" ]]; then
      add_candidate "$boot_dir/dtb/$target_relative" || return 1
    fi
    if [[ "${#candidates[@]}" == 0 ]]; then
      for candidate in "$image_root"/boot/dtb-*/$target_relative "$image_root"/usr/lib/linux-image-*/$target_relative; do
        add_candidate "$candidate" || return 1
      done
    fi
  fi
  [[ "${#candidates[@]}" -gt 0 ]] || {
    echo "No boot-selected H618 Orange Pi Zero 2W DTB found." >&2
    return 1
  }

  local preferred=
  for candidate in "${candidates[@]}"; do
    if [[ "$candidate" == "$image_root"/boot/dtb-*/* ]]; then
      preferred="$candidate"
      break
    fi
  done
  preferred="${preferred:-${candidates[0]}}"
  for candidate in "${candidates[@]}"; do
    cmp -s "$preferred" "$candidate" || {
      echo "Conflicting H618 Orange Pi Zero 2W DTB copies." >&2
      return 1
    }
  done
  printf '%s\n' "$preferred"
}
