#!/bin/sh
set -eu

HERE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
PYTHON=${PYTHON:-python3}

die() {
	printf '%s\n' "AHUB tests: $*" >&2
	exit 1
}

command -v "$PYTHON" >/dev/null 2>&1 || die "python3 is required"
"$HERE/validate-fixture.sh"
STAGING_OUTPUT=$("$HERE/build-ahub-experiment.sh" --test)
case "$STAGING_OUTPUT" in
	*"source_series_patch_count=458"*"user_patch_dir="*"user_overlay="*"package_input_hook_placement=before_kernel_package_callback_linux_image"*"kernel_headers_disable_hook_point=extension_finish_config"*"kernel_headers_option=KERNEL_HAS_WORKING_HEADERS=no"*"generated_linux_image_package=linux-image-current-sunxi64-test.deb"*"linux_headers_packages=none-by-design"*) ;;
	*) die "test build did not validate the pinned full series, overlay merge, and package hook" ;;
esac
printf '%s\n' "$STAGING_OUTPUT" | grep -F -- 'source_series=/' >/dev/null || die "test build did not report the source series"
printf '%s\n' "$STAGING_OUTPUT" | grep -F -- '/patch/kernel/archive/sunxi-6.12/series.conf' >/dev/null || die "test build source series path changed"
printf '%s\n' "$STAGING_OUTPUT" | grep -F -- '/userpatches/build-hooks/normalize-kernel-package-input.patch' >/dev/null || die "test build hook staging path changed"
printf '%s\n' "$STAGING_OUTPUT" | grep -F -- '/lib/functions/compilation/kernel-debs.sh' >/dev/null || die "test build hook target changed"
printf '%s\n' "$STAGING_OUTPUT" | grep -F -- '/userpatches/extensions/ahub-disable-kernel-headers.sh' >/dev/null || die "test build headers extension staging path changed"
printf '%s\n' "$STAGING_OUTPUT" | grep -F -- 'generated_linux_modules_packages=linux-modules-current-sunxi64-test.deb' >/dev/null || die "test build module package output changed"
printf '%s\n' "$STAGING_OUTPUT" | grep -F -- 'linux_headers_packages=none-by-design' >/dev/null || die "test build headers package output changed"
if grep -R -F -- 'Module.symvers' "$HERE/build-ahub-experiment.sh" "$HERE/extensions" >/dev/null; then
	die "headers disable path must not create or validate Module.symvers"
fi
if printf '%s\n' "$STAGING_OUTPUT" | grep -F -- 'staged_patch_files=' >/dev/null; then
	die "test build retained a private patch subset"
fi
PLAN_OUTPUT=$("$HERE/build-ahub-experiment.sh" --dry-run)
case "$PLAN_OUTPUT" in
	*"source_commit=166b786fc978d88f4ff9ee3e33c353afb39763e8"*"compile.sh kernel"*"KERNELPATCHDIR=archive/sunxi-6.12"*"KERNEL_CONFIGURE=no"*"KERNEL_KEEP_CONFIG=no"*) ;;
	*) die "dry-run build plan is incomplete" ;;
esac
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- 'source_series=/' >/dev/null || die "dry-run build plan is missing the source series"
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- '/patch/kernel/archive/sunxi-6.12/series.conf' >/dev/null || die "dry-run build plan source series path changed"
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- 'source_series_patch_count=458' >/dev/null || die "dry-run build plan source series count changed"
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- 'kernel_config=' >/dev/null || die "dry-run build plan is missing the kernel config"
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- 'linux-sunxi64-current.config' >/dev/null || die "dry-run build plan config path changed"
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- 'user_overlay=' >/dev/null || die "dry-run build plan is missing the custom overlay"
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- 'octessera-ahub0-pi123.dtso' >/dev/null || die "dry-run build plan overlay path changed"
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- 'userpatches/build-hooks/normalize-kernel-package-input.patch' >/dev/null || die "dry-run build plan is missing the package hook"
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- 'package_input_hook_placement=before_kernel_package_callback_linux_image' >/dev/null || die "dry-run package hook placement changed"
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- 'EXT=ahub-disable-kernel-headers ./compile.sh kernel' >/dev/null || die "dry-run headers extension was not enabled"
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- 'kernel_package_glob=linux-image-*.deb' >/dev/null || die "dry-run package glob changed"
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- 'kernel_headers_disable_hook_point=extension_finish_config' >/dev/null || die "dry-run headers hook point changed"
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- 'kernel_headers_option=KERNEL_HAS_WORKING_HEADERS=no' >/dev/null || die "dry-run headers option changed"
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- 'kernel_headers_install_option=INSTALL_HEADERS=no' >/dev/null || die "dry-run headers install option changed"
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- 'dtb_package_glob=linux-dtb-*.deb' >/dev/null || die "dry-run DTB package glob changed"
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- 'module_package_glob=linux-modules-*.deb' >/dev/null || die "dry-run module package glob changed"
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- 'headers_package_glob=linux-headers-*.deb' >/dev/null || die "dry-run headers package glob changed"
printf '%s\n' "$PLAN_OUTPUT" | grep -F -- 'runtime_output_validator=required-image-and-dtb-optional-modules-headers-forbidden' >/dev/null || die "dry-run artifact validation changed"
case "$PLAN_OUTPUT" in
	*"BUILD_MINIMAL"*|*"BUILD_DESKTOP"*|*"compile.sh build"*|*"output/images"*) die "dry-run still contains a rootfs/image stage" ;;
esac

if command -v shellcheck >/dev/null 2>&1; then
	shellcheck "$HERE/validate-fixture.sh" "$HERE/deploy-rollback.sh" "$HERE/build-ahub-experiment.sh" "$HERE/check-patch-stack.sh" "$HERE/test-validate.sh"
fi

expect_failure() {
	name=$1
	shift
	if "$@" >/dev/null 2>&1; then
		die "$name unexpectedly passed"
	fi
	printf 'negative: %s\n' "$name"
}

REPO_ROOT=$(CDPATH='' cd -- "$HERE/../../.." && pwd)
CASE_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/octessera-ahub-preflight.XXXXXX")
trap 'rm -rf "$CASE_ROOT"' EXIT HUP INT TERM
CASE_DIR=$CASE_ROOT/tools/orange-pi/ahub-experiment

copy_case() {
	rm -rf "$CASE_ROOT/tools"
	rm -f "$CASE_ROOT/.slim"
	mkdir -p "$CASE_ROOT/tools/orange-pi"
	cp -R "$HERE" "$CASE_DIR"
	ln -s "$REPO_ROOT/.slim" "$CASE_ROOT/.slim"
}

mutate_case() {
	file=$1
	old=$2
	new=$3
	"$PYTHON" - "$CASE_DIR/$file" "$CASE_DIR/stack-lock.json" "$old" "$new" <<'PY'
import hashlib
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
lock_path = pathlib.Path(sys.argv[2])
old, new = sys.argv[3:]
text = path.read_text()
if text.count(old) != 1:
    raise SystemExit(f"expected one mutation point: {old}")
path.write_text(text.replace(old, new))
lock = json.loads(lock_path.read_text())
for asset in lock["assets"]:
    if asset["path"] == path.relative_to(lock_path.parent).as_posix():
        asset["sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
        break
else:
    raise SystemExit(f"asset is not locked: {path}")
lock_path.write_text(json.dumps(lock, indent=2) + "\n")
PY
}

mutate_lock() {
	old=$1
	new=$2
	"$PYTHON" - "$CASE_DIR/stack-lock.json" "$old" "$new" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
old, new = sys.argv[2:]
text = path.read_text()
if text.count(old) != 1:
    raise SystemExit(f"expected one lock mutation point: {old}")
path.write_text(text.replace(old, new))
PY
}

copy_case
mutate_case octessera-ahub0-pi123-overlay.dts 'tdm_num = <0>;' 'tdm_num = <1>;'
expect_failure 'preflight rejects nonzero TDM' "$PYTHON" "$CASE_DIR/preflight.py" "$CASE_DIR"

copy_case
mutate_case octessera-ahub0-pi123-overlay.dts 'pins = "PI1", "PI2", "PI3";' 'pins = "PI0";'
expect_failure 'preflight rejects PI0 audio claim' "$PYTHON" "$CASE_DIR/preflight.py" "$CASE_DIR"

copy_case
mutate_case Kconfig.fragment 'CONFIG_SUNXI_SYS_INFO=n' 'CONFIG_SUNXI_SYS_INFO=y'
expect_failure 'preflight rejects enabled sysinfo driver' "$PYTHON" "$CASE_DIR/preflight.py" "$CASE_DIR"

copy_case
mutate_case Kconfig.fragment 'CONFIG_SND_SOC_PCM5102A=y' 'CONFIG_SND_SOC_PCM5102A=m'
expect_failure 'preflight rejects non builtin PCM5102A' "$PYTHON" "$CASE_DIR/preflight.py" "$CASE_DIR"

copy_case
mutate_case runtime-fixture/deferred-probes.txt 'deferred_probe_count=0' 'deferred_probe_count=1'
expect_failure 'preflight rejects deferred devices' "$PYTHON" "$CASE_DIR/preflight.py" "$CASE_DIR"

copy_case
mutate_lock '"patch_count": 458' '"patch_count": 457'
expect_failure 'preflight rejects incomplete full source series' "$PYTHON" "$CASE_DIR/preflight.py" "$CASE_DIR"

copy_case
mutate_case build-ahub-experiment.sh 'linux-modules-current-sunxi64-test.deb' 'linux-headers-current-sunxi64-test.deb'
expect_failure 'test output rejects linux-headers package' "$CASE_DIR/build-ahub-experiment.sh" --test

expect_failure 'deploy requires target' "$HERE/deploy-rollback.sh" deploy --yes --dtb /boot/dtb/allwinner/sun50i-h618-orangepi-zero2w.dtb
expect_failure 'SSH_BATCH_MODE accepts only yes or no' env SSH_BATCH_MODE=maybe "$HERE/deploy-rollback.sh" deploy --yes --target octessera@192.168.0.217 --dtb /boot/dtb-6.12.30-current-sunxi64/allwinner/sun50i-h618-orangepi-zero2w.dtb
expect_failure 'deploy requires nonempty user@host target' "$HERE/deploy-rollback.sh" deploy --yes --target root@ --dtb /boot/dtb-6.12.30-current-sunxi64/allwinner/sun50i-h618-orangepi-zero2w.dtb
expect_failure 'deploy requires exact DTB' "$HERE/deploy-rollback.sh" deploy --yes --target octessera@192.168.0.217 --dtb /boot/dtb-6.12.30-current-sunxi64/allwinner/wrong.dtb
expect_failure 'rollback requires confirmation' "$HERE/deploy-rollback.sh" rollback --target octessera@192.168.0.217 --dtb /boot/dtb-6.12.30-current-sunxi64/allwinner/sun50i-h618-orangepi-zero2w.dtb --backup-id test
expect_failure 'rollback requires backup ID' "$HERE/deploy-rollback.sh" rollback --yes --target octessera@192.168.0.217 --dtb /boot/dtb-6.12.30-current-sunxi64/allwinner/sun50i-h618-orangepi-zero2w.dtb
expect_failure 'build output must stay outside repository' "$HERE/build-ahub-experiment.sh" --run-kernel --output "$HERE/build-output"

if grep -Eq '(^|[[:space:]])(reboot|shutdown|systemctl[[:space:]]+reboot)([[:space:]]|$)' "$HERE/deploy-rollback.sh"; then
	die "deploy helper contains a reboot command"
fi

FAKE_DIR=$(mktemp -d "${TMPDIR:-/tmp}/octessera-ahub-fake.XXXXXX")
trap 'rm -rf "$CASE_ROOT" "$FAKE_DIR"' EXIT HUP INT TERM
cat > "$FAKE_DIR/ssh" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >> "$FAKE_LOG_DIR/ssh.log"
last=
for arg in "$@"; do
	last=$arg
done
if [ "${FAKE_VALIDATE_REMOTE:-no}" = yes ]; then
	if ! sh -n -c "$last"; then
		printf '%s\n' "$last" >&2
		exit 1
	fi
fi
EOF
cat > "$FAKE_DIR/scp" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >> "$FAKE_LOG_DIR/scp.log"
EOF
chmod +x "$FAKE_DIR/ssh" "$FAKE_DIR/scp"

CLEANUP_DIR=$FAKE_DIR/cleanup
mkdir -p "$CLEANUP_DIR/bin" "$CLEANUP_DIR/tmp"
cat > "$CLEANUP_DIR/bin/rm" <<'EOF'
#!/bin/sh
last=
for arg in "$@"; do
	last=$arg
done
if [ "${FAKE_PRIVILEGED:-no}" != yes ]; then
	mkdir -p "$last/container-cache"
	printf '%s\n' root-owned > "$last/container-cache/cache.marker"
	exit 1
fi
printf '%s\n' "$last" > "$FAKE_CLEANUP_LOG"
exec /bin/rm "$@"
EOF
cat > "$CLEANUP_DIR/bin/sudo" <<'EOF'
#!/bin/sh
[ "${1:-}" = -n ] || exit 1
shift
FAKE_PRIVILEGED=yes "$@"
EOF
chmod +x "$CLEANUP_DIR/bin/rm" "$CLEANUP_DIR/bin/sudo"
CLEANUP_LOG=$CLEANUP_DIR/cleanup.log
FAKE_CLEANUP_LOG=$CLEANUP_LOG PATH="$CLEANUP_DIR/bin:$PATH" TMPDIR="$CLEANUP_DIR/tmp" "$HERE/build-ahub-experiment.sh" --test
cleanup_path=$(cat "$CLEANUP_LOG")
case "$cleanup_path" in
	"$CLEANUP_DIR/tmp"/octessera-ahub-build.*) ;;
	*) die "privileged cleanup path escaped its mktemp prefix" ;;
esac
[ ! -e "$cleanup_path" ] || die "privileged cleanup did not remove the simulated container cache"

FAIL_CLEANUP_DIR=$FAKE_DIR/cleanup-failure
mkdir -p "$FAIL_CLEANUP_DIR/bin" "$FAIL_CLEANUP_DIR/tmp"
cat > "$FAIL_CLEANUP_DIR/bin/rm" <<'EOF'
#!/bin/sh
last=
for arg in "$@"; do
	last=$arg
done
mkdir -p "$last/container-cache"
printf '%s\n' root-owned > "$last/container-cache/cache.marker"
printf '%s\n' "$last" > "$FAKE_CLEANUP_LOG"
exit 1
EOF
cat > "$FAIL_CLEANUP_DIR/bin/sudo" <<'EOF'
#!/bin/sh
[ "${1:-}" = -n ] || exit 1
exit 1
EOF
chmod +x "$FAIL_CLEANUP_DIR/bin/rm" "$FAIL_CLEANUP_DIR/bin/sudo"
FAIL_CLEANUP_LOG=$FAIL_CLEANUP_DIR/cleanup.log
FAKE_CLEANUP_LOG=$FAIL_CLEANUP_LOG PATH="$FAIL_CLEANUP_DIR/bin:$PATH" TMPDIR="$FAIL_CLEANUP_DIR/tmp" "$HERE/build-ahub-experiment.sh" --test
failed_cleanup_path=$(cat "$FAIL_CLEANUP_LOG")
case "$failed_cleanup_path" in
	"$FAIL_CLEANUP_DIR/tmp"/octessera-ahub-build.*) ;;
	*) die "cleanup failure path escaped its mktemp prefix" ;;
esac
[ -f "$failed_cleanup_path/test-output/debs/linux-image-current-sunxi64-test.deb" ] || die "successful test artifacts were lost after cleanup failure"
/bin/rm -rf -- "$failed_cleanup_path"

KEEP_DIR=$FAKE_DIR/keep-work
mkdir -p "$KEEP_DIR/tmp"
TMPDIR="$KEEP_DIR/tmp" "$HERE/build-ahub-experiment.sh" --test --keep-work
kept_path=$(find "$KEEP_DIR/tmp" -mindepth 1 -maxdepth 1 -type d -name 'octessera-ahub-build.*' -print -quit)
[ -n "$kept_path" ] || die "--keep-work did not suppress cleanup"
/bin/rm -rf -- "$kept_path"

FAKE_LOG_DIR=$FAKE_DIR FAKE_VALIDATE_REMOTE=yes SSH_BIN="$FAKE_DIR/ssh" SCP_BIN="$FAKE_DIR/scp" "$HERE/deploy-rollback.sh" deploy --yes \
	--target octessera@192.168.0.217 \
	--dtb /boot/dtb-6.12.30-current-sunxi64/allwinner/sun50i-h618-orangepi-zero2w.dtb
grep -F -- '-o BatchMode=yes -o ConnectTimeout=10 --' "$FAKE_DIR/ssh.log" >/dev/null || die "default SSH_BATCH_MODE was not passed to ssh"
grep -F -- '-o BatchMode=yes -o ConnectTimeout=10 --' "$FAKE_DIR/scp.log" >/dev/null || die "default SSH_BATCH_MODE was not passed to scp"
grep -F 'grep "^BOARD=" /etc/armbian-release | cut -d= -f2 | sed -n "1p"' "$FAKE_DIR/ssh.log" >/dev/null || die "remote BOARD extraction was not generated safely"
grep -F 'orangepizero2w' "$FAKE_DIR/ssh.log" >/dev/null || die "strict BOARD identity check was not generated"
grep -F '/boot/dtb-6.12.30-current-sunxi64/allwinner/overlay/octessera-ahub0-pcm5102.dtbo' "$FAKE_DIR/ssh.log" >/dev/null || die "versioned sibling overlay path was not derived"
if grep -F '/boot/dtb/allwinner/overlay' "$FAKE_DIR/ssh.log" >/dev/null; then
	die "deployment retained the old hardcoded overlay path"
fi

: > "$FAKE_DIR/ssh.log"
: > "$FAKE_DIR/scp.log"
FAKE_LOG_DIR=$FAKE_DIR FAKE_VALIDATE_REMOTE=yes SSH_BIN="$FAKE_DIR/ssh" SCP_BIN="$FAKE_DIR/scp" SSH_BATCH_MODE=no "$HERE/deploy-rollback.sh" deploy --yes \
	--target octessera@192.168.0.217 \
	--dtb /boot/dtb-6.12.30-current-sunxi64/allwinner/sun50i-h618-orangepi-zero2w.dtb
grep -F -- '-o BatchMode=no -o ConnectTimeout=10 --' "$FAKE_DIR/ssh.log" >/dev/null || die "SSH_BATCH_MODE=no was not passed to ssh"
grep -F -- '-o BatchMode=no -o ConnectTimeout=10 --' "$FAKE_DIR/scp.log" >/dev/null || die "SSH_BATCH_MODE=no was not passed to scp"

: > "$FAKE_DIR/ssh.log"
FAKE_LOG_DIR=$FAKE_DIR FAKE_VALIDATE_REMOTE=yes SSH_BIN="$FAKE_DIR/ssh" SCP_BIN="$FAKE_DIR/scp" "$HERE/deploy-rollback.sh" rollback --yes \
	--target octessera@192.168.0.217 \
	--dtb /boot/dtb-6.12.30-current-sunxi64/allwinner/sun50i-h618-orangepi-zero2w.dtb \
	--backup-id fixture
grep -F 'backup=/boot/.octessera-ahub-experiment/fixture' "$FAKE_DIR/ssh.log" >/dev/null || die "rollback backup path was not generated"
grep -F 'manifest' "$FAKE_DIR/ssh.log" >/dev/null || die "rollback manifest path was not generated"
grep -F '/boot/dtb-6.12.30-current-sunxi64/allwinner/overlay/octessera-ahub0-pcm5102.dtbo' "$FAKE_DIR/ssh.log" >/dev/null || die "rollback overlay path was not derived"

printf '%s\n' 'fixture and deploy safety checks passed'
