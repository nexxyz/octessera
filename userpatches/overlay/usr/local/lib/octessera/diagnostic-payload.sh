#!/usr/bin/env bash

octessera_install_diagnostic_payload() {
  local payload_url="$1"
  local payload_sha256="$2"
  local work
  local entry

  [[ -n "$payload_url" ]] || return 0
  [[ "$payload_url" == https://* ]] || { echo "OCTESSERA_PAYLOAD_URL must use HTTPS." >&2; return 1; }
  [[ "$payload_sha256" =~ ^[a-fA-F0-9]{64}$ ]] || { echo "OCTESSERA_PAYLOAD_SHA256 is required." >&2; return 1; }
  work="$(mktemp -d)"
  curl --fail --location --proto '=https' --tlsv1.2 --output "$work/payload.tar" "$payload_url"
  echo "$payload_sha256  $work/payload.tar" | sha256sum -c -
  while IFS= read -r entry; do
    case "$entry" in
      /*|..|../*|*/..|*/../*) echo "Unsafe payload path: $entry" >&2; return 1 ;;
    esac
  done < <(tar -tf "$work/payload.tar")
  while IFS= read -r entry; do
    case "${entry:0:1}" in
      l|h|c|b|p|s) echo "Unsafe payload entry type: $entry" >&2; return 1 ;;
    esac
  done < <(tar -tvf "$work/payload.tar")
  mkdir "$work/extract"
  tar -xf "$work/payload.tar" -C "$work/extract" --no-same-owner --no-same-permissions
  [[ -f "$work/extract/octessera-payload.json" && ! -L "$work/extract/octessera-payload.json" ]] || {
    echo "Payload is missing octessera-payload.json." >&2
    return 1
  }
  jq -e '.name == "octessera-armbian-payload" and .artifact_kind == "diagnostic-only" and .runtime_ready == false and (.enable_runtime // false) == false' "$work/extract/octessera-payload.json" >/dev/null || {
    echo "Orange Pi payloads must be explicitly diagnostic-only and runtime-disabled." >&2
    return 1
  }
  if find -P "$work/extract" -type f -name octessera-pi -print -quit | grep -q .; then
    echo "Orange Pi diagnostic images reject octessera-pi runtime payloads." >&2
    return 1
  fi
  install -D -m 0644 "$work/extract/octessera-payload.json" /etc/octessera/payload.json
  install -d -m 0755 /usr/local/lib/octessera/payload-staged
  cp -a "$work/extract/." /usr/local/lib/octessera/payload-staged/
  rm -rf -- "$work"
}
