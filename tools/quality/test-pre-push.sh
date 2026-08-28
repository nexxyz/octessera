#!/usr/bin/env bash
# Fixture-repository tests for tools/quality/pre-push.sh.
# Proves the runner never mutates the worktree, index, untracked files, or stash list.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUNNER="$ROOT/tools/quality/pre-push.sh"
HOOK="$ROOT/.githooks/pre-push"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

cat > "$TMP/checks.sh" <<'EOF'
run_pre_push_checks() {
  if [ "${PRE_PUSH_FAKE_FAIL:-0}" = "1" ]; then
    run_check "fake-check" false
  else
    run_check "fake-check" true
  fi
}
EOF

make_repo() {
  local dir="$1"
  mkdir -p "$dir"
  git -C "$dir" init -q
  git -C "$dir" config user.name "Fixture Tester"
  git -C "$dir" config user.email "fixture@example.invalid"
  printf 'hello\n' > "$dir/README.md"
  git -C "$dir" add README.md
  git -C "$dir" commit -q -m init
}

snapshot() {
  local dir="$1"
  {
    git -C "$dir" status --porcelain
    printf 'STASH_BEGIN\n'
    git -C "$dir" stash list
    printf 'STASH_END\n'
    sha256sum "$dir/.git/index"
    find "$dir" -path "$dir/.git" -prune -o -type f -print0 | sort -z | xargs -0 sha256sum
  }
}

PRE_RC=0
PRE_BEFORE=
PRE_AFTER=

run_prepush() {
  local repo="$1"
  local checks_file="${PRE_PUSH_CHECKS_FILE_OVERRIDE:-$TMP/checks.sh}"
  shift
  PRE_BEFORE="$(snapshot "$repo")"
  set +e
  (
    cd "$repo"
    PRE_PUSH_CHECKS_FILE="$checks_file" PRE_PUSH_FAKE_FAIL="${FAKE_FAIL:-0}" bash "$RUNNER" "$@"
  ) >"$TMP/last.out" 2>"$TMP/last.err"
  PRE_RC=$?
  set -e
  PRE_AFTER="$(snapshot "$repo")"
}

expect_rc() {
  local label="$1" expected="$2"
  if [ "$PRE_RC" -ne "$expected" ]; then
    printf 'FAIL[%s]: expected exit %s, got %s\n' "$label" "$expected" "$PRE_RC" >&2
    sed 's/^/  err: /' "$TMP/last.err" >&2
    exit 1
  fi
}

expect_err_match() {
  local label="$1" pattern="$2"
  if ! grep -q "$pattern" "$TMP/last.err"; then
    printf 'FAIL[%s]: stderr missing %s\n' "$label" "$pattern" >&2
    exit 1
  fi
}

expect_no_mutation() {
  local label="$1"
  if [ "$PRE_BEFORE" != "$PRE_AFTER" ]; then
    printf 'FAIL[%s]: worktree mutated\n' "$label" >&2
    diff <(printf '%s\n' "$PRE_BEFORE") <(printf '%s\n' "$PRE_AFTER") >&2 || true
    exit 1
  fi
}

expect_no_leftover_worktrees() {
  local label="$1" repo="$2" count
  count="$(git -C "$repo" worktree list --porcelain | grep -c '^worktree ')"
  if [ "$count" -ne 1 ]; then
    printf 'FAIL[%s]: expected 1 worktree, found %s\n' "$label" "$count" >&2
    exit 1
  fi
}

PASS_COUNT=0
pass() {
  PASS_COUNT=$((PASS_COUNT + 1))
  printf 'ok - %s\n' "$1"
}

# 1. Clean worktree, checks pass, default mode.
FAKE_FAIL=0
repo="$TMP/sc1"
make_repo "$repo"
run_prepush "$repo"
expect_rc "sc1" 0
expect_no_mutation "sc1"
expect_no_leftover_worktrees "sc1" "$repo"
pass "clean worktree passes checks in default mode"

# 2. Clean worktree, checks fail -> exit 1, no mutation.
FAKE_FAIL=1
repo="$TMP/sc2"
make_repo "$repo"
run_prepush "$repo"
expect_rc "sc2" 1
expect_no_mutation "sc2"
pass "failing checks exit 1 without mutation"

# 3. Dirty tracked modification refused by default.
FAKE_FAIL=0
repo="$TMP/sc3"
make_repo "$repo"
printf 'dirty\n' >> "$repo/README.md"
run_prepush "$repo"
expect_rc "sc3" 3
expect_err_match "sc3" "uncommitted changes"
expect_no_mutation "sc3"
pass "dirty tracked change refused in default mode"

# 4. Staged change refused, index untouched.
repo="$TMP/sc4"
make_repo "$repo"
printf 'staged\n' >> "$repo/README.md"
git -C "$repo" add README.md
run_prepush "$repo"
expect_rc "sc4" 3
expect_no_mutation "sc4"
pass "staged change refused and index untouched"

# 5. Untracked file refused and preserved (the old stash/pop could lose it).
repo="$TMP/sc5"
make_repo "$repo"
printf 'untracked\n' > "$repo/new.txt"
run_prepush "$repo"
expect_rc "sc5" 3
expect_no_mutation "sc5"
if [ ! -f "$repo/new.txt" ]; then
  printf 'FAIL[sc5]: untracked file lost\n' >&2
  exit 1
fi
pass "untracked file preserved on refusal"

# 6. Dirty worktree + --committed-tree validates HEAD in a temp worktree.
FAKE_FAIL=0
repo="$TMP/sc6"
make_repo "$repo"
printf 'dirty\n' >> "$repo/README.md"
run_prepush "$repo" --committed-tree
expect_rc "sc6" 0
expect_no_mutation "sc6"
expect_no_leftover_worktrees "sc6" "$repo"
pass "committed-tree run leaves main worktree intact and removes temp worktree"

# 7. Dirty worktree + --allow-dirty runs in place without mutation.
repo="$TMP/sc7"
make_repo "$repo"
printf 'dirty\n' >> "$repo/README.md"
run_prepush "$repo" --allow-dirty
expect_rc "sc7" 0
expect_no_mutation "sc7"
pass "allow-dirty run passes without mutation"

# 8. --fast profile accepted on a clean worktree.
FAKE_FAIL=0
repo="$TMP/sc8"
make_repo "$repo"
run_prepush "$repo" --fast
expect_rc "sc8" 0
expect_no_mutation "sc8"
pass "fast profile accepted"

# 9. Unknown flag is a usage error.
repo="$TMP/sc9"
make_repo "$repo"
run_prepush "$repo" --bogus
expect_rc "sc9" 2
expect_no_mutation "sc9"
pass "unknown flag exits 2 without mutation"

cat > "$TMP/file-length-checks.sh" <<EOF
source "$ROOT/tools/quality/pre-push-checks.sh"
run_pre_push_checks() {
  run_check "file length" check_file_length
}
EOF

write_fixture_lines() {
  local file="$1" count="$2" trailing_newline="${3:-yes}"
  : > "$file"
  for ((line = 1; line <= count; line += 1)); do
    printf 'fixture line %s' "$line" >> "$file"
    if [ "$line" -lt "$count" ] || [ "$trailing_newline" = yes ]; then
      printf '\n' >> "$file"
    fi
  done
}

# 10. Owned script extensions are included and generated/vendor/CAD artifacts are excluded.
FAKE_FAIL=0
PRE_PUSH_CHECKS_FILE_OVERRIDE="$TMP/file-length-checks.sh"
repo="$TMP/sc10"
make_repo "$repo"
mkdir -p "$repo/src"
for extension in bash js mjs ps1 psm1 py rs sh ts tsx; do
  printf 'source\n' > "$repo/src/included.$extension"
done
printf 'not scanned\n' > "$repo/src/excluded.txt"
for directory in .opencode .slim artifacts build gen generated release-artifacts target third_party vendor; do
  mkdir -p "$repo/$directory"
  write_fixture_lines "$repo/$directory/excluded.py" 501
done
mkdir -p "$repo/hardware/enclosure/review" "$repo/hardware/pcb/gerber"
write_fixture_lines "$repo/hardware/enclosure/review/excluded.py" 501
write_fixture_lines "$repo/hardware/pcb/gerber/excluded.py" 501
printf '#!/bin/sh\nsource\n' > "$repo/src/shebang-script"
write_fixture_lines "$repo/src/no-shebang" 501
write_fixture_lines "$repo/src/exact-500.py" 500
write_fixture_lines "$repo/src/exact-500-no-newline.sh" 500 no
deployment_root="$repo/tools/pi-image/stage4-octessera/files/root/usr/local/sbin"
mkdir -p "$deployment_root"
write_fixture_lines "$deployment_root/deployment-script" 500 no
mkdir -p "$repo/release-artifacts"
printf '#!/bin/sh\n' > "$repo/release-artifacts/artifact-script"
for ((line = 1; line <= 500; line += 1)); do
  printf 'artifact line %s\n' "$line" >> "$repo/release-artifacts/artifact-script"
done
run_prepush "$repo" --allow-dirty
expect_rc "sc10" 0
expect_no_mutation "sc10"
pass "owned extensions are included while generated/vendor/CAD fixtures are excluded"

# 11. Exact-500 scripts pass, then both newline forms fail at 501 logical lines.
write_fixture_lines "$repo/src/exact-500.py" 501
write_fixture_lines "$repo/src/exact-500-no-newline.sh" 501 no
run_prepush "$repo" --allow-dirty
expect_rc "sc11" 1
expect_err_match "sc11" "src/exact-500.py (501 lines, max 500)"
expect_err_match "sc11" "src/exact-500-no-newline.sh (501 lines, max 500)"
expect_no_mutation "sc11"
pass "exact-500 scripts pass and both 501-line forms are rejected"

# 12. The hook's opt-in environment forwards committed-tree mode.
FAKE_FAIL=0
PRE_PUSH_CHECKS_FILE_OVERRIDE="$TMP/checks.sh"
repo="$TMP/sc12"
make_repo "$repo"
mkdir -p "$repo/tools/quality"
cp "$RUNNER" "$repo/tools/quality/pre-push.sh"
printf 'dirty\n' >> "$repo/README.md"
PRE_BEFORE="$(snapshot "$repo")"
set +e
(
  cd "$repo"
  OCTESSERA_PRE_PUSH_COMMITTED_TREE=1 PRE_PUSH_CHECKS_FILE="$TMP/checks.sh" bash "$HOOK"
) >"$TMP/last.out" 2>"$TMP/last.err"
PRE_RC=$?
set -e
PRE_AFTER="$(snapshot "$repo")"
expect_rc "sc12" 0
expect_no_mutation "sc12"
expect_no_leftover_worktrees "sc12" "$repo"
pass "hook environment validates committed HEAD without touching dirty files"

printf '\nPASS: %d pre-push fixture scenarios\n' "$PASS_COUNT"
