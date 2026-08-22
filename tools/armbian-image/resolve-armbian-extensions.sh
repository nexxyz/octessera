#!/usr/bin/env bash
set -euo pipefail

extensions="${1-}"
for mandatory_extension in octessera_midi octessera_audio octessera_image_sanitize; do
    if [[ ! "$extensions" =~ (^|[[:space:],])${mandatory_extension}([[:space:],]|$) ]]; then
        extensions="${extensions:+$extensions }$mandatory_extension"
    fi
done
printf '%s\n' "$extensions"
