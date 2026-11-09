#!/usr/bin/env bash
# Installs the pinned cross release into the given directory (CI runners).
# With GH_TOKEN set it downloads through the authenticated GitHub API, so it is not
# subject to anonymous-traffic limits; a set but invalid token fails. Without a token
# (local act runs, self-hosted runners) it falls back to an anonymous download.
set -euo pipefail

version=v0.2.5
asset=cross-x86_64-unknown-linux-gnu.tar.gz
sha256=642375d1bcf3bd88272c32ba90e999f3d983050adf45e66bd2d3887e8e838bad
dest="${1:?usage: install-cross.sh <destination-directory>}"

archive="$(mktemp)"
trap 'rm -f "$archive"' EXIT

if [[ -n "${GH_TOKEN:-}" ]]; then
  gh release download "$version" --repo cross-rs/cross --pattern "$asset" --output "$archive" --clobber
else
  curl --fail --location --proto '=https' --tlsv1.2 --output "$archive" \
    "https://github.com/cross-rs/cross/releases/download/$version/$asset"
fi
printf '%s  %s\n' "$sha256" "$archive" | sha256sum -c -

if [[ -w "$dest" ]]; then
  tar -xzf "$archive" -C "$dest"
else
  sudo tar -xzf "$archive" -C "$dest"
fi
"$dest/cross" --version
