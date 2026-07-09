#!/usr/bin/env bash
set -euo pipefail

TARGET="${TARGET:-aarch64-unknown-linux-gnu}"
PROFILE="${PROFILE:-pi-dev}"
OUT_DIR="${OUT_DIR:-target/pi-cross}"
IMAGE="${IMAGE:-octessera-pi-cross:latest}"

docker build -f Dockerfile.pi-zero -t "$IMAGE" .
docker run --rm \
  -v "$PWD:/work" \
  -v octessera-pi-cargo-registry:/usr/local/cargo/registry \
  -v octessera-pi-cargo-git:/usr/local/cargo/git \
  -v octessera-pi-rustup:/usr/local/rustup \
  -w /work \
  -e CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
  -e PKG_CONFIG_PATH=/usr/lib/aarch64-linux-gnu/pkgconfig/ \
  -e PKG_CONFIG_ALLOW_CROSS=1 \
  "$IMAGE" \
  bash -lc "set -euo pipefail; export PATH=/usr/local/cargo/bin:\$PATH; if command -v rustup >/dev/null 2>&1; then rustup target add '$TARGET'; fi; cargo build --target '$TARGET' --profile '$PROFILE' -p octessera-pi --features hardware-pi; mkdir -p '$OUT_DIR'; cp target/'$TARGET'/'$PROFILE'/octessera-pi '$OUT_DIR'/octessera-pi"
