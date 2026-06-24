#!/usr/bin/env bash
set -euo pipefail

TARGET="${TARGET:-aarch64-unknown-linux-gnu}"
PROFILE="${PROFILE:-pi-dev}"
OUT_DIR="${OUT_DIR:-target/pi-cross}"
IMAGE="${IMAGE:-cellsymphony-pi-cross:latest}"

docker build -f Dockerfile.pi-zero -t "$IMAGE" .
docker run --rm \
  -v "$PWD:/work" \
  -v cellsymphony-pi-cargo-registry:/usr/local/cargo/registry \
  -v cellsymphony-pi-cargo-git:/usr/local/cargo/git \
  -v cellsymphony-pi-rustup:/usr/local/rustup \
  -w /work \
  -e CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
  -e PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig/ \
  -e PKG_CONFIG_ALLOW_CROSS=1 \
  "$IMAGE" \
  bash -lc "set -euo pipefail; export PATH=/usr/local/cargo/bin:\$PATH; if command -v rustup >/dev/null 2>&1; then rustup target add '$TARGET'; fi; cargo build --target '$TARGET' --profile '$PROFILE' -p cellsymphony-pi --features hardware-pi; mkdir -p '$OUT_DIR'; cp target/'$TARGET'/'$PROFILE'/cellsymphony-pi '$OUT_DIR'/cellsymphony-pi"
