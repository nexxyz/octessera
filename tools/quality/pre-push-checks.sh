# shellcheck shell=bash
# Check definitions for tools/quality/pre-push.sh. Sourced, not executed.
# Defines run_pre_push_checks <profile> which calls the runner's run_check helper.

check_file_length() {
  local root max=500 failed=0 file rel lines
  root="$(git rev-parse --show-toplevel)"
  while IFS= read -r -d '' file; do
    rel="${file#"$root"/}"
    lines="$(wc -l < "$file")"
    if [ "$lines" -gt "$max" ]; then
      printf '  ✗ %s (%s lines, max %s)\n' "$rel" "$lines" "$max" >&2
      failed=1
    fi
  done < <(find "$root" \
    \( -name '*.rs' -o -name '*.ts' -o -name '*.tsx' -o -name '*.mjs' \) \
    -not -path '*/target/*' -not -path '*/node_modules/*' -not -path '*/dist/*' \
    -not -path '*/.git/*' -not -path '*/third_party/cpal-0.15.3/*' -print0)
  return "$failed"
}

run_pre_push_checks() {
  local profile="$1"
  if [ "$profile" = fast ]; then
    run_check "pnpm lint" corepack pnpm run lint
    run_check "pnpm typecheck" corepack pnpm run typecheck
    run_check "pnpm format:check" corepack pnpm run format:check
    run_check "cargo fmt" cargo fmt --all --check
    run_check "file length" check_file_length
    return 0
  fi

  run_check "pnpm lint" corepack pnpm run lint
  run_check "pnpm typecheck" corepack pnpm run typecheck
  run_check "pnpm format:check" corepack pnpm run format:check
  run_check "pnpm test" corepack pnpm run test
  run_check "pnpm test:coverage" corepack pnpm run test:coverage
  run_check "cargo fmt" cargo fmt --all --check
  run_check "file length" check_file_length
  run_check "cargo test" \
    cargo test --workspace --exclude octessera-desktop --exclude rodio-engine-source
  run_check "factory patch UI scenario" \
    cargo test -p playback-runtime factory_patch_ui_scenario -- --ignored
  run_check "cargo llvm-cov" bash ./tools/quality/check-rust-coverage.sh
  run_check "cargo check desktop" cargo check -p octessera-desktop
  run_check "cargo test desktop" cargo test -p octessera-desktop
  run_check "cargo check pi" cargo check -p octessera-pi
  run_check "tauri build smoke" corepack pnpm run tauri:build:ci
  run_check "cargo clippy" \
    cargo clippy --workspace --exclude octessera-desktop --exclude rodio-engine-source \
    --all-targets -- -D warnings
}
