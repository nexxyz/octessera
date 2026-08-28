#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
PROFILE=full
MODE=auto
PASS=0
FAIL=0

usage() {
  cat >&2 <<'EOF'
Usage: pre-push.sh [--fast] [--committed-tree] [--allow-dirty]

  --fast            run the fast check profile (no cargo tests/builds)
  --committed-tree  validate HEAD in a temporary worktree; safe when dirty
  --allow-dirty     run checks against the current worktree even if dirty

Default: full profile; refuses to run when the worktree has uncommitted changes.
EOF
}

if [ "$#" -gt 0 ]; then
  case "$1" in
    --fast) PROFILE=fast ;;
    --committed-tree) MODE=committed-tree ;;
    --allow-dirty) MODE=allow-dirty ;;
    -h|--help) usage; exit 0 ;;
    *) usage; exit 2 ;;
  esac
  shift
fi
if [ "$#" -gt 0 ]; then
  usage
  exit 2
fi

CHECKS_FILE="${PRE_PUSH_CHECKS_FILE:-$ROOT/tools/quality/pre-push-checks.sh}"
if [ ! -f "$CHECKS_FILE" ]; then
  echo "pre-push: checks file not found: $CHECKS_FILE" >&2
  exit 2
fi
# shellcheck source=/dev/null
source "$CHECKS_FILE"

cd "$ROOT"

run_check() {
  local name="$1"
  shift
  printf "\n--- %s ---\n" "$name"
  if "$@"; then
    printf "✓ %s passed\n" "$name"
    PASS=$((PASS + 1))
  else
    printf "✗ %s FAILED\n" "$name" >&2
    FAIL=$((FAIL + 1))
  fi
}

run_profile() {
  run_pre_push_checks "$PROFILE"
  printf "\n====================\n"
  printf "  %d passed, %d failed\n" "$PASS" "$FAIL"
  printf "====================\n"
  if [ "$FAIL" -gt 0 ]; then
    printf "\npre-push: some checks failed. Fix before pushing.\n" >&2
    return 1
  fi
  return 0
}

if [ "$MODE" = committed-tree ]; then
  TMP_DIR="$(mktemp -d)"
  WT_DIR="$TMP_DIR/wt"
  NODE_MODULES_JUNCTION=0
  cleanup() {
    if [ "$NODE_MODULES_JUNCTION" = 1 ]; then
      MSYS2_ARG_CONV_EXCL='*' cmd.exe /d /c rmdir "$(cygpath -w "$WT_DIR/node_modules")" >/dev/null 2>&1 || true
    fi
    git worktree remove --force "$WT_DIR" 2>/dev/null || true
    rm -rf "$TMP_DIR"
  }
  trap cleanup EXIT
  git worktree add --detach "$WT_DIR" HEAD >/dev/null
  if [ -d "$ROOT/node_modules" ]; then
    rm -rf -- "$WT_DIR/node_modules"
    case "$(uname -s)" in
      MINGW*|MSYS*|CYGWIN*)
        MSYS2_ARG_CONV_EXCL='*' cmd.exe /d /c mklink /J "$(cygpath -w "$WT_DIR/node_modules")" "$(cygpath -w "$ROOT/node_modules")" >/dev/null
        NODE_MODULES_JUNCTION=1
        ;;
      *) ln -s "$ROOT/node_modules" "$WT_DIR/node_modules" ;;
    esac
  fi
  if [ -d "$ROOT/target" ]; then
    export CARGO_TARGET_DIR="$ROOT/target"
  fi
  (
    unset GIT_DIR GIT_WORK_TREE
    cd "$WT_DIR" || exit 2
    run_profile
  )
else
  if [ "$MODE" = auto ] && [ -n "$(git status --porcelain)" ]; then
    echo "pre-push: refusing to run: the worktree has uncommitted changes (staged, modified, or untracked)." >&2
    echo "pre-push: commit first, or use --committed-tree (validates HEAD in a temp worktree) or --allow-dirty." >&2
    exit 3
  fi
  run_profile
fi
