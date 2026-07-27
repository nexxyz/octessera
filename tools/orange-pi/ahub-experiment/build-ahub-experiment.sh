#!/bin/sh
set -eu

HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH='' cd -- "$HERE/../../.." && pwd)
SOURCE_DIR=${ARMBIAN_SOURCE_DIR:-$REPO_ROOT/.slim/clonedeps/repos/armbian__build}
PYTHON=${PYTHON:-python3}
MODE=
OUTPUT_DIR=
KEEP_WORK=0
COMMIT=166b786fc978d88f4ff9ee3e33c353afb39763e8
PATCH_DIR=patch/kernel/archive/sunxi-6.12
STAGED_PATCH_DIR_NAME=archive/sunxi-6.12
PACKAGE_INPUT_HOOK_REL=build-hooks/normalize-kernel-package-input.patch
PACKAGE_INPUT_HOOK_TARGET=lib/functions/compilation/kernel-debs.sh
KERNEL_HEADERS_DISABLE_EXTENSION_REL=extensions/ahub-disable-kernel-headers.sh
KERNEL_HEADERS_DISABLE_EXTENSION_NAME=ahub-disable-kernel-headers
KERNEL_HEADERS_DISABLE_HOOK_POINT=extension_finish_config
KERNEL_HEADERS_OPTION=KERNEL_HAS_WORKING_HEADERS=no
KERNEL_HEADERS_INSTALL_OPTION=INSTALL_HEADERS=no
KERNEL_PACKAGE_GLOB='linux-image-*.deb'
DTB_PACKAGE_GLOB='linux-dtb-*.deb'
MODULE_PACKAGE_GLOB='linux-modules-*.deb'
HEADERS_PACKAGE_GLOB='linux-headers-*.deb'
RUNTIME_OUTPUT_VALIDATOR=required-image-and-dtb-optional-modules-headers-forbidden
USER_PATCH_DIR=
WORKTREE=
TEMP_ROOT=
TMP_PARENT=
ARTIFACTS_STAGED=0

die() {
	printf '%s\n' "AHUB build: $*" >&2
	exit 1
}

usage() {
	cat >&2 <<'EOF'
usage:
  build-ahub-experiment.sh --test
  build-ahub-experiment.sh --dry-run
  build-ahub-experiment.sh --run-kernel --output /absolute/path/out [--keep-work]
EOF
	exit 2
}

while [ "$#" -gt 0 ]; do
	case "$1" in
		--test|--dry-run|--run-kernel)
			[ -z "$MODE" ] || die "choose exactly one of --test, --dry-run, or --run-kernel"
			MODE=${1#--}
			shift
			;;
		--output)
			[ "$#" -ge 2 ] || die "--output needs a value"
			OUTPUT_DIR=$2
			shift 2
			;;
		--keep-work)
			KEEP_WORK=1
			shift
			;;
		*) usage ;;
	esac
done

[ -n "$MODE" ] || usage
if [ "$MODE" = run-kernel ] && [ -z "$OUTPUT_DIR" ]; then
	die "--run-kernel requires --output"
fi

command -v "$PYTHON" >/dev/null 2>&1 || die "python3 is required"
command -v git >/dev/null 2>&1 || die "git is required"
command -v sha256sum >/dev/null 2>&1 || die "sha256sum is required"
command -v patch >/dev/null 2>&1 || die "patch is required"
[ -n "$SOURCE_DIR" ] || die "Armbian source path is empty"
case "$SOURCE_DIR" in
	/*) ;;
	*) die "ARMBIAN_SOURCE_DIR must be absolute" ;;
esac
SOURCE_DIR=$(CDPATH='' cd -- "$SOURCE_DIR" && pwd -P) || die "Armbian source path is not accessible"

TMP_PARENT=${TMPDIR:-/tmp}
case "$TMP_PARENT" in
	/*) ;;
	*) die "TMPDIR must be an absolute path" ;;
esac
TMP_PARENT=$(CDPATH='' cd -- "$TMP_PARENT" && pwd -P) || die "TMPDIR is not an accessible directory"
case "$TMP_PARENT" in
	"$REPO_ROOT"|"$REPO_ROOT/"*) die "TMPDIR must be outside the repository" ;;
esac

case "$COMMIT" in
	''|*[!0-9a-f]*) die "launcher commit contains non-hex characters" ;;
esac
[ "$(printf '%s' "$COMMIT" | wc -c | tr -d ' ')" -eq 40 ] || die "launcher commit is not an immutable 40-character ref"

if [ "$MODE" = run-kernel ]; then
	case "$OUTPUT_DIR" in
		/*) ;;
		*) die "--output must be an absolute path" ;;
	esac
	case "$OUTPUT_DIR/" in
		"$REPO_ROOT/"*|"$HERE/"*) die "--output must be outside the repository" ;;
	esac
fi

ARMBIAN_SOURCE_DIR="$SOURCE_DIR" "$PYTHON" "$HERE/preflight.py" "$HERE"
[ -d "$SOURCE_DIR" ] || die "pinned local Armbian source is missing: $SOURCE_DIR"
[ "$(git -C "$SOURCE_DIR" rev-parse HEAD)" = "$COMMIT" ] || die "Armbian source HEAD is not pinned"
[ "$(git -C "$SOURCE_DIR" rev-parse --verify "$COMMIT^{commit}")" = "$COMMIT" ] || die "pinned commit is not a commit object"

TEMP_ROOT=$(mktemp -d "$TMP_PARENT/octessera-ahub-build.XXXXXX")
is_safe_temp_root() {
	case "$TEMP_ROOT" in
		"$TMP_PARENT"/octessera-ahub-build.[A-Za-z0-9]*) return 0 ;;
		*) return 1 ;;
	esac
}

cleanup_temp() {
	[ "$KEEP_WORK" -eq 0 ] || return 0
	is_safe_temp_root || {
		printf '%s\n' "AHUB build: refusing to remove unverified temporary path: $TEMP_ROOT" >&2
		return 1
	}
	if rm -rf -- "$TEMP_ROOT"; then
		return 0
	fi
	if command -v sudo >/dev/null 2>&1 && sudo -n rm -rf -- "$TEMP_ROOT"; then
		return 0
	fi
	printf '%s\n' "AHUB build: temporary tree retained after cleanup failure: $TEMP_ROOT" >&2
	return 1
}

cleanup_on_exit() {
	status=$?
	cleanup_status=0
	cleanup_temp || cleanup_status=$?
	if [ "$status" -eq 0 ] && [ "$cleanup_status" -ne 0 ]; then
		if [ "$ARTIFACTS_STAGED" -eq 1 ]; then
			printf '%s\n' "AHUB build: artifacts staged; retaining temporary tree after cleanup failure" >&2
		else
			status=$cleanup_status
		fi
	fi
	exit "$status"
}

trap cleanup_on_exit EXIT
trap 'exit 129' HUP INT TERM
WORKTREE=$TEMP_ROOT/armbian-build

git clone --no-local "$SOURCE_DIR" "$WORKTREE" >/dev/null
git -C "$WORKTREE" checkout --detach "$COMMIT" >/dev/null
[ "$(git -C "$WORKTREE" rev-parse HEAD)" = "$COMMIT" ] || die "temporary build source is not pinned"
[ -z "$(git -C "$WORKTREE" symbolic-ref -q --short HEAD || true)" ] || die "temporary build source is not detached"

USER_PATCH_DIR=$WORKTREE/userpatches/kernel/$STAGED_PATCH_DIR_NAME
CORE_PATCH_DIR=$WORKTREE/$PATCH_DIR
USER_BUILD_HOOK_DIR=$WORKTREE/userpatches/build-hooks
USER_EXTENSION_DIR=$WORKTREE/userpatches/extensions
mkdir -p "$USER_PATCH_DIR/overlay_64" "$WORKTREE/userpatches/config/kernel" "$USER_BUILD_HOOK_DIR" "$USER_EXTENSION_DIR"

SERIES_PATH=$CORE_PATCH_DIR/series.conf
[ -f "$SERIES_PATH" ] || die "pinned full source series.conf is missing"
[ "$(git -C "$WORKTREE" ls-tree -r --name-only "$COMMIT" -- "$PATCH_DIR/series.conf")" = "$PATCH_DIR/series.conf" ] || die "pinned source series.conf is not tracked"
SOURCE_PATCH_COUNT=$($PYTHON - "$SERIES_PATH" <<'PY'
import pathlib
import sys

paths = []
for line in pathlib.Path(sys.argv[1]).read_text(encoding="utf-8").splitlines():
    value = line.strip()
    if value and not value.startswith("#") and not value.startswith("-"):
        paths.append(value)
print(len(paths))
PY
)
[ "$SOURCE_PATCH_COUNT" = 458 ] || die "pinned full source series patch count changed"
PACKAGE_INPUT_HOOK_SOURCE=$HERE/$PACKAGE_INPUT_HOOK_REL
PACKAGE_INPUT_HOOK=$USER_BUILD_HOOK_DIR/normalize-kernel-package-input.patch
cp "$PACKAGE_INPUT_HOOK_SOURCE" "$PACKAGE_INPUT_HOOK"
[ "$(sha256sum "$PACKAGE_INPUT_HOOK" | cut -d ' ' -f1)" = "$(sha256sum "$PACKAGE_INPUT_HOOK_SOURCE" | cut -d ' ' -f1)" ] || die "staged package hook hash changed"
patch --dry-run --batch --forward -p1 -d "$WORKTREE" < "$PACKAGE_INPUT_HOOK" >/dev/null || die "package input hook dry-run failed"
patch --batch --forward -p1 -d "$WORKTREE" < "$PACKAGE_INPUT_HOOK" >/dev/null || die "package input hook application failed"
$PYTHON - "$WORKTREE/$PACKAGE_INPUT_HOOK_TARGET" <<'PY'
import pathlib
import sys

lines = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
normalizer = [index for index, line in enumerate(lines) if line == "function ahub_normalize_kernel_package_inputs() {"]
callback = [index for index, line in enumerate(lines) if line == "function kernel_package_callback_linux_image() {"]
calls = [index for index, line in enumerate(lines) if line.strip() == "ahub_normalize_kernel_package_inputs"]
create = [index for index, line in enumerate(lines) if line.strip().startswith("create_kernel_deb ") and "kernel_package_callback_linux_image" in line]
if len(normalizer) != 1 or len(callback) != 1 or len(calls) != 1 or len(create) != 1 or normalizer[0] >= calls[0] or calls[0] != create[0] - 1 or calls[0] >= callback[0]:
    raise SystemExit("package input hook placement changed")
PY
KERNEL_HEADERS_DISABLE_EXTENSION_SOURCE=$HERE/$KERNEL_HEADERS_DISABLE_EXTENSION_REL
KERNEL_HEADERS_DISABLE_EXTENSION=$USER_EXTENSION_DIR/ahub-disable-kernel-headers.sh
cp "$KERNEL_HEADERS_DISABLE_EXTENSION_SOURCE" "$KERNEL_HEADERS_DISABLE_EXTENSION"
[ "$(sha256sum "$KERNEL_HEADERS_DISABLE_EXTENSION" | cut -d ' ' -f1)" = "$(sha256sum "$KERNEL_HEADERS_DISABLE_EXTENSION_SOURCE" | cut -d ' ' -f1)" ] || die "staged kernel headers extension hash changed"
[ "$(grep -c '^function extension_finish_config__ahub_disable_kernel_headers() {' "$KERNEL_HEADERS_DISABLE_EXTENSION")" -eq 1 ] || die "kernel headers extension hook changed"
[ "$(grep -c 'KERNEL_HAS_WORKING_HEADERS=\"no\"' "$KERNEL_HEADERS_DISABLE_EXTENSION")" -eq 1 ] || die "kernel headers option changed"
[ "$(grep -c 'INSTALL_HEADERS=\"no\"' "$KERNEL_HEADERS_DISABLE_EXTENSION")" -eq 1 ] || die "kernel headers install option changed"
cat > "$USER_PATCH_DIR/0000.patching_config.yaml" <<'EOF'
config:
  overlay-directories:
    - { source: "overlay_64", target: "arch/arm64/boot/dts/allwinner/overlay" }
EOF
cp "$HERE/Kconfig.fragment" "$WORKTREE/userpatches/config/kernel/linux-sunxi64-current.config"
cp "$HERE/octessera-ahub0-pi123-overlay.dts" "$USER_PATCH_DIR/overlay_64/octessera-ahub0-pi123.dtso"

STAGED_CONFIG=$WORKTREE/userpatches/config/kernel/linux-sunxi64-current.config
STAGED_OVERLAY=$USER_PATCH_DIR/overlay_64/octessera-ahub0-pi123.dtso
[ "$(sha256sum "$STAGED_CONFIG" | cut -d ' ' -f1)" = "$(sha256sum "$HERE/Kconfig.fragment" | cut -d ' ' -f1)" ] || die "staged Kconfig hash changed"
[ "$(sha256sum "$STAGED_OVERLAY" | cut -d ' ' -f1)" = "$(sha256sum "$HERE/octessera-ahub0-pi123-overlay.dts" | cut -d ' ' -f1)" ] || die "staged overlay hash changed"
[ -f "$USER_PATCH_DIR/0000.patching_config.yaml" ] || die "user patch configuration is missing"
grep -F 'overlay-directories:' "$USER_PATCH_DIR/0000.patching_config.yaml" >/dev/null || die "overlay merge configuration is missing"
grep -F '{ source: "overlay_64", target: "arch/arm64/boot/dts/allwinner/overlay" }' "$USER_PATCH_DIR/0000.patching_config.yaml" >/dev/null || die "overlay merge target changed"
printf '%s\n' "source_patch_dir=$CORE_PATCH_DIR" "source_series=$SERIES_PATH" "source_series_patch_count=$SOURCE_PATCH_COUNT" "user_patch_dir=$USER_PATCH_DIR" "user_overlay=$STAGED_OVERLAY" "package_input_hook=$PACKAGE_INPUT_HOOK" "package_input_hook_target=$WORKTREE/$PACKAGE_INPUT_HOOK_TARGET" "package_input_hook_placement=before_kernel_package_callback_linux_image" "kernel_headers_disable_extension=$KERNEL_HEADERS_DISABLE_EXTENSION" "kernel_headers_disable_hook_point=$KERNEL_HEADERS_DISABLE_HOOK_POINT" "kernel_headers_option=$KERNEL_HEADERS_OPTION" "kernel_headers_install_option=$KERNEL_HEADERS_INSTALL_OPTION"

BUILD_COMMAND="EXT=$KERNEL_HEADERS_DISABLE_EXTENSION_NAME ./compile.sh kernel BOARD=orangepizero2w BRANCH=current KERNELPATCHDIR=$STAGED_PATCH_DIR_NAME KERNEL_CONFIGURE=no KERNEL_KEEP_CONFIG=no NON_INTERACTIVE=yes"

validate_kernel_output() {
	output_root=$1
	debs_dir=$output_root/debs
	config_artifact=$output_root/ahub-experiment/linux-sunxi64-current.config
	[ -d "$debs_dir" ] || die "Armbian kernel package output directory is missing: $debs_dir"
	image_packages=$(find "$debs_dir" -maxdepth 1 -type f -name "$KERNEL_PACKAGE_GLOB" -printf '%f\n' | LC_ALL=C sort)
	dtb_packages=$(find "$debs_dir" -maxdepth 1 -type f -name "$DTB_PACKAGE_GLOB" -printf '%f\n' | LC_ALL=C sort)
	module_packages=$(find "$debs_dir" -maxdepth 1 -type f -name "$MODULE_PACKAGE_GLOB" -printf '%f\n' | LC_ALL=C sort)
	header_packages=$(find "$debs_dir" -maxdepth 1 -type f -name "$HEADERS_PACKAGE_GLOB" -printf '%f\n' | LC_ALL=C sort)
	image_count=$(printf '%s\n' "$image_packages" | sed '/^$/d' | wc -l | tr -d ' ')
	dtb_count=$(printf '%s\n' "$dtb_packages" | sed '/^$/d' | wc -l | tr -d ' ')
	header_count=$(printf '%s\n' "$header_packages" | sed '/^$/d' | wc -l | tr -d ' ')
	[ "$image_count" -eq 1 ] || die "expected exactly one generated linux-image package, got $image_count"
	image_package=$(printf '%s\n' "$image_packages" | sed '/^$/d')
	[ -s "$debs_dir/$image_package" ] || die "generated linux-image package is empty: $image_package"
	[ "$dtb_count" -gt 0 ] || die "no linux-dtb package was produced"
	[ "$header_count" -eq 0 ] || die "linux-headers packages are forbidden for the deploy-only experiment: $header_packages"
	for package in $dtb_packages $module_packages; do
		[ -s "$debs_dir/$package" ] || die "generated kernel package is empty: $package"
	done
	[ -f "$config_artifact" ] || die "built kernel config artifact is missing"
	[ "$(sha256sum "$config_artifact" | cut -d ' ' -f1)" = "$(sha256sum "$STAGED_CONFIG" | cut -d ' ' -f1)" ] || die "built kernel config artifact differs from staged config"
	printf '%s\n' "generated_linux_image_package=$image_package"
	printf '%s\n' "generated_linux_dtb_packages=$dtb_packages"
	printf '%s\n' "generated_linux_modules_packages=$module_packages"
	printf '%s\n' "linux_headers_packages=none-by-design"
}

if [ "$MODE" = test ]; then
	TEST_OUTPUT=$TEMP_ROOT/test-output
	mkdir -p "$TEST_OUTPUT/debs" "$TEST_OUTPUT/ahub-experiment"
	printf '%s\n' fixture > "$TEST_OUTPUT/debs/linux-image-current-sunxi64-test.deb"
	printf '%s\n' fixture > "$TEST_OUTPUT/debs/linux-dtb-test.deb"
	printf '%s\n' fixture > "$TEST_OUTPUT/debs/linux-modules-current-sunxi64-test.deb"
	cp "$STAGED_CONFIG" "$TEST_OUTPUT/ahub-experiment/linux-sunxi64-current.config"
	validate_kernel_output "$TEST_OUTPUT"
	ARTIFACTS_STAGED=1
	printf '%s\n' "test mode validated pinned full series, overlay merge, config, and kernel packages"
elif [ "$MODE" = dry-run ]; then
	printf '%s\n' "source=$SOURCE_DIR"
	printf '%s\n' "source_commit=$COMMIT"
	printf '%s\n' "source_patch_dir=$CORE_PATCH_DIR"
	printf '%s\n' "source_series=$SERIES_PATH"
	printf '%s\n' "source_series_patch_count=$SOURCE_PATCH_COUNT"
	printf '%s\n' "kernel_config=$STAGED_CONFIG"
	printf '%s\n' "user_patch_dir=$USER_PATCH_DIR"
	printf '%s\n' "user_overlay=$STAGED_OVERLAY"
	printf '%s\n' "package_input_hook=$PACKAGE_INPUT_HOOK"
	printf '%s\n' "package_input_hook_target=$WORKTREE/$PACKAGE_INPUT_HOOK_TARGET"
	printf '%s\n' "package_input_hook_placement=before_kernel_package_callback_linux_image"
	printf '%s\n' "kernel_headers_disable_extension=$KERNEL_HEADERS_DISABLE_EXTENSION"
	printf '%s\n' "kernel_headers_disable_hook_point=$KERNEL_HEADERS_DISABLE_HOOK_POINT"
	printf '%s\n' "kernel_headers_option=$KERNEL_HEADERS_OPTION"
	printf '%s\n' "kernel_headers_install_option=$KERNEL_HEADERS_INSTALL_OPTION"
	printf '%s\n' "kernel_package_glob=$KERNEL_PACKAGE_GLOB"
	printf '%s\n' "dtb_package_glob=$DTB_PACKAGE_GLOB"
	printf '%s\n' "module_package_glob=$MODULE_PACKAGE_GLOB"
	printf '%s\n' "headers_package_glob=$HEADERS_PACKAGE_GLOB"
	printf '%s\n' "runtime_output_validator=$RUNTIME_OUTPUT_VALIDATOR"
	printf '%s\n' "temporary_userpatches=$WORKTREE/userpatches"
	printf '%s\n' "build_command=cd $WORKTREE && $BUILD_COMMAND"
	printf '%s\n' "output=$OUTPUT_DIR"
else
	mkdir -p "$OUTPUT_DIR"
	[ -z "$(find "$OUTPUT_DIR" -mindepth 1 -maxdepth 1 -print -quit)" ] || die "--output must be empty"
	( cd "$WORKTREE" && sh -c "$BUILD_COMMAND" )
	mkdir -p "$WORKTREE/output/ahub-experiment"
	cp "$STAGED_CONFIG" "$WORKTREE/output/ahub-experiment/linux-sunxi64-current.config"
	validate_kernel_output "$WORKTREE/output"
	cp -a "$WORKTREE/output/debs/." "$OUTPUT_DIR/"
	cp "$WORKTREE/output/ahub-experiment/linux-sunxi64-current.config" "$OUTPUT_DIR/"
	sha256sum "$OUTPUT_DIR"/* > "$OUTPUT_DIR/SHA256SUMS"
	ARTIFACTS_STAGED=1
	printf '%s\n' "experimental Armbian kernel packages written to $OUTPUT_DIR"
fi
