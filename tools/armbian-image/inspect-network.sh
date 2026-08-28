#!/usr/bin/env bash
# shellcheck disable=SC2154

octessera_require_wifi_foundation() {
  local helper_path=usr/local/sbin/octessera-wifi-foundation
  local unit_path=etc/systemd/system/octessera-wifi-foundation.service
  local binary_path=usr/local/bin/wifi-connect artifact_doc_root=usr/local/share/doc/octessera/wifi-connect helper_content unit_content metadata_content path
  for path in "$helper_path" "$unit_path" "$binary_path"; do
    stat_path "$path" || { echo "Missing inactive Wi-Fi foundation path: $path." >&2; exit 1; }
  done
  require_root_mode "$helper_path" 755
  require_root_mode "$unit_path" 644
  require_root_mode "$binary_path" 755
  if [[ "${constructor_policy_required:-false}" == true ]]; then
    for path in "$artifact_doc_root/LICENSE" "$artifact_doc_root/THIRD-PARTY-NOTICES.md" "$artifact_doc_root/wifi-connect.metadata.json" "$artifact_doc_root/cargo-metadata.json"; do
      stat_path "$path" || { echo "Missing patched wifi-connect documentation path: $path." >&2; exit 1; }
    done
    for path in "$artifact_doc_root/LICENSE" "$artifact_doc_root/THIRD-PARTY-NOTICES.md" "$artifact_doc_root/wifi-connect.metadata.json" "$artifact_doc_root/cargo-metadata.json"; do require_root_mode "$path" 644; done
    [[ "$(hash_path "$binary_path")" == 4a6ea81ad10a199064c2c9bf3f2b9fa39daadff3d8beacbf5685f88b64561627 ]] || { echo "Installed wifi-connect binary has the wrong SHA-256." >&2; exit 1; }
    metadata_content="$(read_file "$artifact_doc_root/wifi-connect.metadata.json")"
    printf '%s\n' "$metadata_content" | grep -qF '"binary_sha256": "4a6ea81ad10a199064c2c9bf3f2b9fa39daadff3d8beacbf5685f88b64561627"' || { echo "Installed wifi-connect metadata has the wrong binary SHA-256." >&2; exit 1; }
    printf '%s\n' "$metadata_content" | grep -qF '"patch_sha256": "c9538ec7428b37c29fdfbe738cb10913a1036247270616c062228d8066f98dc6"' || { echo "Installed wifi-connect metadata has the wrong patch SHA-256." >&2; exit 1; }
  fi
  helper_content="$(read_file "$helper_path")"
  unit_content="$(read_file "$unit_path")"
  printf '%s\n' "$helper_content" | grep -qF -- '--portal-interface wlan0'
  printf '%s\n' "$helper_content" | grep -qF -- '--portal-gateway 192.168.42.1'
  printf '%s\n' "$helper_content" | grep -qF -- '900s'
  printf '%s\n' "$unit_content" | grep -qFx 'User=root'
  printf '%s\n' "$unit_content" | grep -qFx 'Group=root'
  printf '%s\n' "$unit_content" | grep -qFx 'ExecStart=/usr/local/sbin/octessera-wifi-foundation'
  printf '%s\n' "$unit_content" | grep -qFx 'TimeoutStartSec=905s'
  if printf '%s\n' "$helper_content" "$unit_content" | grep -Eiq 'sidecar|hostname|ssh|password|country|setup[-_ ]?(complete|force)|credential|secret|/sys/class/net|iw[[:space:]]+dev|nmcli.*device|mac|wpa_passphrase|chpasswd|ssid=|psk=|BEGIN (RSA|OPENSSH|PRIVATE) KEY'; then
    echo 'Wi-Fi foundation contains forbidden behavior or secret handling.' >&2
    exit 1
  fi
}
