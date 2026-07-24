#!/usr/bin/env bash
set -euo pipefail

TARGET="${TARGET:-aarch64-unknown-linux-gnu}"
PROFILE="${PROFILE:-pi-dev}"
BOARD_PROFILE="${BOARD_PROFILE:-raspberry-pi-zero-2w}"
OUT_DIR="${OUT_DIR:-target/pi-cross}"
IMAGE="${IMAGE:-octessera-pi-cross:latest}"

source ./tools/pi/board-profile.sh
require_raspberry_pi_board_profile "$BOARD_PROFILE"

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
  bash -lc "set -euo pipefail; export PATH=/usr/local/cargo/bin:\$PATH; if command -v rustup >/dev/null 2>&1; then rustup target add '$TARGET'; fi; cargo build --target '$TARGET' --profile '$PROFILE' -p octessera-pi --features hardware-raspberry-pi-zero-2w; mkdir -p '$OUT_DIR'; cp target/'$TARGET'/'$PROFILE'/octessera-pi '$OUT_DIR'/octessera-pi; printf '%s\n' '{\"schema_version\":1,\"board_profile\":\"raspberry-pi-zero-2w\",\"binary\":\"octessera-pi\",\"arch\":\"aarch64-unknown-linux-gnu\",\"cargo_feature\":\"hardware-raspberry-pi-zero-2w\"}' > '$OUT_DIR'/octessera-pi.metadata.json"
