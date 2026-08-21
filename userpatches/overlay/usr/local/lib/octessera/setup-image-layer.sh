#!/usr/bin/env bash
set -euo pipefail

overlay_dir="${1:?overlay directory is required}"

install_required() {
  local source_path="$overlay_dir/$1"
  local target_path="$2"
  local mode="$3"
  [[ -f "$source_path" && ! -L "$source_path" ]] || { echo "Missing setup portal overlay: $1" >&2; exit 1; }
  install -D -o root -g root -m "$mode" "$source_path" "$target_path"
}

for setup_file in \
  etc/octessera/setup-profile \
  usr/local/sbin/octessera-wifi-connect \
  usr/local/sbin/octessera-setup-sidecar \
  usr/local/sbin/octessera-setup-request \
  usr/local/sbin/octessera-setup-request-cleanup \
  usr/local/sbin/octessera-setup-start \
  usr/local/sbin/octessera-setup-cleanup \
  usr/local/lib/octessera/setup-status.py \
  usr/local/lib/octessera/setup-status-cli.py \
  usr/local/lib/octessera/setup-call.py \
  etc/systemd/system/octessera-setup.service \
  etc/systemd/system/octessera-setup-request.path \
  etc/systemd/system/octessera-setup-request.service \
  usr/local/share/octessera-setup-ui/app.js \
  usr/local/share/octessera-setup-ui/index.html \
  usr/local/share/octessera-setup-ui/styles.css \
  usr/local/share/octessera-setup-ui/README.md \
  usr/local/share/octessera-setup-ui/octessera-mark.svg \
  usr/local/share/octessera-setup-ui/octessera-wordmark.svg; do
  install_required "$setup_file" "/$setup_file" 0644
done

install_required usr/local/sbin/octessera-wifi-connect /usr/local/sbin/octessera-wifi-connect 0755
install_required usr/local/sbin/octessera-setup-sidecar /usr/local/sbin/octessera-setup-sidecar 0755
install_required usr/local/sbin/octessera-setup-request /usr/local/sbin/octessera-setup-request 0755
install_required usr/local/sbin/octessera-setup-request-cleanup /usr/local/sbin/octessera-setup-request-cleanup 0755
install_required usr/local/sbin/octessera-setup-start /usr/local/sbin/octessera-setup-start 0755
install_required usr/local/sbin/octessera-setup-cleanup /usr/local/sbin/octessera-setup-cleanup 0755
install_required usr/local/lib/octessera/setup-status.py /usr/local/lib/octessera/setup-status.py 0755
install_required usr/local/lib/octessera/setup-status-cli.py /usr/local/lib/octessera/setup-status-cli.py 0644
install_required usr/local/lib/octessera/setup-call.py /usr/local/lib/octessera/setup-call.py 0755
install_required etc/octessera/setup-profile /etc/octessera/setup-profile 0644
install_required etc/systemd/system/octessera-setup.service /etc/systemd/system/octessera-setup.service 0644
install_required etc/systemd/system/octessera-setup-request.path /etc/systemd/system/octessera-setup-request.path 0644
install_required etc/systemd/system/octessera-setup-request.service /etc/systemd/system/octessera-setup-request.service 0644
for setup_ui_file in app.js index.html styles.css README.md octessera-mark.svg octessera-wordmark.svg; do
  install_required "usr/local/share/octessera-setup-ui/$setup_ui_file" "/usr/local/share/octessera-setup-ui/$setup_ui_file" 0644
done

setup_service_link=/etc/systemd/system/multi-user.target.wants/octessera-setup.service
if [[ -L "$setup_service_link" ]]; then
  [[ "$(readlink "$setup_service_link")" == "/etc/systemd/system/octessera-setup.service" ]] || { echo "Unexpected setup service enablement link." >&2; exit 1; }
  rm -f "$setup_service_link"
elif [[ -e "$setup_service_link" ]]; then
  echo "Unexpected setup service enablement path." >&2
  exit 1
fi
