#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"

if command -v cargo >/dev/null 2>&1; then
  cargo_bin=(cargo)
elif command -v cargo.exe >/dev/null 2>&1; then
  cargo_bin=(cargo.exe)
else
  printf 'cargo is required on PATH.\n' >&2
  exit 1
fi

if ! "${cargo_bin[@]}" llvm-cov --version >/dev/null 2>&1; then
  printf 'cargo-llvm-cov is required. Install it with `cargo install cargo-llvm-cov` and ensure `llvm-tools-preview` is available.\n' >&2
  exit 1
fi

threshold="${1:-80}"
packages=(
  platform-core
  playback-runtime
  realtime-engine
)

for package in "${packages[@]}"; do
  printf '\n--- Rust coverage: %s ---\n' "$package"
  "${cargo_bin[@]}" llvm-cov --package "$package" --summary-only --fail-under-lines "$threshold"
done
