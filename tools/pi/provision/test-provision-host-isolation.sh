#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
before="$(mktemp)"
trap 'rm -f "$before"' EXIT

for host_path in /etc/octessera /etc/systemd/system/octessera.service \
  /usr/local/sbin/octessera-usb-gadget /opt/octessera \
  /usr/local/lib/octessera/rpi_uart_release.py; do
    if [ -e "$host_path" ] || [ -L "$host_path" ]; then
        printf '%s\n' "$host_path" >> "$before"
    fi
done

bash "$script_dir/test-provision.sh"

for host_path in /etc/octessera /etc/systemd/system/octessera.service \
  /usr/local/sbin/octessera-usb-gadget /opt/octessera \
  /usr/local/lib/octessera/rpi_uart_release.py; do
    if { [ -e "$host_path" ] || [ -L "$host_path" ]; } && ! grep -qxF "$host_path" "$before"; then
        printf 'FAIL: provisioning wrote host path: %s\n' "$host_path" >&2
        exit 1
    fi
done

printf 'PASS: no host paths were written\n'
