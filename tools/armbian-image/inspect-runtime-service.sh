#!/usr/bin/env bash
# shellcheck disable=SC2154

octessera_require_runtime_elf() {
  local path="$1" binary_path
  if [[ -d "$target" ]]; then binary_path="$target/$path"; else binary_path="$inspect_work/octessera-pi"; rm -f -- "$binary_path"; debugfs -R "$(octessera_debugfs_dump_request "$path" "$binary_path")" "$target" >/dev/null 2>"$inspect_work/runtime-elf.stderr" || { cat -- "$inspect_work/runtime-elf.stderr" >&2; echo "Unable to inspect runtime binary: $path." >&2; exit 1; }; fi
  if python3 - "$binary_path" <<'PY'
import sys
from pathlib import Path
header = Path(sys.argv[1]).read_bytes()[:20]
if len(header) != 20 or header[:7] != b"\x7fELF\x02\x01\x01" or header[18:20] != b"\xb7\x00":
    raise SystemExit(1)
PY
  then
    :
  else
    echo "Runtime binary is not ELF64 AArch64: $path." >&2
    exit 1
  fi
}

octessera_require_runtime_service() {
  local service_content="$1" required_line line
  for required_line in \
    'StartLimitIntervalSec=30s' 'StartLimitBurst=3' \
    'After=octessera-provision-musical-default.service octessera-orange-usb-gadget.service sound.target' \
    'Requires=octessera-device-apply-reboot.socket' 'Requires=octessera-provision-musical-default.service' \
    'After=octessera-device-apply-reboot.socket' 'User=octessera-runtime' 'Group=octessera-runtime' \
    'Environment=OCTESSERA_EXPECTED_BOARD_PROFILE=orange-pi-zero-2w' \
    'Environment=OCTESSERA_PI_STORE_DIR=/var/lib/octessera/presets' \
    'Environment=OCTESSERA_PI_SAMPLES_DIR=/var/lib/octessera/samples' \
    'Environment=OCTESSERA_CANDIDATE_HEALTH_PATH=/run/octessera/candidate-ready.json' \
    'Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1' 'RuntimeDirectory=octessera' 'RuntimeDirectoryMode=0755' \
    'NoNewPrivileges=yes' 'ProtectSystem=strict' 'ReadWritePaths=/var/lib/octessera /run/octessera /run/octessera-boot' \
    'PrivateTmp=yes' 'ProtectHome=yes' 'ProtectKernelTunables=yes' 'ProtectKernelModules=yes' 'ProtectControlGroups=yes' \
    'RestrictNamespaces=yes' 'LockPersonality=yes' 'LimitRTPRIO=70' 'LimitMEMLOCK=infinity' 'Nice=-10' \
    'ExecStart=/usr/local/bin/octessera-pi' 'Restart=on-failure' 'RestartPreventExitStatus=78' 'RestartSec=5s'; do
    printf '%s\n' "$service_content" | grep -qFx "$required_line" || { echo "Orange runtime service is missing: $required_line" >&2; exit 1; }
  done
  if printf '%s\n' "$service_content" | grep -Eq '^(AmbientCapabilities|CapabilityBoundingSet)=|LimitRTPRIO=80|^(PrivateDevices|DevicePolicy)=|^(Restart=always|StartLimitAction=|OnFailure=|Requisite=|BindsTo=|PartOf=)|octessera-update-(guard|broker|socket)'; then echo 'Orange runtime service has an unsafe device or unsupported updater policy.' >&2; exit 1; fi
  while IFS= read -r line; do [[ "$line" == 'Requires=octessera-device-apply-reboot.socket' || "$line" == 'Requires=octessera-provision-musical-default.service' || "$line" == 'Requires=octessera-update-recovery.service' ]] || { echo 'Orange runtime service has an unexpected Requires dependency.' >&2; exit 1; }; done < <(printf '%s\n' "$service_content" | grep '^Requires=' || true)
  [[ "$(printf '%s\n' "$service_content" | grep -c '^Requires=')" == 3 ]] || { echo 'Orange runtime service has an unexpected Requires dependency count.' >&2; exit 1; }
}
