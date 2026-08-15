#!/usr/bin/env bash

# shellcheck disable=SC2154

octessera_image_metadata_value() {
  local content="$1"
  local key="$2"
  local record
  record="$(printf '%s\n' "$content" | awk -F= -v wanted="$key" '$1 == wanted { count++; value = substr($0, length(wanted) + 2) } END { if (count == 1) print value; else exit 1 }')" || return 1
  printf '%s' "$record"
}

octessera_require_image_contract() {
  local expected_mode="$1"
  local contract_content
  require_root_mode etc/octessera/image-contract.json 644
  contract_content="$(read_file etc/octessera/image-contract.json)"
  jq -e --arg expected_mode "$expected_mode" 'type == "object" and ((keys | sort) == ["image_kind", "runtime_enabled_default", "schema_version"]) and .schema_version == 1 and .image_kind == $expected_mode and (.runtime_enabled_default == (.image_kind == "production"))' <<< "$contract_content" >/dev/null || {
    echo "Image contract does not explicitly match mode $expected_mode." >&2
    exit 1
  }
  [[ "$(hash_path etc/octessera/image-contract.json)" == "$(octessera_image_metadata_value "$profile_metadata" OCTESSERA_IMAGE_CONTRACT_SHA256)" ]] || {
    echo "Image contract hash is not recorded exactly in build metadata." >&2
    exit 1
  }
}

octessera_require_image_symlink() {
  local path="$1"
  shift
  local expected_target
  local metadata
  local actual_target
  stat_path "$path" || { echo "Missing required runtime symlink: $path." >&2; exit 1; }
  if [[ -d "$target" ]]; then
    [[ -L "$target/$path" ]] || { echo "Runtime path is not a symlink: $path." >&2; exit 1; }
    actual_target="$(readlink -- "$target/$path")"
  else
    metadata="$(octessera_debugfs_stat_metadata "$target" "$path")" || { echo "Unable to inspect runtime symlink: $path." >&2; exit 1; }
    actual_target="$(octessera_debugfs_fast_link_target "$metadata")" || { echo "Unable to inspect runtime symlink: $path." >&2; exit 1; }
    [[ "$(octessera_debugfs_type "$metadata")" == symlink ]] || { echo "Runtime path is not a symlink: $path." >&2; exit 1; }
  fi
  for expected_target in "$@"; do
    [[ "$actual_target" != "$expected_target" ]] || return 0
  done
  echo "Runtime symlink target mismatch at $path." >&2
  exit 1
}

octessera_require_absent_path() {
  local path="$1"
  local status
  if stat_path "$path"; then
    echo "Production image contains unsupported runtime path: $path." >&2
    exit 1
  else
    status=$?
    [[ "$status" == 1 ]] || { echo "Unable to inspect unsupported runtime path: $path." >&2; exit 1; }
  fi
}

octessera_require_runtime_entry_set() {
  local release_path="$1"
  local entry
  local entry_type
  local metadata
  local entries_text
  local -a entries=()
  if [[ -d "$target" ]]; then
    while IFS=$'\t' read -r entry_type entry; do
      [[ -n "$entry" ]] || continue
      entries+=("$entry_type:$entry")
    done < <(find -P "$target/$release_path" -mindepth 1 -maxdepth 1 -printf '%y\t%f\n' | LC_ALL=C sort -k2)
  else
    local listing
    local listing_line
    listing="$(octessera_debugfs_list_path "$target" "$release_path")" || { echo "Unable to enumerate runtime release: $release_path." >&2; exit 1; }
    while IFS= read -r listing_line; do
      [[ -n "$listing_line" ]] || continue
      entry="$(octessera_debugfs_ls_entry_name "$listing_line")" || { echo "Malformed runtime release entry." >&2; exit 1; }
      [[ "$entry" == . || "$entry" == .. ]] && continue
      metadata="$(octessera_debugfs_stat_metadata "$target" "$release_path/$entry")" || { echo "Unable to inspect runtime release entry: $entry." >&2; exit 1; }
      entry_type="$(octessera_debugfs_type "$metadata")" || { echo "Malformed runtime release entry." >&2; exit 1; }
      entries+=("$entry_type:$entry")
    done <<< "$listing"
  fi
  entries_text="$(printf '%s\n' "${entries[@]}" | LC_ALL=C sort | paste -sd ' ' -)"
  [[ "$entries_text" == 'regular:SHA256SUMS regular:octessera-pi regular:octessera-runtime.json' || "$entries_text" == 'f:SHA256SUMS f:octessera-pi f:octessera-runtime.json' ]] || {
    echo "Runtime release contains unexpected entries: $release_path." >&2
    exit 1
  }
}

octessera_require_real_directory() {
  local path="$1"
  local metadata
  stat_path "$path" || { echo "Missing required runtime directory: $path." >&2; exit 1; }
  if [[ -d "$target" ]]; then
    [[ -d "$target/$path" && ! -L "$target/$path" ]] || { echo "Runtime directory is unsafe: $path." >&2; exit 1; }
  else
    metadata="$(octessera_debugfs_stat_metadata "$target" "$path")" || { echo "Unable to inspect runtime directory: $path." >&2; exit 1; }
    [[ "$(octessera_debugfs_type "$metadata")" == directory ]] || { echo "Runtime path is not a directory: $path." >&2; exit 1; }
  fi
}

octessera_require_owned_mode() {
  local path="$1"
  local owner="$2"
  local mode="$3"
  local actual
  local metadata
  local actual_user
  local actual_group
  local actual_mode
  local expected_mode
  expected_mode="$(octessera_canonical_mode "$mode")" || { echo "Invalid expected runtime mode for $path." >&2; exit 1; }
  if [[ -d "$target" ]]; then
    actual="$(stat -c '%u:%g %a' "$target/$path")"
    actual_user="${actual%% *}"
    actual_mode="${actual#* }"
    actual_mode="$(octessera_canonical_mode "$actual_mode")" || { echo "Invalid runtime mode: $path." >&2; exit 1; }
    actual="$actual_user $actual_mode"
  else
    metadata="$(octessera_debugfs_stat_metadata "$target" "$path")" || { echo "Unable to inspect runtime ownership: $path." >&2; exit 1; }
    actual_user="$(printf '%s\n' "$metadata" | awk '/^User:/ { for (position = 1; position < NF; position++) if ($position == "User:") print $(position + 1) }')"
    actual_group="$(printf '%s\n' "$metadata" | awk '/^User:/ { for (position = 1; position < NF; position++) if ($position == "Group:") print $(position + 1) }')"
    actual_mode="$(octessera_debugfs_mode "$metadata")" || { echo "Missing runtime mode: $path." >&2; exit 1; }
    actual_mode="$(octessera_canonical_mode "$actual_mode")" || { echo "Invalid runtime mode: $path." >&2; exit 1; }
    actual="$actual_user:$actual_group $actual_mode"
  fi
  [[ "$actual" == "$owner $expected_mode" ]] || {
    echo "Unsafe runtime ownership or mode at $path." >&2
    exit 1
  }
}

octessera_reject_runtime_sudoers() {
  local path
  local content
  local status
  local listing
  local listing_line
  local name
  local metadata
  local entry_type

  check_sudoers_file() {
    local candidate="$1"
    content="$(read_file "$candidate")"
    if printf '%s\n' "$content" | grep -Eq '(^|[^[:alnum:]_-])octessera-runtime([^[:alnum:]_-]|$)'; then
      echo "Production runtime account appears in sudoers: $candidate." >&2
      return 1
    fi
  }
  for path in etc/sudoers etc/sudoers.d/octessera-update etc/sudoers.d/octessera-ssh-key-admin; do
    if stat_path "$path"; then
      check_sudoers_file "$path" || return 1
    else
      status=$?
      [[ "$status" == 1 ]] || { echo "Unable to inspect sudoers path: $path." >&2; return 1; }
    fi
  done
  if [[ -d "$target" ]]; then
    if [[ -d "$target/etc/sudoers.d" && ! -L "$target/etc/sudoers.d" ]]; then
      while IFS= read -r -d '' path; do
        check_sudoers_file "${path#"$target/"}" || return 1
      done < <(find -P "$target/etc/sudoers.d" -type f -print0)
    fi
  else
    if stat_path etc/sudoers.d; then
      listing="$(octessera_debugfs_list_path "$target" etc/sudoers.d)" || { echo "Unable to enumerate sudoers.d." >&2; return 1; }
      while IFS= read -r listing_line; do
        [[ -n "$listing_line" ]] || continue
        name="$(octessera_debugfs_ls_entry_name "$listing_line")" || { echo "Malformed sudoers.d entry." >&2; return 1; }
        [[ "$name" == . || "$name" == .. ]] && continue
        metadata="$(octessera_debugfs_stat_metadata "$target" "etc/sudoers.d/$name")" || { echo "Unable to inspect sudoers.d entry: $name." >&2; return 1; }
        if entry_type="$(octessera_debugfs_type "$metadata")"; then
          [[ "$entry_type" == regular ]] || continue
          check_sudoers_file "etc/sudoers.d/$name" || return 1
        else
          echo "Unable to parse sudoers.d entry: $name." >&2
          return 1
        fi
      done <<< "$listing"
    else
      status=$?
      [[ "$status" == 1 ]] || { echo "Unable to inspect sudoers.d." >&2; return 1; }
    fi
  fi
}

octessera_require_runtime_account() {
  local passwd_content="$1"
  local group_content="$2"
  local runtime_passwd
  local runtime_shadow
  local runtime_group
  local runtime_name
  local runtime_password
  local runtime_uid
  local runtime_gid
  local runtime_home
  local runtime_shell
  local runtime_group_gid
  local protected_group

  runtime_passwd="$(printf '%s\n' "$passwd_content" | awk -F: '$1 == "octessera-runtime" { count++; record = $0 } END { if (count == 1) print record; else exit 1 }')" || { echo "Production image is missing the unique octessera-runtime account." >&2; exit 1; }
  IFS=: read -r runtime_name runtime_password runtime_uid runtime_gid _ runtime_home runtime_shell <<< "$runtime_passwd"
  [[ "$runtime_name" == octessera-runtime && "$runtime_uid" =~ ^[0-9]+$ && "$runtime_uid" -lt 1000 && "$runtime_home" == /nonexistent && "$runtime_shell" == /usr/sbin/nologin ]] || {
    echo "Production octessera-runtime account is not a locked system no-shell user." >&2
    exit 1
  }
  runtime_shadow="$(read_file etc/shadow)"
  runtime_password="$(printf '%s\n' "$runtime_shadow" | awk -F: '$1 == "octessera-runtime" { count++; hash = $2 } END { if (count == 1) print hash; else exit 1 }')" || { echo "Production image is missing the octessera-runtime shadow entry." >&2; exit 1; }
  case "$runtime_password" in
    ""|\!*|\**|x) ;;
    *) echo "octessera-runtime has an unlocked password." >&2; exit 1 ;;
  esac
  runtime_group="$(printf '%s\n' "$group_content" | awk -F: '$1 == "octessera-runtime" { count++; record = $0 } END { if (count == 1) print record; else exit 1 }')" || { echo "Production image is missing the octessera-runtime group." >&2; exit 1; }
  IFS=: read -r _ _ runtime_group_gid _ <<< "$runtime_group"
  [[ "$runtime_gid" == "$runtime_group_gid" ]] || { echo "octessera-runtime primary group does not match its account." >&2; exit 1; }
  for protected_group in sudo admin; do
    if printf '%s\n' "$group_content" | awk -F: -v wanted="$protected_group" '$1 == wanted && ("," $4 ",") ~ /,octessera-runtime,/' | grep -q .; then
      echo "octessera-runtime is present in protected admin group: $protected_group." >&2
      exit 1
    fi
  done
  octessera_reject_runtime_sudoers || { echo "Production runtime account sudo separation failed." >&2; exit 1; }
  printf '%s:%s' "$runtime_uid" "$runtime_gid"
}

octessera_require_runtime_elf() {
  local path="$1"
  local binary_path
  if [[ -d "$target" ]]; then
    binary_path="$target/$path"
  else
    binary_path="$inspect_work/octessera-pi"
    rm -f -- "$binary_path"
    debugfs -R "$(octessera_debugfs_dump_request "$path" "$binary_path")" "$target" >/dev/null 2>"$inspect_work/runtime-elf.stderr" || {
      cat -- "$inspect_work/runtime-elf.stderr" >&2
      echo "Unable to inspect runtime binary: $path." >&2
      exit 1
    }
  fi
  if ! python3 - "$binary_path" <<'PY'
import sys
from pathlib import Path

header = Path(sys.argv[1]).read_bytes()[:20]
if len(header) != 20 or header[:7] != b"\x7fELF\x02\x01\x01" or header[18:20] != b"\xb7\x00":
    raise SystemExit(1)
PY
  then
    echo "Runtime binary is not ELF64 AArch64: $path." >&2
    exit 1
  fi
}

octessera_require_runtime_service() {
  local service_content="$1"
  for required_line in \
    'StartLimitIntervalSec=30s' \
    'StartLimitBurst=3' \
    'After=octessera-provision-musical-default.service octessera-orange-usb-gadget.service sound.target' \
    'Requires=octessera-device-apply-reboot.socket' \
    'After=octessera-device-apply-reboot.socket' \
    'User=octessera-runtime' \
    'Group=octessera-runtime' \
    'Environment=OCTESSERA_EXPECTED_BOARD_PROFILE=orange-pi-zero-2w' \
    'Environment=OCTESSERA_PI_STORE_DIR=/var/lib/octessera/presets' \
    'Environment=OCTESSERA_PI_SAMPLES_DIR=/var/lib/octessera/samples' \
    'Environment=OCTESSERA_CANDIDATE_HEALTH_PATH=/run/octessera/candidate-ready.json' \
    'Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1' \
    'RuntimeDirectory=octessera' \
    'RuntimeDirectoryMode=0755' \
    'NoNewPrivileges=yes' \
    'ProtectSystem=strict' \
    'ReadWritePaths=/var/lib/octessera /run/octessera /run/octessera-boot' \
    'PrivateTmp=yes' \
    'ProtectHome=yes' \
    'ProtectKernelTunables=yes' \
    'ProtectKernelModules=yes' \
    'ProtectControlGroups=yes' \
    'RestrictNamespaces=yes' \
    'LockPersonality=yes' \
    'LimitRTPRIO=70' \
    'LimitMEMLOCK=infinity' \
    'Nice=-10' \
    'ExecStart=/usr/local/bin/octessera-pi' \
    'Restart=on-failure' \
    'RestartPreventExitStatus=78' \
    'RestartSec=5s'; do
    printf '%s\n' "$service_content" | grep -qFx "$required_line" || { echo "Orange runtime service is missing: $required_line" >&2; exit 1; }
  done
  if printf '%s\n' "$service_content" | grep -Eq '^(AmbientCapabilities|CapabilityBoundingSet)=|LimitRTPRIO=80|^(PrivateDevices|DevicePolicy)=|^(Restart=always|StartLimitAction=|OnFailure=|Requisite=|BindsTo=|PartOf=)|octessera-update'; then
    echo "Orange runtime service has an unsafe device or unsupported updater policy." >&2
    exit 1
  fi
  while IFS= read -r line; do
    [[ "$line" == 'Requires=octessera-device-apply-reboot.socket' ]] || { echo "Orange runtime service has an unexpected Requires dependency." >&2; exit 1; }
  done < <(printf '%s\n' "$service_content" | grep '^Requires=' || true)
}

octessera_require_runtime_udev_rule() {
  local rule_path=etc/udev/rules.d/70-octessera-orange-runtime.rules
  local expected_rule
  local rule_content
  local metadata

  if [[ -d "$target" ]]; then
    [[ -f "$target/$rule_path" && ! -L "$target/$rule_path" ]] || { echo "Orange runtime udev rule is not a regular file." >&2; exit 1; }
  else
    metadata="$(octessera_debugfs_stat_metadata "$target" "$rule_path")" || { echo "Unable to inspect Orange runtime udev rule." >&2; exit 1; }
    [[ "$(octessera_debugfs_type "$metadata")" == regular ]] || { echo "Orange runtime udev rule is not a regular file." >&2; exit 1; }
  fi
  require_root_mode "$rule_path" 644
  expected_rule=$'KERNEL=="i2c-2", GROUP="octessera-runtime", MODE="0660"\nKERNEL=="spidev1.0", GROUP="octessera-runtime", MODE="0660"\nKERNEL=="gpiochip1", GROUP="octessera-runtime", MODE="0660"'
  if ! rule_content="$(read_file "$rule_path" || exit; printf '\037')"; then
    echo "Unable to read Orange runtime udev rule." >&2
    exit 1
  fi
  rule_content="${rule_content%$'\037'}"
  [[ "$rule_content" == "$expected_rule" || "$rule_content" == "$expected_rule"$'\n' ]] || { echo "Orange runtime udev rule content is not exact." >&2; exit 1; }
}

octessera_require_orange_boot_service() {
  local service_content
  service_content="$(read_file etc/systemd/system/octessera-orange-boot-splash.service)"
  for required_line in \
    'User=octessera-runtime' \
    'Group=octessera-runtime' \
    'ExecStart=/usr/local/sbin/octessera-orange-oled-logo boot-loop' \
    'RuntimeDirectory=octessera-boot' \
    'RuntimeDirectoryMode=0750' \
    'RuntimeDirectoryPreserve=yes' \
    'ProtectSystem=strict' \
    'DevicePolicy=closed' \
    'DeviceAllow=/dev/spidev1.0 rw' \
    'DeviceAllow=/dev/gpiochip1 rw' \
    'After=systemd-udev-trigger.service systemd-modules-load.service systemd-udevd.service local-fs.target'; do
    printf '%s\n' "$service_content" | grep -qFx "$required_line" || { echo "Orange boot service is missing: $required_line" >&2; exit 1; }
  done
  ! printf '%s\n' "$service_content" | grep -q '^Conflicts=' || { echo 'Orange boot service conflicts with runtime.' >&2; exit 1; }
  require_root_mode etc/systemd/system/octessera-orange-boot-splash.service 644
  octessera_require_image_symlink etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service ../octessera-orange-boot-splash.service /etc/systemd/system/octessera-orange-boot-splash.service
}

octessera_require_device_apply_lane() {
  local socket_content
  local service_content
  local helper_content
  socket_content="$(read_file etc/systemd/system/octessera-device-apply-reboot.socket)"
  service_content="$(read_file etc/systemd/system/octessera-device-apply-reboot@.service)"
  helper_content="$(read_file usr/local/sbin/octessera-device-apply-reboot)"
  for line in \
    'Before=sound.target octessera.service' \
    'After=local-fs.target' \
    'ListenStream=/run/octessera-device-apply/reboot.sock' \
    'SocketMode=0660' \
    'SocketUser=root' \
    'SocketGroup=octessera-runtime' \
    'Accept=yes'; do
    printf '%s\n' "$socket_content" | grep -qFx "$line" || { echo "Orange apply socket is missing: $line" >&2; exit 1; }
  done
  ! printf '%s\n' "$socket_content" | grep -qFx 'After=local-fs.target octessera-provision-musical-default.service' || { echo 'Orange apply socket must not wait for musical provisioning.' >&2; exit 1; }
  for line in \
    'User=root' \
    'Group=root' \
    'StandardInput=socket' \
    'StandardOutput=socket' \
    'ExecStart=/usr/local/sbin/octessera-device-apply-reboot' \
    'NoNewPrivileges=yes' \
    'ProtectSystem=strict' \
    'ProtectHome=yes' \
    'RestrictAddressFamilies=AF_UNIX'; do
    printf '%s\n' "$service_content" | grep -qFx "$line" || { echo "Orange apply service is missing: $line" >&2; exit 1; }
  done
  if printf '%s\n' "$service_content" | grep -qF 'systemctl'; then
    echo "Orange apply service must not embed arbitrary systemctl commands." >&2
    exit 1
  fi
  for line in \
    'SYSTEMCTL_PATH = "/usr/bin/systemctl"' \
    'REBOOT_REQUEST = b"reboot\n"' \
    'POWEROFF_REQUEST = b"poweroff\n"' \
    'ACCEPTED = b"accepted\n"' \
    'REJECTED = b"rejected\n"' \
    'if request == REBOOT_REQUEST:' \
    '_validate_config(CONFIG_PATH)' \
    'command = "poweroff"' \
    'subprocess.run([SYSTEMCTL_PATH, command], check=True)' \
    'output_stream.write(ACCEPTED)' \
    'output_stream.write(REJECTED)'; do
    printf '%s\n' "$helper_content" | grep -qF "$line" || { echo "Orange apply helper is missing: $line" >&2; exit 1; }
  done
  if printf '%s\n' "$helper_content" | grep -Eq 'os\.system|shell=True|subprocess\.Popen|subprocess\.call'; then
    echo 'Orange apply helper contains an unapproved command broker path.' >&2
    exit 1
  fi
}

octessera_require_device_config_assets() {
  require_root_mode usr/local/lib/octessera/device_config.py 644
  require_root_mode usr/local/sbin/octessera-device-apply-reboot 755
  require_root_mode usr/share/octessera/defaults/pi-default.json 644
}

octessera_require_orange_shutdown_service() {
  local service_content
  service_content="$(read_file etc/systemd/system/octessera-orange-oled-shutdown.service)"
  for required_line in \
    'After=octessera.service' \
    'Before=shutdown.target reboot.target halt.target' \
    'User=octessera-runtime' \
    'Group=octessera-runtime' \
    'ProtectSystem=strict' \
    'ReadWritePaths=/run/octessera-boot' \
    'DevicePolicy=closed' \
    'DeviceAllow=/dev/spidev1.0 rw' \
    'DeviceAllow=/dev/gpiochip1 rw' \
    'ExecStart=-/usr/local/sbin/octessera-orange-oled-logo shutdown' \
    'TimeoutStartSec=5'; do
    printf '%s\n' "$service_content" | grep -qFx "$required_line" || { echo "Orange shutdown service is missing: $required_line" >&2; exit 1; }
  done
  ! printf '%s\n' "$service_content" | grep -qFx 'SupplementaryGroups=audio i2c spi gpio' || { echo 'Orange shutdown service requires unavailable supplementary groups.' >&2; exit 1; }
}

octessera_require_orange_suspend_service() {
  local service_content
  service_content="$(read_file etc/systemd/system/octessera-orange-oled-suspend.service)"
  for required_line in \
    'After=octessera.service' \
    'Requisite=octessera.service' \
    'Before=sleep.target' \
    'RequiredBy=sleep.target' \
    'StopWhenUnneeded=yes' \
    'Type=oneshot' \
    'RemainAfterExit=yes' \
    'User=octessera-runtime' \
    'Group=octessera-runtime' \
    'RuntimeDirectory=octessera-oled-suspend' \
    'RuntimeDirectoryMode=0700' \
    'RestrictAddressFamilies=AF_UNIX' \
    'ExecStart=/usr/local/sbin/octessera-orange-oled-suspend prepare' \
    'ExecStop=/usr/local/sbin/octessera-orange-oled-suspend resume' \
    'TimeoutStartSec=8' \
    'TimeoutStopSec=8'; do
    printf '%s\n' "$service_content" | grep -qFx "$required_line" || { echo "Orange suspend service is missing: $required_line" >&2; exit 1; }
  done
  ! printf '%s\n' "$service_content" | grep -qFx 'SupplementaryGroups=audio i2c spi gpio' || { echo 'Orange suspend service requires unavailable supplementary groups.' >&2; exit 1; }
  ! printf '%s\n' "$service_content" | grep -qE '^(Conflicts=|BusName=)|systemctl' || { echo 'Orange suspend service has an unsafe lifecycle dependency.' >&2; exit 1; }
  octessera_require_image_symlink etc/systemd/system/sleep.target.requires/octessera-orange-oled-suspend.service ../octessera-orange-oled-suspend.service /etc/systemd/system/octessera-orange-oled-suspend.service
  reject_path etc/systemd/system/sleep.target.wants/octessera-orange-oled-suspend.service
  reject_path lib/systemd/system-sleep/octessera-orange-oled
  reject_path usr/lib/systemd/system-sleep/octessera-orange-oled
}

octessera_inspect_runtime_mode() {
  local metadata_content="$1"
  local requested_mode="${2:-diagnostic}"
  local image_mode
  local runtime_default
  local version
  local binary_hash
  local manifest_hash
  local metadata_hash
  local release_path
  local runtime_metadata
  local runtime_sums
  local actual_binary_hash
  local actual_manifest_hash
  local runtime_owner
  local passwd_content
  local group_content

  image_mode="$(octessera_image_metadata_value "$metadata_content" OCTESSERA_IMAGE_MODE)" || { echo "Build metadata is missing the explicit Orange image mode." >&2; exit 1; }
  runtime_default="$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_ENABLED_DEFAULT)" || { echo "Build metadata is missing the runtime default." >&2; exit 1; }
  [[ "$image_mode" == "$requested_mode" ]] || { echo "Inspector mode $requested_mode does not match image metadata mode $image_mode." >&2; exit 1; }
  octessera_require_orange_boot_service
  octessera_require_orange_shutdown_service
  octessera_require_orange_suspend_service
  octessera_require_device_apply_lane
  octessera_require_device_config_assets
  octessera_require_image_symlink etc/systemd/system/sockets.target.wants/octessera-device-apply-reboot.socket ../octessera-device-apply-reboot.socket /etc/systemd/system/octessera-device-apply-reboot.socket
  case "$image_mode:$runtime_default" in
    diagnostic:false)
      octessera_require_image_contract diagnostic
      for path in \
        etc/systemd/system/octessera.service \
        etc/systemd/system/multi-user.target.wants/octessera.service \
        usr/local/bin/octessera-pi \
        opt/octessera/current \
        opt/octessera/releases; do
        reject_path "$path"
      done
      [[ "$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_VERSION)" == none ]] || { echo "Diagnostic image has a runtime version." >&2; exit 1; }
      [[ "$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_BINARY_SHA256)" == none ]] || { echo "Diagnostic image has a runtime binary hash." >&2; exit 1; }
      [[ "$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_MANIFEST_SHA256)" == none ]] || { echo "Diagnostic image has a runtime manifest hash." >&2; exit 1; }
      [[ "$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_METADATA_SHA256)" == none ]] || { echo "Diagnostic image has runtime metadata." >&2; exit 1; }
      ;;
    production:true)
      octessera_require_image_contract production
      for path in \
        etc/systemd/system/octessera-update-guard.service \
        etc/systemd/system/octessera-update-recovery.service \
        etc/systemd/system/multi-user.target.wants/octessera-update-recovery.service \
        usr/local/sbin/octessera-update \
        usr/local/sbin/octessera-update-guard \
        usr/local/sbin/octessera-update-recovery \
        usr/local/lib/octessera/updater_protocol.py \
        usr/local/lib/octessera/updater_state.py \
        usr/local/lib/octessera/updater_assets.py \
        usr/local/lib/octessera/updater_guard.py \
        usr/local/lib/octessera/updater_cli.py \
        etc/sudoers.d/octessera-update; do
        octessera_require_absent_path "$path"
      done
      version="$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_VERSION)"
      binary_hash="$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_BINARY_SHA256)"
      manifest_hash="$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_MANIFEST_SHA256)"
      metadata_hash="$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_METADATA_SHA256)"
      [[ "$version" =~ ^[A-Za-z0-9][A-Za-z0-9._+-]{0,63}$ && "$binary_hash" =~ ^[a-f0-9]{64}$ && "$manifest_hash" =~ ^[a-f0-9]{64}$ && "$metadata_hash" =~ ^[a-f0-9]{64}$ ]] || { echo "Production image metadata has invalid runtime hashes or version." >&2; exit 1; }
      release_path="opt/octessera/releases/$version"
      octessera_require_real_directory opt/octessera
      octessera_require_real_directory opt/octessera/releases
      octessera_require_real_directory "$release_path"
      require_root_mode opt/octessera 755
      require_root_mode opt/octessera/releases 755
      require_root_mode "$release_path" 555
      octessera_require_runtime_entry_set "$release_path"
      require_root_mode "$release_path/octessera-pi" 555
      require_root_mode "$release_path/octessera-runtime.json" 444
      require_root_mode "$release_path/SHA256SUMS" 444
      actual_binary_hash="$(hash_path "$release_path/octessera-pi")"
      actual_manifest_hash="$(hash_path "$release_path/SHA256SUMS")"
      [[ "$actual_binary_hash" == "$binary_hash" ]] || { echo "Production runtime binary hash mismatch." >&2; exit 1; }
      [[ "$actual_manifest_hash" == "$manifest_hash" ]] || { echo "Production runtime manifest hash mismatch." >&2; exit 1; }
      [[ "$(hash_path "$release_path/octessera-runtime.json")" == "$metadata_hash" ]] || { echo "Production runtime metadata hash mismatch." >&2; exit 1; }
      runtime_metadata="$(read_file "$release_path/octessera-runtime.json")"
      jq -e 'type == "object" and ((keys | sort) == ["artifact_kind", "binary_sha256", "name", "profile", "runtime_ready", "version"]) and .name == "octessera-pi" and .profile == "orange-pi-zero-2w" and .artifact_kind == "production-runtime" and .runtime_ready == true and (.version | type == "string") and (.binary_sha256 | type == "string" and test("^[a-f0-9]{64}$"))' <<< "$runtime_metadata" >/dev/null || { echo "Production runtime metadata is not exact." >&2; exit 1; }
      [[ "$(jq -r '.version' <<< "$runtime_metadata")" == "$version" && "$(jq -r '.binary_sha256' <<< "$runtime_metadata")" == "$binary_hash" ]] || { echo "Production runtime metadata is not hash-bound." >&2; exit 1; }
      runtime_sums="$(read_file "$release_path/SHA256SUMS")"
      [[ "$runtime_sums" =~ ^([a-f0-9]{64})[[:space:]][[:space:]]octessera-pi$ && "${BASH_REMATCH[1]}" == "$binary_hash" ]] || { echo "Production SHA256SUMS is not exact." >&2; exit 1; }
      octessera_require_runtime_elf "$release_path/octessera-pi"
      octessera_require_image_symlink opt/octessera/current "/opt/octessera/releases/$version"
      octessera_require_image_symlink usr/local/bin/octessera-pi /opt/octessera/current/octessera-pi
      passwd_content="$(read_file etc/passwd)"
      group_content="$(read_file etc/group)"
      runtime_owner="$(octessera_require_runtime_account "$passwd_content" "$group_content")"
      for runtime_group in audio i2c spi gpio; do
        printf '%s\n' "$group_content" | awk -F: -v wanted="$runtime_group" '$1 == wanted && ("," $4 ",") ~ /,octessera-runtime,/' | grep -q . || {
          echo "Production image is missing octessera-runtime membership in group: $runtime_group." >&2
          exit 1
        }
      done
      octessera_require_real_directory var/lib/octessera/presets
      octessera_require_real_directory var/lib/octessera/samples
      octessera_require_owned_mode var/lib/octessera/presets "$runtime_owner" 755
      octessera_require_owned_mode var/lib/octessera/samples "$runtime_owner" 755
      octessera_require_runtime_udev_rule
      require_root_mode etc/systemd/system/octessera.service 644
      require_root_mode etc/systemd/system/multi-user.target.wants/octessera.service 777
      octessera_require_image_symlink etc/systemd/system/multi-user.target.wants/octessera.service ../octessera.service /etc/systemd/system/octessera.service
      octessera_require_runtime_service "$(read_file etc/systemd/system/octessera.service)"
      ;;
    *) echo "Image mode/runtime default combination is invalid: $image_mode/$runtime_default." >&2; exit 1 ;;
  esac
}
