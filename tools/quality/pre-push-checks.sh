# shellcheck shell=bash
# Check definitions for tools/quality/pre-push.sh. Sourced, not executed.
# Defines run_pre_push_checks <profile> which calls the runner's run_check command executor.

check_pi_linux_host_tests() {
  if ! command -v wsl.exe >/dev/null 2>&1 || ! MSYS2_ARG_CONV_EXCL='*' wsl.exe docker info >/dev/null 2>&1; then
    echo "Linux Pi host tests skipped: Docker/WSL unavailable; CI will run them" >&2
    return 0
  fi
  local root_wsl script
  root_wsl="$(MSYS2_ARG_CONV_EXCL='*' wsl.exe wslpath -u "$(cygpath -m "$(git rev-parse --show-toplevel)")" | tr -d '\r')"
  script='set -e
for feature in hardware-raspberry-pi-zero-2w hardware-orange-pi-zero-2w; do
  cargo test -p octessera-pi --no-default-features --features "$feature profiling"
  cargo clippy -p octessera-pi --no-default-features --features "$feature" --bin octessera-pi -- -W clippy::cognitive_complexity -D warnings
done'
  printf 'FROM rust:1.99.0-bookworm\nRUN rustup component add clippy rustfmt llvm-tools && rustup target add aarch64-unknown-linux-gnu && apt-get update && apt-get install -y --no-install-recommends pkg-config libasound2-dev && rm -rf /var/lib/apt/lists/*\n' |
    MSYS2_ARG_CONV_EXCL='*' wsl.exe docker build -q -t octessera-pi-host-tests - >/dev/null || return 1
  MSYS2_ARG_CONV_EXCL='*' wsl.exe docker run -i --rm \
    -v "$root_wsl:/src" -w /src \
    -v octessera-pi-host-target:/target -v octessera-pi-host-cargo:/usr/local/cargo/registry \
    -e CARGO_TARGET_DIR=/target \
    octessera-pi-host-tests bash -s <<<"$script"
}

run_pre_push_checks() {
  local profile="$1"
  if [ "$profile" = fast ]; then
    run_check "pnpm lint" corepack pnpm run lint
    run_check "pnpm typecheck" corepack pnpm run typecheck
    run_check "pnpm format:check" corepack pnpm run format:check
    run_check "cargo fmt" cargo fmt --all --check
    return 0
  fi

  run_check "pnpm lint" corepack pnpm run lint
  run_check "pnpm typecheck" corepack pnpm run typecheck
  run_check "pnpm format:check" corepack pnpm run format:check
  run_check "pnpm quality:test" corepack pnpm run quality:test
  run_check "pnpm capabilities:test" corepack pnpm run capabilities:test
  run_check "pnpm test:coverage" corepack pnpm run test:coverage
  run_check "cargo fmt" cargo fmt --all --check
  run_check "repository integrity fixtures" bash ./tools/quality/test-repository-integrity.sh
  run_check "repository integrity gate" bash ./tools/quality/repository-integrity.sh
  run_check "legal packaging tests" python3 -m unittest discover -s tools/legal -p 'test_*.py'
  run_check "cargo test" \
    cargo test --workspace --exclude octessera-desktop --exclude rodio-engine-source \
    --exclude platform-core --exclude playback-runtime --exclude realtime-engine \
    --features octessera-pi/profiling
  run_check "cargo test pi routing-tree executor" \
    cargo test -p realtime-engine -p rodio-engine-source --features rodio-engine-source/routing-tree-executor
  run_check "factory patch UI scenario" \
    cargo test -p playback-runtime factory_patch_ui_scenario -- --ignored
  run_check "UI screen sweep" \
    cargo test -p playback-runtime every_walked_screen_fits_and_has_help -- --ignored
  run_check "cargo llvm-cov" bash ./tools/quality/check-rust-coverage.sh
  run_check "tauri build smoke" corepack pnpm run tauri:build:ci
  run_check "cargo test desktop" cargo test -p octessera-desktop
  run_check "desktop e2e" corepack pnpm run e2e
  run_check "cargo test pi production" cargo test -p octessera-pi
  run_check "linux pi host tests" check_pi_linux_host_tests
  run_check "cargo clippy production cognitive" \
    cargo clippy --workspace --lib --bins -- -W clippy::cognitive_complexity -D warnings
  run_check "cargo clippy" \
    cargo clippy --workspace --exclude octessera-desktop --exclude rodio-engine-source \
    --features octessera-pi/profiling --all-targets -- -D warnings
}
