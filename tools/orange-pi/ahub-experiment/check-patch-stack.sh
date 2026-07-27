#!/bin/sh
set -eu

HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH='' cd -- "$HERE/../../.." && pwd)
PYTHON=${PYTHON:-python3}
KERNEL_SOURCE=
KERNEL_COMMIT=e46dc0adfe39724bcf52cea47b8f9c9aed86a394
ARMBIAN_SOURCE_DIR=${ARMBIAN_SOURCE_DIR:-$REPO_ROOT/.slim/clonedeps/repos/armbian__build}
PATCH_DIR=patch/kernel/archive/sunxi-6.18
TEMP_ROOT=
WORKTREE=

die() {
	printf '%s\n' "AHUB patch check: $*" >&2
	exit 1
}

usage() {
	cat >&2 <<'EOF'
usage:
  check-patch-stack.sh --kernel-source /absolute/path/to/linux
EOF
	exit 2
}

while [ "$#" -gt 0 ]; do
	case "$1" in
		--kernel-source)
			[ "$#" -ge 2 ] || die "--kernel-source needs a value"
			KERNEL_SOURCE=$2
			shift 2
			;;
		*) usage ;;
	esac
done

[ -n "$KERNEL_SOURCE" ] || usage
case "$KERNEL_SOURCE" in
	/*) ;;
	*) die "--kernel-source must be absolute" ;;
esac

command -v "$PYTHON" >/dev/null 2>&1 || die "python3 is required"
command -v git >/dev/null 2>&1 || die "git is required"
case "$ARMBIAN_SOURCE_DIR" in
	/*) ;;
	*) die "ARMBIAN_SOURCE_DIR must be absolute" ;;
esac
ARMBIAN_SOURCE_DIR=$(CDPATH='' cd -- "$ARMBIAN_SOURCE_DIR" && pwd -P) || die "Armbian source is not accessible"
ARMBIAN_SOURCE_DIR="$ARMBIAN_SOURCE_DIR" "$PYTHON" "$HERE/preflight.py" "$HERE"

KERNEL_SOURCE=$(CDPATH='' cd -- "$KERNEL_SOURCE" && pwd -P) || die "kernel source is not accessible"
[ "$(git -C "$KERNEL_SOURCE" rev-parse HEAD)" = "$KERNEL_COMMIT" ] || die "kernel source HEAD is not pinned to $KERNEL_COMMIT"

TEMP_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/octessera-ahub-patch-check.XXXXXX")
cleanup() {
	if [ -n "$WORKTREE" ]; then
		git -C "$KERNEL_SOURCE" worktree remove --force "$WORKTREE" >/dev/null 2>&1 || true
	fi
	rm -rf -- "$TEMP_ROOT"
}
trap cleanup EXIT HUP INT TERM

WORKTREE=$TEMP_ROOT/linux
git -C "$KERNEL_SOURCE" worktree add --detach "$WORKTREE" "$KERNEL_COMMIT" >/dev/null

apply_patch() {
	patch_path=$1
	git -C "$WORKTREE" apply --check --ignore-space-change --ignore-whitespace --whitespace=nowarn "$patch_path" || die "patch dry-run failed: $patch_path"
	git -C "$WORKTREE" apply --ignore-space-change --ignore-whitespace --whitespace=nowarn "$patch_path" || die "patch application failed: $patch_path"
}

while IFS= read -r line || [ -n "$line" ]; do
	patch_entry=$(printf '%s' "$line" | sed 's/^[[:space:]]*//; s/[[:space:]]*$//')
	case "$patch_entry" in
		''|\#*|-*) continue ;;
	esac
	apply_patch "$ARMBIAN_SOURCE_DIR/$PATCH_DIR/$patch_entry"
done < "$ARMBIAN_SOURCE_DIR/$PATCH_DIR/series.conf"

printf '%s\n' "AHUB full Armbian patch stack applies cleanly to Linux $KERNEL_COMMIT"
