#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=tools/armbian-image/test-inspector-fixture.sh
source "$script_dir/test-inspector-fixture.sh"
module_dir="$script_dir"
# shellcheck source=tools/armbian-image/inspect-network.sh
source "$module_dir/inspect-network.sh"

target="$fake_image"
export target
stat_path() {
  case "$1" in
    usr/local/sbin/octessera-wifi-foundation|etc/systemd/system/octessera-wifi-foundation.service|usr/local/bin/wifi-connect) return 0 ;;
    usr/local/share/doc/octessera/wifi-connect/*) return 0 ;;
    *) return 1 ;;
  esac
}
require_root_mode() { :; }
hash_path() {
  [[ "$1" == usr/local/bin/wifi-connect ]] && printf '%s\n' 4a6ea81ad10a199064c2c9bf3f2b9fa39daadff3d8beacbf5685f88b64561627
}
read_file() {
  case "$1" in
    usr/local/sbin/octessera-wifi-foundation) cat "$root/userpatches/overlay/usr/local/sbin/octessera-wifi-foundation" ;;
    etc/systemd/system/octessera-wifi-foundation.service) cat "$root/userpatches/overlay/etc/systemd/system/octessera-wifi-foundation.service" ;;
    usr/local/bin/wifi-connect) return 0 ;;
    usr/local/share/doc/octessera/wifi-connect/wifi-connect.metadata.json)
      printf '%s\n' '"binary_sha256": "4a6ea81ad10a199064c2c9bf3f2b9fa39daadff3d8beacbf5685f88b64561627"' '"patch_sha256": "c9538ec7428b37c29fdfbe738cb10913a1036247270616c062228d8066f98dc6"'
      ;;
    *) return 1 ;;
  esac
}
octessera_require_wifi_foundation
