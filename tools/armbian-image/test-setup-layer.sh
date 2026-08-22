#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$root/tools/armbian-image/validation-assertions.sh"
orange="$root/userpatches/overlay"
raspberry="$root/tools/pi-image/stage4-octessera/files/root"

check_common() {
  local tree="$1"
  local user="$2"
  local profile="$3"
  local owner="$4"
  local sidecar="$tree/usr/local/sbin/octessera-setup-sidecar"
  local wrapper="$tree/usr/local/sbin/octessera-wifi-connect"
  local helper="$tree/usr/local/sbin/octessera-setup-request"
  local status="$tree/usr/local/lib/octessera/setup-status.py"
  local setup_unit="$tree/etc/systemd/system/octessera-setup.service"
  local request_unit="$tree/etc/systemd/system/octessera-setup-request.service"
  for path in \
    "$tree/etc/octessera/setup-profile" \
    "$tree/etc/default/locale" \
    "$wrapper" \
    "$sidecar" \
    "$helper" \
    "$tree/usr/local/sbin/octessera-setup-start" \
    "$tree/usr/local/sbin/octessera-setup-cleanup" \
    "$tree/usr/local/sbin/octessera-setup-request-cleanup" \
    "$status" \
    "$tree/usr/local/lib/octessera/setup-status-cli.py" \
    "$tree/usr/local/lib/octessera/setup-call.py" \
    "$setup_unit" \
    "$tree/etc/systemd/system/octessera-setup-request.path" \
    "$request_unit" \
    "$tree/usr/local/share/octessera-setup-ui/index.html" \
    "$tree/usr/local/share/octessera-setup-ui/app.js" \
    "$tree/usr/local/share/octessera-setup-ui/styles.css"; do
    [[ -f "$path" && ! -L "$path" ]] || { echo "Missing setup layer file: $path" >&2; exit 1; }
  done
  grep -qxF "$profile" "$tree/etc/octessera/setup-profile"
  grep -qxF 'LANG=C.UTF-8' "$tree/etc/default/locale"
  grep -qxF 'LANGUAGE=en' "$tree/etc/default/locale"
  grep -qxF 'LC_MESSAGES=C.UTF-8' "$tree/etc/default/locale"
  grep -qF 'ALLOWED_ORIGINS = frozenset(("http://192.168.42.1", "http://192.168.42.1:80"))' "$sidecar"
  grep -qF 'ipaddress.ip_network("192.168.42.0/24")' "$sidecar"
  grep -qF 'MAX_BODY = 16384' "$sidecar"
  grep -qF 'Content-Length' "$sidecar"
  grep -qF 'Transfer-Encoding' "$sidecar"
  grep -qF 'content_type != "application/json"' "$sidecar"
  grep -qF 'HTTPServer(("0.0.0.0", 8080), Handler)' "$sidecar"
  grep -qF 'settimeout(10)' "$sidecar"
  grep -qF 'READINESS_PATH' "$sidecar"
  grep -qF 'PUBLIC_DIR = "/run/octessera-setup-status"' "$status"
  grep -qF 'RECEIPT_DIR' "$status"
  grep -qF 'fcntl.flock' "$status"
  grep -qF 'ATTEMPT_RE = re.compile(r"^[0-9a-f]{32}$")' "$status"
  grep -qF 'BOOT_RE = re.compile' "$status"
  octessera_reject_file_match 'Setup sidecar contains an unapproved server, environment, or privilege path.' -Eq 'ThreadingHTTPServer|os\.environ|setup-force|sudoers' "$sidecar"
  grep -qF "USER = \"$user\"" "$sidecar"
  grep -qF "REQUEST_OWNER = \"$owner\"" "$helper"
  grep -qF "PROFILE = \"$profile\"" "$helper"
  grep -qF 'metadata.st_size == 33' "$helper"
  grep -qF 'os.rename(REQUEST_PATH, claim_path)' "$helper"
  octessera_reject_file_match 'Setup request helper must not use hard links.' -qF 'os.link(' "$helper"
  grep -qF 'start-or-attach' "$helper"
  grep -qF 'REQUEST_PATH = "/run/octessera/setup-portal.request"' "$helper"
  grep -qF 'CONTROL_DIR = "/run/octessera-setup-control"' "$helper"
  grep -qF '["systemctl", "start", SETUP_UNIT]' "$helper"
  octessera_reject_file_match 'Setup request helper contains an unapproved restart or shell fallback.' -Eq 'systemctl.*restart|list-unit-files|shell=True|setup-force' "$helper"
  grep -qF 'interface=wlan0' "$wrapper"
  # shellcheck disable=SC2016
  grep -qF '/sys/class/net/$interface/address' "$wrapper"
  grep -qF 'remaining_seconds' "$wrapper"
  octessera_reject_file_match 'Setup Wi-Fi wrapper contains setup or privilege state.' -Eq 'attempt_id|token_hex|OCTESSERA_SETUP|setup-force|systemctl|sudo' "$wrapper"
  grep -qFx 'PathExists=/run/octessera/setup-portal.request' "$tree/etc/systemd/system/octessera-setup-request.path"
  grep -qFx 'Unit=octessera-setup-request.service' "$tree/etc/systemd/system/octessera-setup-request.path"
  grep -qFx 'User=root' "$tree/etc/systemd/system/octessera-setup-request.service"
  grep -qFx 'Group=root' "$tree/etc/systemd/system/octessera-setup-request.service"
  grep -qFx 'ExecStart=/usr/local/sbin/octessera-setup-request' "$tree/etc/systemd/system/octessera-setup-request.service"
  grep -qFx 'ExecStopPost=/usr/local/sbin/octessera-setup-request-cleanup' "$tree/etc/systemd/system/octessera-setup-request.service"
  grep -qFx 'RuntimeDirectory=octessera-setup' "$setup_unit"
  grep -qF ' -/run/octessera-setup-control -/run/octessera-setup-status' "$setup_unit"
  grep -qFx 'RuntimeDirectoryMode=0700' "$setup_unit"
  grep -qFx 'RuntimeMaxSec=1800s' "$setup_unit"
  grep -qFx 'ExecStartPre=/usr/local/sbin/octessera-setup-start' "$setup_unit"
  grep -qFx 'ExecStopPost=/usr/local/sbin/octessera-setup-cleanup' "$setup_unit"
  grep -qFx 'TimeoutStopSec=10s' "$setup_unit"
  grep -qFx 'KillMode=control-group' "$setup_unit"
  grep -qFx 'NoNewPrivileges=yes' "$setup_unit"
  grep -qFx 'PrivateTmp=yes' "$setup_unit"
  grep -qFx 'ProtectKernelModules=yes' "$setup_unit"
  grep -qFx 'ProtectControlGroups=yes' "$setup_unit"
  grep -qFx 'ProtectKernelLogs=yes' "$setup_unit"
  grep -qFx 'RestrictNamespaces=yes' "$setup_unit"
  grep -qFx 'LockPersonality=yes' "$setup_unit"
  grep -qFx 'RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6' "$setup_unit"
  octessera_reject_file_match 'Setup service must not impose a startup timeout.' -q '^TimeoutStartSec=' "$setup_unit"
  for line in UMask=0077 PrivateTmp=yes ProtectKernelModules=yes ProtectControlGroups=yes ProtectKernelLogs=yes RestrictNamespaces=yes LockPersonality=yes RestrictAddressFamilies=AF_UNIX; do
    grep -qFx "$line" "$request_unit"
  done
  grep -qF 'sshPublicKey' "$tree/usr/local/share/octessera-setup-ui/app.js"
  grep -qF 'sshPasswordConfirm' "$tree/usr/local/share/octessera-setup-ui/app.js"
  for helper in octessera-setup-start octessera-setup-cleanup octessera-setup-request-cleanup; do
    bash -n "$tree/usr/local/sbin/$helper"
  done
}

check_common "$orange" octessera orange-pi-zero-2w octessera-runtime
check_common "$raspberry" pi raspberry-pi-zero-2w pi

sudoers_fixture="$(mktemp -d)"
sudoers_pattern='^[[:space:]]*[^#]*\bNOPASSWD[[:space:]]*:[[:space:]]*ALL([[:space:]]|$)'
printf '%s\n' 'octessera ALL=(root) NOPASSWD: /sbin/shutdown' > "$sudoers_fixture/sudoers"
if grep -Eiq "$sudoers_pattern" "$sudoers_fixture/sudoers"; then exit 1; fi
printf '%s\n' 'octessera ALL=(ALL) NOPASSWD: ALL' > "$sudoers_fixture/sudoers"
grep -Eiq "$sudoers_pattern" "$sudoers_fixture/sudoers"
mkdir "$sudoers_fixture/sudoers.d"
printf '%s\n' 'octessera ALL=(root) NOPASSWD: /usr/local/sbin/octessera-update' > "$sudoers_fixture/sudoers.d/octessera-update"
if grep -Eiq "$sudoers_pattern" "$sudoers_fixture/sudoers.d/octessera-update"; then exit 1; fi
printf '%s\n' 'octessera ALL=(ALL) NOPASSWD: ALL' > "$sudoers_fixture/sudoers.d/negative"
grep -Eiq "$sudoers_pattern" "$sudoers_fixture/sudoers.d/negative"
rm -rf "$sudoers_fixture"

locale_fixture="$(mktemp)"
locale_pattern='^[[:space:]]*(export[[:space:]]+)?(LANG|LANGUAGE|LC_[[:alnum:]_]+)[[:space:]]*='
printf '%s\n' 'export LC_MESSAGES=de_DE.UTF-8' 'LC_TIME=de_DE.UTF-8' > "$locale_fixture"
grep -Eq "$locale_pattern" "$locale_fixture"
printf '%s\n' 'export LC_ALL=de_DE.UTF-8' > "$locale_fixture"
grep -Eq "$locale_pattern" "$locale_fixture"
rm -f "$locale_fixture"

raspberry_setup_assets=(
  tools/pi-image/stage4-octessera/files/root/etc/default/locale
  tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-wifi-connect
  tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup-sidecar
  tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup-request
  tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup-request-cleanup
  tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup-start
  tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-setup-cleanup
  tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/setup-status.py
  tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/setup-status-cli.py
  tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/setup-call.py
  tools/pi-image/stage4-octessera/files/root/usr/local/share/octessera-setup-ui/app.js
  tools/pi-image/stage4-octessera/files/root/usr/local/share/octessera-setup-ui/index.html
  tools/pi-image/stage4-octessera/files/root/usr/local/share/octessera-setup-ui/styles.css
  tools/pi-image/stage4-octessera/files/root/usr/local/share/octessera-setup-ui/README.md
  tools/pi-image/stage4-octessera/files/root/usr/local/share/octessera-setup-ui/octessera-mark.svg
  tools/pi-image/stage4-octessera/files/root/usr/local/share/octessera-setup-ui/octessera-wordmark.svg
)
for path in "${raspberry_setup_assets[@]}"; do
  [[ -f "$root/$path" && ! -L "$root/$path" ]] || { echo "Missing Raspberry setup source: $path" >&2; exit 1; }
done
if [[ -d "$root/.git" ]]; then
  for path in "${raspberry_setup_assets[@]}"; do
    if git -c safe.directory="$root" check-ignore --no-index -q -- "$root/$path"; then
      echo "Raspberry setup source is ignored: $path" >&2
      exit 1
    fi
  done
  for path in \
    tools/pi-image/stage4-octessera/files/root/usr/local/bin/octessera-network-health \
    tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-update; do
    git -c safe.directory="$root" check-ignore --no-index -q -- "$root/$path" || {
      echo "Unrelated Raspberry staged output is no longer ignored: $path" >&2
      exit 1
    }
  done
fi

grep -qF 'openssh-server' "$root/tools/pi-image/stage4-octessera/00-install-deps/00-run-chroot.sh"
grep -qF 'openssh-server' "$root/userpatches/customize-image.sh"
grep -qF 'install -D -o root -g root -m 0755' "$root/tools/pi-image/stage4-octessera/02-setup-service/00-run.sh"
grep -qF 'octessera-setup-request.path' "$root/tools/pi-image/stage4-octessera/02-setup-service/00-run.sh"
grep -qF 'octessera-setup-request.path' "$root/userpatches/customize-image.sh"
grep -qF 'systemctl enable octessera-setup-request.path' "$root/userpatches/customize-image.sh"
for unit in dnsmasq.service systemd-networkd-wait-online.service NetworkManager-wait-online.service; do
  grep -qF "$unit" "$root/userpatches/customize-image.sh"
  grep -qF "$unit" "$root/tools/pi-image/stage4-octessera/02-setup-service/00-run.sh"
done
grep -qF '(LANG|LANGUAGE|LC_[[:alnum:]_]+)' "$root/userpatches/customize-image.sh"
grep -qF '(LANG|LANGUAGE|LC_[[:alnum:]_]+)' "$root/tools/pi-image/stage4-octessera/02-setup-service/00-run.sh"
grep -qF 'RuntimeDirectory=octessera-setup' "$root/userpatches/overlay/etc/systemd/system/octessera-setup.service"
grep -qF 'RuntimeDirectory=octessera-setup' "$root/tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera-setup.service"
grep -qF ' -/run/octessera-setup-control -/run/octessera-setup-status' "$root/userpatches/overlay/etc/systemd/system/octessera-setup.service"
grep -qF ' -/run/octessera-setup-control -/run/octessera-setup-status' "$root/tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera-setup.service"
octessera_reject_file_match 'Orange construction must not enable the interactive setup service.' -qF 'systemctl enable octessera-setup.service' "$root/userpatches/customize-image.sh"
grep -qF 'setup_service_link=/etc/systemd/system/multi-user.target.wants/octessera-setup.service' "$root/userpatches/overlay/usr/local/lib/octessera/setup-image-layer.sh"
grep -qF "rm -f \"\$setup_service_link\"" "$root/userpatches/overlay/usr/local/lib/octessera/setup-image-layer.sh"
octessera_reject_file_match 'Raspberry setup must not enable the interactive setup service at image construction time.' -Eq 'enable.*octessera-setup\.service|ln -s[f]?.*multi-user\.target\.wants/octessera-setup\.service' "$root/tools/pi-image/stage4-octessera/02-setup-service/00-run.sh"
grep -qF 'octessera-setup-request.path' "$root/tools/pi-image/stage4-octessera/02-setup-service/00-run.sh"
grep -qF 'setup-finalize-failed' "$root/userpatches/customize-image.sh"
grep -qF 'setup-finalize-failed' "$root/tools/pi-image/stage4-octessera/04-sanitize-release-image/00-run.sh"
grep -qF 'setup-image-layer.sh' "$root/userpatches/customize-image.sh"
grep -qF 'install -D -o root -g root' "$root/userpatches/overlay/usr/local/lib/octessera/setup-image-layer.sh"
grep -qF -- '--verification-profile legacy-setup-layer' "$root/resources/image-mutations/orange-pi-zero-2w-setup.json"
grep -qF -- '--verification-profile legacy-setup-layer' "$root/resources/image-mutations/raspberry-pi-zero-2w-setup.json"
grep -qF -- '--verification-profile full-constructor|legacy-runtime-only|legacy-setup-layer' "$root/tools/armbian-image/inspect-built-image.sh"
grep -qF -- '--verification-profile full-constructor|legacy-runtime-only|legacy-setup-layer' "$root/tools/pi-image/verify-sanitized-image.sh"
grep -qF 'require_orange_constructor_policy' "$root/tools/armbian-image/setup-layer-proof.sh"
grep -qF 'require_raspberry_constructor_policy' "$root/tools/pi-image/verify-sanitized-image.sh"
grep -qF 'constructor_policy_required=true' "$root/tools/armbian-image/inspect-built-image.sh"
grep -qF 'setup_layer_required=true' "$root/tools/armbian-image/inspect-built-image.sh"
grep -qF 'CONSTRUCTOR_POLICY_REQUIRED=true' "$root/tools/pi-image/verify-sanitized-image.sh"
grep -qF 'SETUP_LAYER_REQUIRED=true' "$root/tools/pi-image/verify-sanitized-image.sh"
grep -qF 'octessera_remove_raspberry_parent_sudoers' "$root/tools/pi-image/stage4-octessera/02-setup-service/00-run.sh"
if bash "$root/tools/armbian-image/inspect-built-image.sh" fixture >/dev/null 2>&1; then
  echo 'Orange image inspection accepted a missing verification profile.' >&2
  exit 1
fi
if bash "$root/tools/armbian-image/inspect-built-image.sh" --verification-profile invalid fixture >/dev/null 2>&1; then
  echo 'Orange image inspection accepted an invalid verification profile.' >&2
  exit 1
fi
if bash "$root/tools/armbian-image/inspect-built-image.sh" --verification-profile legacy-runtime-only --verification-profile legacy-setup-layer fixture >/dev/null 2>&1; then
  echo 'Orange image inspection accepted conflicting verification profiles.' >&2
  exit 1
fi
if bash "$root/tools/pi-image/verify-sanitized-image.sh" fixture >/dev/null 2>&1; then
  echo 'Raspberry image inspection accepted a missing verification profile.' >&2
  exit 1
fi
if bash "$root/tools/pi-image/verify-sanitized-image.sh" --verification-profile invalid fixture >/dev/null 2>&1; then
  echo 'Raspberry image inspection accepted an invalid verification profile.' >&2
  exit 1
fi
if bash "$root/tools/pi-image/verify-sanitized-image.sh" --verification-profile legacy-runtime-only --verification-profile legacy-setup-layer fixture >/dev/null 2>&1; then
  echo 'Raspberry image inspection accepted conflicting verification profiles.' >&2
  exit 1
fi
if bash "$root/tools/armbian-image/inspect-output-images.sh" --mode diagnostic fixture >/dev/null 2>&1; then
  echo 'Armbian output inspection accepted a missing verification profile.' >&2
  exit 1
fi
if bash "$root/tools/armbian-image/inspect-output-images.sh" --verification-profile invalid --mode diagnostic fixture >/dev/null 2>&1; then
  echo 'Armbian output inspection accepted an invalid verification profile.' >&2
  exit 1
fi
grep -qF 'Orange setup path must be absent' "$root/tools/armbian-image/setup-layer-proof.sh"
grep -qF 'octessera_require_image_symlink etc/systemd/system/multi-user.target.wants/octessera-setup-request.path ../octessera-setup-request.path' "$root/tools/armbian-image/setup-layer-proof.sh"
bash -n "$root/userpatches/overlay/usr/local/lib/octessera/setup-image-layer.sh"

# shellcheck source=tools/armbian-image/inspect-runtime.sh
source "$root/tools/armbian-image/inspect-runtime.sh"
setup_link_root="$(mktemp -d)"
trap 'rm -rf "$setup_link_root"' EXIT
mkdir -p "$setup_link_root/etc/systemd/system/multi-user.target.wants"
target="$setup_link_root"
stat_path() { [[ -e "$target/$1" || -L "$target/$1" ]]; }
setup_link_path=etc/systemd/system/multi-user.target.wants/octessera-setup-request.path
ln -s ../octessera-setup-request.path "$setup_link_root/$setup_link_path"
octessera_require_image_symlink "$setup_link_path" ../octessera-setup-request.path
rm "$setup_link_root/$setup_link_path"
printf '%s\n' wrong > "$setup_link_root/$setup_link_path"
if (octessera_require_image_symlink "$setup_link_path" ../octessera-setup-request.path); then
  echo 'Setup request path proof accepted a regular file.' >&2
  exit 1
fi
rm "$setup_link_root/$setup_link_path"
ln -s ../wrong-request.path "$setup_link_root/$setup_link_path"
if (octessera_require_image_symlink "$setup_link_path" ../octessera-setup-request.path); then
  echo 'Setup request path proof accepted a wrong symlink target.' >&2
  exit 1
fi

for path in \
  "$root/tools/pi-image/stage4-octessera/00-install-deps/00-run-chroot.sh" \
  "$root/tools/pi-image/stage4-octessera/02-setup-service/00-run.sh" \
  "$root/tools/pi-image/stage4-octessera/04-sanitize-release-image/00-run.sh" \
  "$root/userpatches/customize-image.sh"; do
  bash -n "$path"
done

if command -v systemd-analyze >/dev/null 2>&1; then
  for tree in "$orange" "$raspberry"; do
    unit_root="$(mktemp -d)"
    mkdir -p "$unit_root/etc/systemd/system" "$unit_root/usr/local/sbin" "$unit_root/usr/local/lib/octessera"
    cp "$tree/etc/systemd/system/octessera-setup.service" "$unit_root/etc/systemd/system/"
    cp "$tree/etc/systemd/system/octessera-setup-request.path" "$unit_root/etc/systemd/system/"
    cp "$tree/etc/systemd/system/octessera-setup-request.service" "$unit_root/etc/systemd/system/"
    chmod 0644 "$unit_root/etc/systemd/system/"*.service "$unit_root/etc/systemd/system/"*.path
    for unit in NetworkManager.service local-fs.target multi-user.target; do
      printf '%s\n' '[Unit]' "Description=$unit" > "$unit_root/etc/systemd/system/$unit"
    done
    printf '%s\n' '[Unit]' 'Description=NetworkManager' '[Service]' 'Type=oneshot' 'ExecStart=/bin/true' > "$unit_root/etc/systemd/system/NetworkManager.service"
    for unit in sysinit.target basic.target; do
      printf '%s\n' '[Unit]' "Description=$unit" > "$unit_root/etc/systemd/system/$unit"
    done
    mkdir -p "$unit_root/bin"
    printf '%s\n' '#!/bin/sh' 'exit 0' > "$unit_root/bin/true"
    chmod 0755 "$unit_root/bin/true"
    printf '%s\n' '#!/bin/sh' 'exit 0' > "$unit_root/usr/local/sbin/octessera-wifi-connect"
    for helper in octessera-wifi-connect octessera-setup-request octessera-setup-start octessera-setup-cleanup octessera-setup-request-cleanup; do
      printf '%s\n' '#!/bin/sh' 'exit 0' > "$unit_root/usr/local/sbin/$helper"
    done
    chmod 0755 "$unit_root/usr/local/sbin/"*
    systemd-analyze --root="$unit_root" verify octessera-setup.service octessera-setup-request.path octessera-setup-request.service
    rm -rf "$unit_root"
  done
else
  echo "systemd-analyze unavailable; setup unit static checks passed"
fi

echo "Setup image-source layer static tests passed"
