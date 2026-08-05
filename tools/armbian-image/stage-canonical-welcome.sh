#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source_file="$root/tools/pi-image/stage4-octessera/files/root/etc/profile.d/octessera-welcome.sh"
overlay="${1:-$root/userpatches/overlay}"
destination="$overlay/etc/profile.d/octessera-welcome.sh"

[[ -f "$source_file" && ! -L "$source_file" ]] || { echo "Canonical welcome source is missing or symlinked: $source_file" >&2; exit 1; }
install -D -m 0644 "$source_file" "$destination"
cmp -s "$source_file" "$destination" || { echo "Staged welcome differs from canonical source." >&2; exit 1; }
