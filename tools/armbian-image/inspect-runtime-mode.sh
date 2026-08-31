#!/usr/bin/env bash
# shellcheck disable=SC2154
module_dir="$(dirname "${BASH_SOURCE[0]}")"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$module_dir/validation-assertions.sh"

octessera_inspect_runtime_mode() {
  local metadata_content="$1" requested_mode="${2:-diagnostic}" image_mode runtime_default version binary_hash manifest_hash metadata_hash release_path runtime_metadata runtime_sums actual_binary_hash actual_manifest_hash runtime_owner passwd_content group_content login_defs_content hosts_content expected_hostname_hash
  image_mode="$(octessera_image_metadata_value "$metadata_content" OCTESSERA_IMAGE_MODE)" || { echo 'Build metadata is missing the explicit Orange image mode.' >&2; exit 1; }
  runtime_default="$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_ENABLED_DEFAULT)" || { echo 'Build metadata is missing the runtime default.' >&2; exit 1; }
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
      octessera_require_real_directory var/lib/octessera/samples
      runtime_owner="$(octessera_runtime_owner_from_passwd "$(read_file etc/passwd)")"
      octessera_require_owned_mode var/lib/octessera/samples "$runtime_owner" 755
      for path in etc/systemd/system/octessera.service etc/systemd/system/multi-user.target.wants/octessera.service usr/local/bin/octessera-pi opt/octessera/current opt/octessera/releases; do reject_path "$path"; done
      [[ "$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_VERSION)" == none ]]
      [[ "$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_BINARY_SHA256)" == none ]]
      [[ "$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_MANIFEST_SHA256)" == none ]]
      [[ "$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_METADATA_SHA256)" == none ]]
      ;;
    production:true)
      octessera_require_image_contract production
      for path in etc/systemd/system/octessera-update-guard.service etc/systemd/system/octessera-update-recovery.service etc/systemd/system/multi-user.target.wants/octessera-update-recovery.service usr/local/sbin/octessera-update usr/local/sbin/octessera-update-guard usr/local/sbin/octessera-update-recovery usr/local/lib/octessera/updater_protocol.py usr/local/lib/octessera/updater_contract.py usr/local/lib/octessera/updater_state.py usr/local/lib/octessera/updater_assets.py usr/local/lib/octessera/updater_guard.py usr/local/lib/octessera/updater_cli.py usr/local/lib/octessera/updater_profiles.py usr/local/sbin/octessera-update-broker etc/systemd/system/octessera-update.socket etc/systemd/system/octessera-update@.service etc/sudoers.d/octessera-update; do stat_path "$path" || { echo "Production image is missing updater path: $path." >&2; exit 1; }; done
      for path in usr/local/sbin/octessera-update usr/local/sbin/octessera-update-broker usr/local/sbin/octessera-update-guard usr/local/sbin/octessera-update-recovery; do require_root_mode "$path" 755; done
      for path in usr/local/lib/octessera/updater_protocol.py usr/local/lib/octessera/updater_contract.py usr/local/lib/octessera/updater_state.py usr/local/lib/octessera/updater_assets.py usr/local/lib/octessera/updater_guard.py usr/local/lib/octessera/updater_cli.py usr/local/lib/octessera/updater_profiles.py etc/systemd/system/octessera-update.socket etc/systemd/system/octessera-update@.service; do require_root_mode "$path" 644; done
      require_root_mode etc/sudoers.d/octessera-update 440
      octessera_require_image_symlink etc/systemd/system/sockets.target.wants/octessera-update.socket ../octessera-update.socket /etc/systemd/system/octessera-update.socket
      update_socket_unit="$(read_file etc/systemd/system/octessera-update.socket)"
      printf '%s\n' "$update_socket_unit" | grep -q '^SocketMode=0660$'
      printf '%s\n' "$update_socket_unit" | grep -q '^SocketGroup=octessera-runtime$'
      update_sudoers="$(read_file etc/sudoers.d/octessera-update)"
      octessera_reject_text_match 'Production updater sudoers must not grant runtime or unrestricted administrator access.' "$update_sudoers" -Eq 'octessera-runtime|ALL=\(ALL\)'
      version="$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_VERSION)"
      binary_hash="$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_BINARY_SHA256)"
      manifest_hash="$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_MANIFEST_SHA256)"
      metadata_hash="$(octessera_image_metadata_value "$metadata_content" OCTESSERA_RUNTIME_METADATA_SHA256)"
      [[ "$version" =~ ^[A-Za-z0-9][A-Za-z0-9._+-]{0,63}$ && "$binary_hash" =~ ^[a-f0-9]{64}$ && "$manifest_hash" =~ ^[a-f0-9]{64}$ && "$metadata_hash" =~ ^[a-f0-9]{64}$ ]]
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
      require_root_mode "$release_path/update-manifest.json" 444
      actual_binary_hash="$(hash_path "$release_path/octessera-pi")"
      actual_manifest_hash="$(hash_path "$release_path/SHA256SUMS")"
      [[ "$actual_binary_hash" == "$binary_hash" ]]
      [[ "$actual_manifest_hash" == "$manifest_hash" ]]
      [[ "$(hash_path "$release_path/octessera-runtime.json")" == "$metadata_hash" ]]
      runtime_metadata="$(read_file "$release_path/octessera-runtime.json")"
      jq -e 'type == "object" and ((keys | sort) == ["artifact_kind", "binary_sha256", "name", "profile", "runtime_ready", "version"]) and .name == "octessera-pi" and .profile == "orange-pi-zero-2w" and .artifact_kind == "production-runtime" and .runtime_ready == true and (.version | type == "string") and (.binary_sha256 | type == "string" and test("^[a-f0-9]{64}$"))' <<< "$runtime_metadata" >/dev/null
      [[ "$(jq -r .version <<< "$runtime_metadata")" == "$version" && "$(jq -r .binary_sha256 <<< "$runtime_metadata")" == "$binary_hash" ]]
      runtime_sums="$(read_file "$release_path/SHA256SUMS")"
      [[ "$runtime_sums" =~ ^([a-f0-9]{64})[[:space:]][[:space:]]octessera-pi$ && "${BASH_REMATCH[1]}" == "$binary_hash" ]]
      octessera_require_runtime_elf "$release_path/octessera-pi"
      octessera_require_image_symlink opt/octessera/current "/opt/octessera/releases/$version"
      octessera_require_image_symlink usr/local/bin/octessera-pi /opt/octessera/current/octessera-pi
      passwd_content="$(read_file etc/passwd)"
      group_content="$(read_file etc/group)"
      runtime_owner="$(octessera_require_runtime_account "$passwd_content" "$group_content")"
      for runtime_group in audio i2c spi gpio video; do printf '%s\n' "$group_content" | awk -F: -v wanted="$runtime_group" '$1 == wanted && ("," $4 ",") ~ /,octessera-runtime,/' | grep -q . || { echo "Production image is missing octessera-runtime membership in group: $runtime_group." >&2; exit 1; }; done
      octessera_require_real_directory var/lib/octessera/presets
      octessera_require_real_directory var/lib/octessera/samples
      octessera_require_owned_mode var/lib/octessera/presets "$runtime_owner" 755
      octessera_require_owned_mode var/lib/octessera/samples "$runtime_owner" 755
      octessera_require_runtime_udev_rule
      require_root_mode etc/systemd/system/octessera.service 644
      require_root_mode etc/systemd/system/multi-user.target.wants/octessera.service 777
      octessera_require_image_symlink etc/systemd/system/multi-user.target.wants/octessera.service ../octessera.service /etc/systemd/system/octessera.service
      stat_path etc/login.defs || { echo 'Production image is missing /etc/login.defs.' >&2; exit 1; }
      require_root_mode etc/login.defs 644
      login_defs_content="$(read_file etc/login.defs)"
      [[ "$(printf '%s\n' "$login_defs_content" | awk '$1 == "TTYPERM" { count++; if (NF == 2) record = $1 " " $2 } END { if (count != 1 || record != "TTYPERM 0620") exit 1; print record }')" == 'TTYPERM 0620' ]] || { echo 'Production /etc/login.defs must contain exactly one active TTYPERM 0620.' >&2; exit 1; }
      [[ "$(printf '%s\n' "$group_content" | awk -F: '$1 == "tty" { count++ } END { print count + 0 }')" == 1 ]] || { echo 'Production group database must contain exactly one tty group.' >&2; exit 1; }
      stat_path etc/hostname || { echo 'Production image is missing /etc/hostname.' >&2; exit 1; }
      stat_path etc/hosts || { echo 'Production image is missing /etc/hosts.' >&2; exit 1; }
      expected_hostname_hash="$(printf '%s\n' 'octessera-opi' | sha256sum | awk '{ print $1 }')"
      [[ "$(hash_path etc/hostname)" == "$expected_hostname_hash" ]] || { echo 'Production /etc/hostname is not exactly octessera-opi.' >&2; exit 1; }
      hosts_content="$(read_file etc/hosts)"
      if ! printf '%s\n' "$hosts_content" | awk '
        {
          line = $0
          sub(/#.*/, "", line)
          $0 = line
          if ($1 == "127.0.1.1") {
            ipv4_rows++
            for (i = 2; i <= NF; i++) if ($i == "octessera-opi") ipv4_target = 1
          }
          if ($1 == "::1") {
            ipv6_rows++
            for (i = 2; i <= NF; i++) if ($i == "octessera-opi") ipv6_target = 1
          }
          for (i = 1; i <= NF; i++) if ($i == "orangepizero2w") old_alias = 1
        }
        END { exit !(ipv4_rows == 1 && ipv4_target && ipv6_rows >= 1 && ipv6_target && !old_alias) }
      '; then
        echo 'Production /etc/hosts hostname coherence is invalid.' >&2
        exit 1
      fi
      octessera_require_runtime_service "$(read_file etc/systemd/system/octessera.service)"
      ;;
    *) echo "Image mode/runtime default combination is invalid: $image_mode/$runtime_default." >&2; exit 1 ;;
  esac
}
