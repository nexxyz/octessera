#!/bin/sh
set -eu

HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH='' cd -- "$HERE/../../.." && pwd)
SOURCE_DIR=${ARMBIAN_SOURCE_DIR:-$REPO_ROOT/.slim/clonedeps/repos/armbian__build}
PYTHON=${PYTHON:-python3}
MODE=
OUTPUT_DIR=
KEEP_WORK=0
COMMIT=fa7a7b2294d9e760a77630950afd460b7a0b2a26
PACKAGE_REVISION=26.8.0-trunk.413
KERNEL_ABI=6.18.38-current-sunxi64
KERNEL_VERSION=6.18.38
PATCH_DIR=patch/kernel/archive/sunxi-6.18
STAGED_PATCH_DIR_NAME=archive/sunxi-6.18
SOURCE_CONFIG_REL=config/kernel/linux-sunxi64-current.config
KERNEL_PACKAGE_GLOB='linux-image-*.deb'
DTB_PACKAGE_GLOB='linux-dtb-*.deb'
MODULE_PACKAGE_GLOB='linux-modules-*.deb'
HEADERS_PACKAGE_GLOB='linux-headers-*.deb'
RUNTIME_OUTPUT_VALIDATOR=required-image-and-dtb-staged-only-native-extra-packages-permitted
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
[ "$MODE" = dry-run ] || command -v dpkg-deb >/dev/null 2>&1 || die "dpkg-deb is required"
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

CORE_PATCH_DIR=$WORKTREE/$PATCH_DIR
mkdir -p "$WORKTREE/userpatches/config/kernel"
SOURCE_CONFIG=$WORKTREE/$SOURCE_CONFIG_REL

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
[ "$SOURCE_PATCH_COUNT" = 515 ] || die "pinned full source series patch count changed"
[ -f "$SOURCE_CONFIG" ] || die "pinned complete source config is missing"
[ "$(wc -l < "$SOURCE_CONFIG" | tr -d ' ')" -gt 1000 ] || die "pinned source config is not complete"

STAGED_CONFIG=$WORKTREE/userpatches/config/kernel/linux-sunxi64-current.config
cp "$SOURCE_CONFIG" "$STAGED_CONFIG"
$PYTHON - "$STAGED_CONFIG" "$HERE/Kconfig.fragment" <<'PY'
import pathlib
import re
import sys

config_path = pathlib.Path(sys.argv[1])
override_path = pathlib.Path(sys.argv[2])
lines = config_path.read_text(encoding="utf-8").splitlines()
overrides = {}
for line in override_path.read_text(encoding="utf-8").splitlines():
    match = re.fullmatch(r"(CONFIG_[A-Za-z0-9_]+)=(y|m|n)", line)
    if match is None:
        match = re.fullmatch(r"# (CONFIG_[A-Za-z0-9_]+) is not set", line)
        if match is None:
            raise SystemExit(f"invalid config override: {line}")
        key, value = match.group(1), "n"
    else:
        key, value = match.groups()
    if key in overrides:
        raise SystemExit(f"duplicate config override: {key}")
    overrides[key] = value
if set(overrides) != {"CONFIG_SND_SOC_PCM5102A", "CONFIG_NVMEM_SUNXI_SID", "CONFIG_SUNXI_SYS_INFO"}:
    raise SystemExit("config override set changed")
for key, value in overrides.items():
    matches = [index for index, line in enumerate(lines) if re.fullmatch(rf"(?:{re.escape(key)}=(?:y|m|n)|# {re.escape(key)} is not set)", line)]
    replacement = f"# {key} is not set" if value == "n" else f"{key}={value}"
    if len(matches) > 1:
        raise SystemExit(f"duplicate source config entry: {key}")
    if matches:
        lines[matches[0]] = replacement
    else:
        lines.append(replacement)
config_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
PY
[ "$(wc -l < "$STAGED_CONFIG" | tr -d ' ')" -gt 1000 ] || die "staged kernel config is not complete"
printf '%s\n' "source_patch_dir=$CORE_PATCH_DIR" "source_series=$SERIES_PATH" "source_series_patch_count=$SOURCE_PATCH_COUNT" "source_config=$SOURCE_CONFIG" "kernel_config=$STAGED_CONFIG" "kernel_config_line_count=$(wc -l < "$STAGED_CONFIG" | tr -d ' ')" "package_revision=$PACKAGE_REVISION" "kernel_abi=$KERNEL_ABI"

BUILD_COMMAND="./compile.sh kernel REVISION=$PACKAGE_REVISION BOARD=orangepizero2w BRANCH=current KERNELPATCHDIR=$STAGED_PATCH_DIR_NAME KERNEL_CONFIGURE=no KERNEL_KEEP_CONFIG=no NON_INTERACTIVE=yes"

validate_package_metadata() {
	package=$1
	kind=$2
	package_name=$(dpkg-deb -f "$package" Package)
	package_version=$(dpkg-deb -f "$package" Version)
	package_architecture=$(dpkg-deb -f "$package" Architecture)
	[ "$package_version" = "$PACKAGE_REVISION" ] || die "package version changed: $package ($package_version)"
	[ "$package_architecture" = arm64 ] || die "package architecture changed: $package ($package_architecture)"
	if [ "$kind" = image ]; then
		[ "$package_name" = linux-image-current-sunxi64 ] || die "linux-image package identity changed: $package_name"
		[ "$(dpkg-deb -f "$package" Source)" = "linux-$KERNEL_VERSION" ] || die "linux-image source metadata changed"
		[ "$(dpkg-deb -f "$package" Armbian-Kernel-Version)" = "$KERNEL_VERSION" ] || die "linux-image kernel version metadata changed"
		[ "$(dpkg-deb -f "$package" Armbian-Kernel-Version-Family)" = "$KERNEL_ABI" ] || die "linux-image kernel family metadata changed"
	else
		[ "$package_name" = linux-dtb-current-sunxi64 ] || die "linux-dtb package identity changed: $package_name"
	fi
}

validate_kernel_output() {
	output_root=$1
	debs_dir=$output_root/debs
	config_artifact=$output_root/ahub-experiment/linux-sunxi64-current.config
	[ -d "$debs_dir" ] || die "Armbian kernel package output directory is missing: $debs_dir"
	command -v dpkg-deb >/dev/null 2>&1 || die "dpkg-deb is required to validate the packaged kernel"
	image_packages=$(find "$debs_dir" -maxdepth 1 -type f -name "$KERNEL_PACKAGE_GLOB" -printf '%f\n' | LC_ALL=C sort)
	dtb_packages=$(find "$debs_dir" -maxdepth 1 -type f -name "$DTB_PACKAGE_GLOB" -printf '%f\n' | LC_ALL=C sort)
	module_packages=$(find "$debs_dir" -maxdepth 1 -type f -name "$MODULE_PACKAGE_GLOB" -printf '%f\n' | LC_ALL=C sort)
	header_packages=$(find "$debs_dir" -maxdepth 1 -type f -name "$HEADERS_PACKAGE_GLOB" -printf '%f\n' | LC_ALL=C sort)
	image_count=$(printf '%s\n' "$image_packages" | sed '/^$/d' | wc -l | tr -d ' ')
	dtb_count=$(printf '%s\n' "$dtb_packages" | sed '/^$/d' | wc -l | tr -d ' ')
	[ "$image_count" -eq 1 ] || die "expected exactly one generated linux-image package, got $image_count"
	image_package=$(printf '%s\n' "$image_packages" | sed '/^$/d')
	[ -s "$debs_dir/$image_package" ] || die "generated linux-image package is empty: $image_package"
	[ "$dtb_count" -eq 1 ] || die "expected exactly one generated linux-dtb package, got $dtb_count"
	dtb_package=$(printf '%s\n' "$dtb_packages" | sed '/^$/d')
	[ -s "$debs_dir/$dtb_package" ] || die "generated linux-dtb package is empty: $dtb_package"
	validate_package_metadata "$debs_dir/$image_package" image
	validate_package_metadata "$debs_dir/$dtb_package" dtb
	for package in "$debs_dir"/*.deb; do
		[ -e "$package" ] || continue
		case "$(basename "$package")" in
			linux-image-*.deb|linux-dtb-*.deb|linux-modules-*.deb|linux-headers-*.deb|linux-libc-dev-*.deb) ;;
			*) die "unexpected kernel package in native output: $(basename "$package")" ;;
		esac
	done
	image_root=$(mktemp -d "$TEMP_ROOT/ahub-image.XXXXXX")
	dtb_root=$(mktemp -d "$TEMP_ROOT/ahub-dtb.XXXXXX")
	dpkg-deb -x "$debs_dir/$image_package" "$image_root" >/dev/null
	dpkg-deb -x "$debs_dir/$dtb_package" "$dtb_root" >/dev/null
	if find "$image_root" "$dtb_root" -type f -name 'octessera-ahub0-pcm5102.dtbo' -print -quit | grep -q .; then
		die "octessera DTBO is embedded in a deploy package"
	fi
	dtb_path=$dtb_root/boot/dtb-$KERNEL_ABI/allwinner/sun50i-h618-orangepi-zero2w.dtb
	[ -s "$dtb_path" ] || die "generated H618 Zero2W DTB is missing"
	image_dtb_path=$image_root/usr/lib/linux-image-$KERNEL_ABI/allwinner/sun50i-h618-orangepi-zero2w.dtb
	[ -s "$image_dtb_path" ] || die "generated H618 Zero2W DTB is missing from linux-image"
	config_path=$image_root/boot/config-$KERNEL_ABI
	[ -f "$config_path" ] || die "packaged kernel config is missing: boot/config-$KERNEL_ABI"
	mkdir -p "$output_root/ahub-experiment"
	$PYTHON - "$config_path" "$config_artifact" <<'PY'
import pathlib
import shutil
import sys

required = {
    "CONFIG_ARCH_SUNXI": "y", "CONFIG_SOUND": "y", "CONFIG_SND": "y", "CONFIG_SND_SOC": "y",
    "CONFIG_REGMAP_MMIO": "y", "CONFIG_NVMEM_SUNXI_SID": "y", "CONFIG_SUNXI_SYS_INFO": "n",
    "CONFIG_SND_SOC_GENERIC_DMAENGINE_PCM": "y", "CONFIG_SND_SOC_SUNXI_AHUB": "y",
    "CONFIG_SND_SOC_SUNXI_AHUB_DAM": "y", "CONFIG_SND_SOC_SUNXI_MACH": "y", "CONFIG_SND_SOC_PCM5102A": "y",
}
values = {}
for line in pathlib.Path(sys.argv[1]).read_text(encoding="utf-8").splitlines():
    if line.startswith("# CONFIG_") and line.endswith(" is not set"):
        key, value = line[2:].split(" ", 1)[0], "n"
    elif line.startswith("CONFIG_") and "=" in line:
        key, value = line.split("=", 1)
    else:
        continue
    if key in values:
        raise SystemExit(f"duplicate packaged kernel config entry: {key}")
    values[key] = value
for key, expected in required.items():
    if values.get(key) != expected:
        raise SystemExit(f"packaged kernel config violates {key}={expected}: {values.get(key)!r}")
shutil.copyfile(sys.argv[1], sys.argv[2])
PY
	printf '%s\n' "generated_linux_image_package=$image_package"
	printf '%s\n' "generated_linux_dtb_package=$dtb_package"
	printf '%s\n' "native_linux_headers_packages=${header_packages:-none}" "native_linux_modules_packages=${module_packages:-none}"
	printf '%s\n' "packaged_kernel_config=$config_artifact" "packaged_kernel_abi=$KERNEL_ABI"
}

validate_staged_artifact() {
	staged_root=$1
	staged_packages=$(find "$staged_root" -maxdepth 1 -type f -name '*.deb' -printf '%f\n' | LC_ALL=C sort)
	staged_image_packages=$(find "$staged_root" -maxdepth 1 -type f -name "$KERNEL_PACKAGE_GLOB" -printf '%f\n' | LC_ALL=C sort)
	staged_dtb_packages=$(find "$staged_root" -maxdepth 1 -type f -name "$DTB_PACKAGE_GLOB" -printf '%f\n' | LC_ALL=C sort)
	staged_header_packages=$(find "$staged_root" -maxdepth 1 -type f -name "$HEADERS_PACKAGE_GLOB" -printf '%f\n' | LC_ALL=C sort)
	[ "$(printf '%s\n' "$staged_image_packages" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1 ] || die "staged artifact must contain exactly one linux-image package"
	[ "$(printf '%s\n' "$staged_dtb_packages" | sed '/^$/d' | wc -l | tr -d ' ')" -eq 1 ] || die "staged artifact must contain exactly one linux-dtb package"
	[ -z "$(printf '%s\n' "$staged_header_packages" | sed '/^$/d')" ] || die "staged artifact contains forbidden linux-headers packages"
	for package in $staged_packages; do
		case "$package" in
			linux-image-*.deb|linux-dtb-*.deb) ;;
			*) die "staged artifact contains non-deploy package: $package" ;;
		esac
	done
	[ -f "$staged_root/linux-sunxi64-current.config" ] || die "staged packaged kernel config is missing"
}

stage_deploy_artifacts() {
	native_root=$1
	staged_root=$2
	mkdir -p "$staged_root"
	image_package=$(find "$native_root/debs" -maxdepth 1 -type f -name "$KERNEL_PACKAGE_GLOB" -printf '%f\n')
	dtb_package=$(find "$native_root/debs" -maxdepth 1 -type f -name "$DTB_PACKAGE_GLOB" -printf '%f\n')
	cp "$native_root/debs/$image_package" "$staged_root/"
	cp "$native_root/debs/$dtb_package" "$staged_root/"
	cp "$native_root/ahub-experiment/linux-sunxi64-current.config" "$staged_root/"
	validate_staged_artifact "$staged_root"
}

if [ "$MODE" = test ]; then
	TEST_OUTPUT=$TEMP_ROOT/test-output
	TEST_CONFIG=${AHUB_TEST_IMAGE_CONFIG:-$HERE/runtime-fixture/running-kernel.config}
	[ -f "$TEST_CONFIG" ] || die "test packaged kernel config is missing: $TEST_CONFIG"
	mkdir -p "$TEST_OUTPUT/debs"
	make_test_deb() {
		package_name=$1
		control_package=$2
		package_root=$(mktemp -d "$TEMP_ROOT/test-package.XXXXXX")
		mkdir -p "$package_root/DEBIAN"
		cat > "$package_root/DEBIAN/control" <<EOF
Package: $control_package
Version: ${AHUB_TEST_PACKAGE_VERSION:-$PACKAGE_REVISION}
Section: kernel
Priority: optional
Architecture: ${AHUB_TEST_PACKAGE_ARCHITECTURE:-arm64}
Maintainer: Octessera <octessera@example.invalid>
Description: AHUB experiment fixture package
EOF
		if [ "$control_package" = linux-image-current-sunxi64 ]; then
			printf '%s\n' "Source: linux-$KERNEL_VERSION" "Armbian-Kernel-Version: $KERNEL_VERSION" "Armbian-Kernel-Version-Family: $KERNEL_ABI" >> "$package_root/DEBIAN/control"
			mkdir -p "$package_root/boot"
			cp "$TEST_CONFIG" "$package_root/boot/config-$KERNEL_ABI"
			if [ "${AHUB_TEST_MISSING_DTB:-no}" != yes ]; then
				mkdir -p "$package_root/usr/lib/linux-image-$KERNEL_ABI/allwinner"
				printf '%s\n' fixture > "$package_root/usr/lib/linux-image-$KERNEL_ABI/allwinner/sun50i-h618-orangepi-zero2w.dtb"
			fi
		else
			if [ "${AHUB_TEST_MISSING_DTB:-no}" != yes ]; then
				mkdir -p "$package_root/boot/dtb-$KERNEL_ABI/allwinner"
				printf '%s\n' fixture > "$package_root/boot/dtb-$KERNEL_ABI/allwinner/sun50i-h618-orangepi-zero2w.dtb"
			fi
		fi
		if [ "${AHUB_TEST_EMBED_DTBO:-no}" = yes ]; then
			if [ "$control_package" = linux-image-current-sunxi64 ]; then
				mkdir -p "$package_root/usr/lib/linux-image-$KERNEL_ABI/overlay"
				printf '%s\n' fixture > "$package_root/usr/lib/linux-image-$KERNEL_ABI/overlay/octessera-ahub0-pcm5102.dtbo"
			else
				mkdir -p "$package_root/boot/dtb-$KERNEL_ABI/allwinner/overlay"
				printf '%s\n' fixture > "$package_root/boot/dtb-$KERNEL_ABI/allwinner/overlay/octessera-ahub0-pcm5102.dtbo"
			fi
		fi
		dpkg-deb --build "$package_root" "$TEST_OUTPUT/debs/$package_name" >/dev/null
	}
	make_test_deb "linux-image-current-sunxi64_${PACKAGE_REVISION}_arm64.deb" linux-image-current-sunxi64
	make_test_deb "linux-dtb-current-sunxi64_${PACKAGE_REVISION}_arm64.deb" linux-dtb-current-sunxi64
	if [ -n "${AHUB_TEST_EXTRA_PACKAGE:-}" ]; then
		printf '%s\n' fixture > "$TEST_OUTPUT/debs/$AHUB_TEST_EXTRA_PACKAGE"
	fi
	validate_kernel_output "$TEST_OUTPUT"
	STAGED_OUTPUT=$TEMP_ROOT/staged-output
	stage_deploy_artifacts "$TEST_OUTPUT" "$STAGED_OUTPUT"
	ARTIFACTS_STAGED=1
	printf '%s\n' "staged_linux_image_package=$(find "$STAGED_OUTPUT" -maxdepth 1 -type f -name "$KERNEL_PACKAGE_GLOB" -printf '%f\n')" "staged_linux_dtb_package=$(find "$STAGED_OUTPUT" -maxdepth 1 -type f -name "$DTB_PACKAGE_GLOB" -printf '%f\n')" "staged_linux_headers_packages=none" "test mode validated pinned full series, native package selection, staged deploy artifacts, and packaged kernel config"
elif [ "$MODE" = dry-run ]; then
	printf '%s\n' "source=$SOURCE_DIR"
	printf '%s\n' "source_commit=$COMMIT"
	printf '%s\n' "source_patch_dir=$CORE_PATCH_DIR"
	printf '%s\n' "source_series=$SERIES_PATH"
	printf '%s\n' "source_series_patch_count=$SOURCE_PATCH_COUNT" "kernel_config=$STAGED_CONFIG" "package_revision=$PACKAGE_REVISION" "kernel_abi=$KERNEL_ABI"
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
	validate_kernel_output "$WORKTREE/output"
	stage_deploy_artifacts "$WORKTREE/output" "$OUTPUT_DIR"
	sha256sum "$OUTPUT_DIR"/* > "$OUTPUT_DIR/SHA256SUMS"
	ARTIFACTS_STAGED=1
	printf '%s\n' "experimental Armbian kernel packages written to $OUTPUT_DIR"
fi
