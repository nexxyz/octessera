#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
hook="$root/userpatches/overlay/etc/initramfs-tools/hooks/octessera-orange-boot-splash"
boot_service="$root/userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service"
shutdown_service="$root/userpatches/overlay/etc/systemd/system/octessera-orange-oled-shutdown.service"
suspend_service="$root/userpatches/overlay/etc/systemd/system/octessera-orange-oled-suspend.service"
python313_files="$root/tools/armbian-image/fixtures/python313-initramfs-closure-files.txt"
python313_fixture="$root/tools/armbian-image/fixtures/python313-initramfs-closure"
oled_logo="$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo"

[[ -f "$hook" ]] || { echo "Missing Orange initramfs boot-splash hook." >&2; exit 1; }
for required_line in \
    'Type=simple' \
    'User=octessera-runtime' \
    'Group=octessera-runtime' \
    'ExecStart=/usr/local/sbin/octessera-orange-oled-logo boot-loop' \
    'RuntimeDirectory=octessera-boot' \
    'RuntimeDirectoryMode=0750' \
    'RuntimeDirectoryPreserve=yes' \
    'UMask=0027' \
    'KillMode=control-group' \
    'TimeoutStopSec=2' \
    'Restart=no'; do
    grep -qFx "$required_line" "$boot_service" || { echo "Orange boot service is missing: $required_line" >&2; exit 1; }
done
for required_line in \
    'DevicePolicy=closed' \
    'DeviceAllow=/dev/spidev1.0 rw' \
    'DeviceAllow=/dev/gpiochip1 rw'; do
    grep -qFx "$required_line" "$boot_service" || { echo "Orange boot service is missing: $required_line" >&2; exit 1; }
done
! grep -q '^Conflicts=' "$boot_service" || { echo "Orange boot service must not conflict with runtime." >&2; exit 1; }
grep -qFx 'After=systemd-udev-trigger.service systemd-modules-load.service systemd-udevd.service local-fs.target' "$boot_service"
grep -qFx 'Before=sysinit.target octessera.service' "$boot_service"
! grep -q '^Conflicts=' "$shutdown_service" || { echo "Orange shutdown service must not conflict with runtime." >&2; exit 1; }
for required_line in \
    'After=octessera.service' \
    'Before=shutdown.target reboot.target halt.target' \
    'User=octessera-runtime' \
    'Group=octessera-runtime' \
    'ProtectSystem=strict' \
    'ReadWritePaths=/run/octessera-boot' \
    'DevicePolicy=closed' \
    'DeviceAllow=/dev/spidev1.0 rw' \
    'DeviceAllow=/dev/gpiochip1 rw' \
    'ExecStart=-/usr/local/sbin/octessera-orange-oled-logo shutdown' \
    'TimeoutStartSec=5'; do
    grep -qFx "$required_line" "$shutdown_service" || { echo "Orange shutdown service is missing: $required_line" >&2; exit 1; }
done
! grep -qFx 'SupplementaryGroups=audio i2c spi gpio' "$shutdown_service" || { echo 'Orange shutdown service must use the runtime account without named supplementary groups.' >&2; exit 1; }
[[ ! -e "$root/userpatches/overlay/lib/systemd/system-sleep/octessera-orange-oled" ]] || { echo 'Orange system-sleep hook must be removed.' >&2; exit 1; }
grep -qF '["gpioset", "--chip", self.chip, f"{offset}={value}"]' "$oled_logo" || { echo 'Orange OLED GPIO control must use the fixed libgpiod v2 syntax.' >&2; exit 1; }
! grep -qF -- '--mode=wait' "$oled_logo" || { echo 'Orange OLED GPIO control must not use the removed libgpiod v1 mode option.' >&2; exit 1; }
! grep -qE -- '--(daemonize|toggle|hold-period)' "$oled_logo" || { echo 'Orange OLED GPIO control must retain process-held ownership.' >&2; exit 1; }
for required_line in \
    'After=octessera.service' \
    'Requisite=octessera.service' \
    'Before=sleep.target' \
    'RequiredBy=sleep.target' \
    'StopWhenUnneeded=yes' \
    'Type=oneshot' \
    'RemainAfterExit=yes' \
    'User=octessera-runtime' \
    'RuntimeDirectory=octessera-oled-suspend' \
    'RuntimeDirectoryMode=0700' \
    'RestrictAddressFamilies=AF_UNIX' \
    'ExecStart=/usr/local/sbin/octessera-orange-oled-suspend prepare' \
    'ExecStop=/usr/local/sbin/octessera-orange-oled-suspend resume' \
    'TimeoutStartSec=8' \
    'TimeoutStopSec=8'; do
    grep -qFx "$required_line" "$suspend_service" || { echo "Orange suspend service is missing: $required_line" >&2; exit 1; }
done
! grep -qFx 'SupplementaryGroups=audio i2c spi gpio' "$suspend_service" || { echo 'Orange suspend service must use the runtime account without named supplementary groups.' >&2; exit 1; }
! grep -qE '(^|[[:space:]])(systemctl|dbus|Conflicts=)' "$suspend_service" || { echo 'Orange suspend service contains a forbidden lifecycle dependency.' >&2; exit 1; }

require_boot_service_contract() {
    local candidate="$1"
    grep -qFx 'DevicePolicy=closed' "$candidate" && \
        grep -qFx 'DeviceAllow=/dev/spidev1.0 rw' "$candidate" && \
        grep -qFx 'DeviceAllow=/dev/gpiochip1 rw' "$candidate" && \
        grep -qFx 'After=systemd-udev-trigger.service systemd-modules-load.service systemd-udevd.service local-fs.target' "$candidate"
}
for missing in \
    'DevicePolicy=closed' \
    'DeviceAllow=/dev/spidev1.0 rw' \
    'DeviceAllow=/dev/gpiochip1 rw' \
    'After=systemd-udev-trigger.service systemd-modules-load.service systemd-udevd.service local-fs.target'; do
    negative_service="$(mktemp)"
    grep -vFx "$missing" "$boot_service" > "$negative_service"
    if require_boot_service_contract "$negative_service"; then
        echo "Orange boot service accepted missing contract: $missing" >&2
        rm -f "$negative_service"
        exit 1
    fi
    rm -f "$negative_service"
done
if command -v systemd-analyze >/dev/null 2>&1; then
    systemd_work="$(mktemp -d)"
    mkdir -p "$systemd_work/etc/systemd/system" "$systemd_work/usr/local/sbin"
    cp "$boot_service" "$systemd_work/etc/systemd/system/octessera-orange-boot-splash.service"
    cp "$shutdown_service" "$systemd_work/etc/systemd/system/octessera-orange-oled-shutdown.service"
    cp "$suspend_service" "$systemd_work/etc/systemd/system/octessera-orange-oled-suspend.service"
    chmod 0644 "$systemd_work/etc/systemd/system/octessera-orange-boot-splash.service"
    chmod 0644 "$systemd_work/etc/systemd/system/octessera-orange-oled-shutdown.service"
    chmod 0644 "$systemd_work/etc/systemd/system/octessera-orange-oled-suspend.service"
    printf '%s\n' '#!/bin/sh' 'exit 0' > "$systemd_work/usr/local/sbin/octessera-orange-oled-logo"
    chmod 0755 "$systemd_work/usr/local/sbin/octessera-orange-oled-logo"
    printf '%s\n' '#!/bin/sh' 'exit 0' > "$systemd_work/usr/local/sbin/octessera-orange-oled-suspend"
    chmod 0755 "$systemd_work/usr/local/sbin/octessera-orange-oled-suspend"
    for unit in local-fs.target sysinit.target shutdown.target reboot.target halt.target sleep.target; do
        printf '%s\n' '[Unit]' "Description=$unit" > "$systemd_work/etc/systemd/system/$unit"
    done
    for unit in systemd-udev-trigger.service systemd-modules-load.service systemd-udevd.service octessera.service; do
        if [[ "$unit" == systemd-udev-trigger.service || "$unit" == systemd-modules-load.service || "$unit" == systemd-udevd.service ]]; then
            printf '%s\n' '[Unit]' "Description=$unit" 'DefaultDependencies=no' '[Service]' 'Type=oneshot' 'ExecStart=/bin/true' > "$systemd_work/etc/systemd/system/$unit"
        else
            printf '%s\n' '[Unit]' "Description=$unit" '[Service]' 'Type=oneshot' 'ExecStart=/bin/true' > "$systemd_work/etc/systemd/system/$unit"
        fi
    done
    mkdir -p "$systemd_work/bin"
    printf '%s\n' '#!/bin/sh' 'exit 0' > "$systemd_work/bin/true"
    chmod 0755 "$systemd_work/bin/true"
    systemd-analyze --root="$systemd_work" verify octessera-orange-boot-splash.service octessera-orange-oled-shutdown.service octessera-orange-oled-suspend.service
    rm -rf "$systemd_work"
fi
bash -n "$hook"
[[ "$(sh "$hook" prereqs)" == "" ]] || { echo "Orange boot-splash hook has unexpected prerequisites." >&2; exit 1; }

if grep -qF 'copy_dir' "$hook"; then
    echo "Orange initramfs hook uses unavailable copy_dir." >&2
    exit 1
fi
if grep -qF 'find ' "$hook"; then
    echo "Orange initramfs hook must use the explicit Python runtime closure." >&2
    exit 1
fi
grep -qF 'for python_dir in /usr/lib/python3.*;' "$hook" || { echo "Python standard-library path enumeration is missing." >&2; exit 1; }
grep -qF "\"\$python_dir/encodings/__init__.py\"" "$hook" || { echo "Python encoding runtime path is missing." >&2; exit 1; }
grep -qF "\"\$python_dir/re/_parser.py\"" "$hook" || { echo "Python regular-expression runtime path is missing." >&2; exit 1; }
grep -qF "copy_file binary \"\$python_file\" \"\$python_file\"" "$hook" || { echo "Python files are not copied as initramfs binaries." >&2; exit 1; }
grep -qF 'Orange initramfs hook missing Python target directory:' "$hook" || { echo "Missing fail-closed Python standard-library check." >&2; exit 1; }
grep -qF 'for python_module in fcntl math _json _posixsubprocess select _struct zlib; do' "$hook" || { echo "Python extension-module closure is missing." >&2; exit 1; }
grep -qF "copy_exec \"\$python_file\" \"\$python_file\"" "$hook" || { echo "Python extension modules are not copied with dependencies." >&2; exit 1; }
grep -qF "\"\$python_dir/_collections_abc.py\"" "$hook" || { echo "Python 3.13 _collections_abc.py closure path is missing." >&2; exit 1; }
! grep -qF "\"\$python_dir/collections/abc.py\"" "$hook" || { echo "Removed Python 3.13 collections/abc.py path is still required." >&2; exit 1; }
grep -qF 'missing Python target file:' "$hook" || { echo "Missing Python target-file diagnostic is absent." >&2; exit 1; }
grep -qF 'missing Python target module:' "$hook" || { echo "Missing Python target-module diagnostic is absent." >&2; exit 1; }

[[ -f "$python313_files" ]] || { echo "Missing Python 3.13 initramfs closure fixture." >&2; exit 1; }
[[ -f "$python313_fixture/imports.py" ]] || { echo "Missing Python 3.13 closure import fixture." >&2; exit 1; }

for required_line in \
    'copy_exec /usr/local/sbin/octessera-orange-oled-logo /usr/local/sbin/octessera-orange-oled-logo' \
    'copy_file binary /usr/local/sbin/octessera-orange-oled-handoff.py /usr/local/sbin/octessera-orange-oled-handoff.py' \
    'copy_exec /usr/bin/setsid /usr/bin/setsid' \
    'copy_exec /usr/bin/gpioset /usr/bin/gpioset' \
    'copy_file asset /usr/share/octessera/oled/octessera-mark.svg' \
    'copy_file asset /usr/share/octessera/oled/octessera-wordmark.svg'; do
    grep -qF "$required_line" "$hook" || { echo "Orange initramfs dependency was removed: $required_line" >&2; exit 1; }
done
for removed_line in \
    'manual_add_modules spi-sun6i' \
    'manual_add_modules spidev' \
    'manual_add_modules pinctrl-sunxi'; do
    ! grep -qF "$removed_line" "$hook" || { echo "Obsolete Orange initramfs module addition returned: $removed_line" >&2; exit 1; }
done
for removed_line in \
    'modprobe spi-sun6i' \
    'modprobe spidev' \
    'modprobe pinctrl-sunxi'; do
    ! grep -qF "$removed_line" "$root/userpatches/overlay/etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash" || { echo "Obsolete Orange initramfs module load returned: $removed_line" >&2; exit 1; }
done
for json_file in json/__init__.py json/decoder.py json/encoder.py json/scanner.py; do
    grep -qF "\$python_dir/$json_file" "$hook" || { echo "Orange initramfs JSON closure is missing: $json_file" >&2; exit 1; }
done

run_python313_closure_import_test() {
    command -v python3 >/dev/null 2>&1 || { echo "Python 3 is required for the Python 3.13 closure import fixture." >&2; exit 1; }
    PYTHONPATH="$python313_fixture" PYTHONDONTWRITEBYTECODE=1 python3 -S "$python313_fixture/imports.py"
    printf 'Orange Python 3.13 closure import fixture passed\n'
}

run_python313_closure_hook_test() {
    local work
    local python_dir
    local hook_functions
    local copy_log
    local fixture_hook
    local missing_module_python
    local python_file
    local python_module

    work="$(mktemp -d)"
    python_dir="$work/usr/lib/python3.13"
    hook_functions="$work/hook-functions"
    copy_log="$work/copy.log"
    fixture_hook="$work/hook"
    missing_module_python="$work/python3-failing"
    mkdir -p "$python_dir/lib-dynload"
    cat > "$hook_functions" <<'EOF'
copy_exec() { printf 'copy_exec %s %s\n' "$1" "$2" >> "$OCTESSERA_COPY_LOG"; }
copy_file() { printf 'copy_file %s %s %s\n' "$1" "$2" "$3" >> "$OCTESSERA_COPY_LOG"; }
EOF
    while IFS= read -r python_file; do
        [[ -n "$python_file" ]] || continue
        [[ "$python_file" != /* && "$python_file" != *..* ]] || { echo "Unsafe Python 3.13 fixture path: $python_file" >&2; rm -rf "$work"; exit 1; }
        mkdir -p "$python_dir/$(dirname "$python_file")"
        : > "$python_dir/$python_file"
    done < "$python313_files"
    for python_module in fcntl math _json _posixsubprocess select _struct zlib; do
        : > "$python_dir/lib-dynload/${python_module}.so"
    done
    make_fixture_hook() {
        local output="$1"
        local python_source="$2"
        sed \
            -e "s|^\. /usr/share/initramfs-tools/hook-functions$|. \"$hook_functions\"|" \
            -e "s|copy_exec /usr/bin/python3 /usr/bin/python3|copy_exec \"$python_source\" /usr/bin/python3|" \
            -e "s|/usr/bin/python3 -c|\"$python_source\" -c|" \
            -e "s|^for python_dir in /usr/lib/python3\.\*; do$|for python_dir in \"$python_dir\"; do|" \
            "$hook" > "$output"
        chmod 0755 "$output"
    }
    make_fixture_hook "$fixture_hook" "$(command -v python3)"
    if ! OCTESSERA_COPY_LOG="$copy_log" sh "$fixture_hook" >"$work/hook.out" 2>&1; then
        cat "$work/hook.out" >&2
        rm -rf "$work"
        exit 1
    fi
    grep -qF "copy_file binary $python_dir/_collections_abc.py $python_dir/_collections_abc.py" "$copy_log" || {
        echo "Python 3.13 fixture hook did not copy _collections_abc.py." >&2
        rm -rf "$work"
        exit 1
    }
    if grep -qF 'collections/abc.py' "$copy_log"; then
        echo "Python 3.13 fixture hook copied the removed collections/abc.py path." >&2
        rm -rf "$work"
        exit 1
    fi

    rm "$python_dir/_collections_abc.py"
    if OCTESSERA_COPY_LOG="$copy_log" sh "$fixture_hook" >"$work/missing-file.out" 2>&1; then
        echo "Python 3.13 fixture hook accepted a missing target file." >&2
        rm -rf "$work"
        exit 1
    fi
    grep -qF "missing Python target file: $python_dir/_collections_abc.py" "$work/missing-file.out" || {
        echo "Missing target-file diagnostic did not name the Python file." >&2
        cat "$work/missing-file.out" >&2
        rm -rf "$work"
        exit 1
    }
    : > "$python_dir/_collections_abc.py"

    printf '%s\n' '#!/bin/sh' 'exit 1' > "$missing_module_python"
    chmod 0755 "$missing_module_python"
    rm "$python_dir/lib-dynload/zlib.so"
    make_fixture_hook "$work/missing-module-hook" "$missing_module_python"
    if OCTESSERA_COPY_LOG="$copy_log" sh "$work/missing-module-hook" >"$work/missing-module.out" 2>&1; then
        echo "Python 3.13 fixture hook accepted a missing target module." >&2
        rm -rf "$work"
        exit 1
    fi
    grep -qF 'missing Python target module: zlib' "$work/missing-module.out" || {
        echo "Missing target-module diagnostic did not name the Python module." >&2
        cat "$work/missing-module.out" >&2
        rm -rf "$work"
        exit 1
    }
    rm -rf "$work"
    printf 'Orange Python 3.13 closure hook fixture passed\n'
}

run_python313_closure_import_test
run_python313_closure_hook_test

run_extracted_runtime_test() {
    local hook_functions=/usr/share/initramfs-tools/hook-functions
    local work
    local destination
    local extracted
    local archive
    local hook_copy
    local oled_source
    local mark_source
    local wordmark_source
    local handoff_source
    local fake_gpioset
    local python_stdlib
    local python_file
    local python_module
    local runtime_output
    local runtime_status
    local kernel_version
    local update_hook_path
    local update_status
    local python313_collections

    [[ -r "$hook_functions" ]] || { echo "Orange initramfs extraction test requires $hook_functions." >&2; return 1; }
    command -v cpio >/dev/null 2>&1 || { echo "Orange initramfs extraction test requires cpio." >&2; return 1; }
    command -v lsinitramfs >/dev/null 2>&1 || { echo "Orange initramfs extraction test requires lsinitramfs." >&2; return 1; }
    command -v unmkinitramfs >/dev/null 2>&1 || { echo "Orange initramfs extraction test requires unmkinitramfs." >&2; return 1; }
    command -v python3 >/dev/null 2>&1 || { echo "Orange initramfs extraction test requires Python 3." >&2; return 1; }

    work="$(mktemp -d)"
    destination="$work/dest"
    extracted="$work/extracted"
    archive="$work/initramfs.img"
    hook_copy="$work/hook"
    oled_source="$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo"
    mark_source="$root/userpatches/overlay/usr/local/share/octessera-setup-ui/octessera-mark.svg"
    wordmark_source="$root/userpatches/overlay/usr/local/share/octessera-setup-ui/octessera-wordmark.svg"
    handoff_source="$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-handoff.py"
    fake_gpioset="$work/gpioset"
    mkdir -p "$destination" "$extracted" "$work/boot"
    printf '%s\n' '#!/bin/sh' 'exit 0' > "$fake_gpioset"
    chmod 0755 "$fake_gpioset"
    sed \
        -e "s|copy_exec /usr/local/sbin/octessera-orange-oled-logo /usr/local/sbin/octessera-orange-oled-logo|copy_exec \"$oled_source\" /usr/local/sbin/octessera-orange-oled-logo|" \
        -e "s|copy_file binary /usr/local/sbin/octessera-orange-oled-handoff.py /usr/local/sbin/octessera-orange-oled-handoff.py|copy_file binary \"$handoff_source\" /usr/local/sbin/octessera-orange-oled-handoff.py|" \
        -e "s|copy_exec /usr/bin/gpioset /usr/bin/gpioset|copy_exec \"$fake_gpioset\" /usr/bin/gpioset|" \
        -e "s|copy_file asset /usr/share/octessera/oled/octessera-mark.svg|copy_file asset \"$mark_source\" /usr/share/octessera/oled/octessera-mark.svg|" \
        -e "s|copy_file asset /usr/share/octessera/oled/octessera-wordmark.svg|copy_file asset \"$wordmark_source\" /usr/share/octessera/oled/octessera-wordmark.svg|" \
        "$hook" > "$hook_copy"

    kernel_version="$(uname -r)"
    if [[ "$(id -u)" == 0 ]] && command -v update-initramfs >/dev/null 2>&1; then
        update_hook_path="/etc/initramfs-tools/hooks/octessera-orange-boot-splash-test-$$"
        cp "$hook_copy" "$update_hook_path"
        chmod 0755 "$update_hook_path"
        set +e
        update-initramfs -c -k "$kernel_version" -b "$work/boot" > "$work/update-initramfs.log" 2>&1
        update_status=$?
        set -e
        rm -f "$update_hook_path"
        [[ "$update_status" == 0 ]] || { echo "update-initramfs failed for the Orange boot-splash hook." >&2; cat "$work/update-initramfs.log" >&2; rm -rf "$work"; exit 1; }
        archive="$work/boot/initrd.img-$kernel_version"
    else
        if ! DESTDIR="$destination" MODULESDIR=/lib/modules/"$kernel_version" version="$kernel_version" verbose=n sh "$hook_copy"; then
            echo "Orange initramfs hook failed under the installed initramfs-tools hook contract." >&2
            rm -rf "$work"
            exit 1
        fi
        (
            cd "$destination"
            find . -print | cpio -o -H newc --quiet | gzip -n > "$archive"
        )
    fi
    lsinitramfs "$archive" > "$work/contents"
    grep -qF 'usr/local/sbin/octessera-orange-oled-logo' "$work/contents" || { echo "Extracted initramfs is missing the OLED executable." >&2; rm -rf "$work"; exit 1; }
    grep -qF 'usr/local/sbin/octessera-orange-oled-handoff.py' "$work/contents" || { echo "Extracted initramfs is missing the OLED handoff module." >&2; rm -rf "$work"; exit 1; }
    grep -qF 'usr/share/octessera/oled/octessera-mark.svg' "$work/contents" || { echo "Extracted initramfs is missing the mark asset." >&2; rm -rf "$work"; exit 1; }
    grep -qF 'usr/share/octessera/oled/octessera-wordmark.svg' "$work/contents" || { echo "Extracted initramfs is missing the wordmark asset." >&2; rm -rf "$work"; exit 1; }
    unmkinitramfs "$archive" "$extracted"
    cmp "$oled_source" "$extracted/usr/local/sbin/octessera-orange-oled-logo"
    cmp "$handoff_source" "$extracted/usr/local/sbin/octessera-orange-oled-handoff.py"
    cmp "$mark_source" "$extracted/usr/share/octessera/oled/octessera-mark.svg"
    cmp "$wordmark_source" "$extracted/usr/share/octessera/oled/octessera-wordmark.svg"

    python_stdlib="$(python3 -c 'import sysconfig; print(sysconfig.get_path("stdlib"))')"
    for python_module in fcntl math _json _posixsubprocess select _struct zlib; do
        for python_file in "$python_stdlib/lib-dynload/${python_module}"*.so; do
            if [[ -f "$python_file" ]]; then
                [[ -f "$extracted$python_file" ]] || { echo "Extracted initramfs is missing Python extension $python_file." >&2; rm -rf "$work"; exit 1; }
            fi
        done
    done
    for python_file in "$python_stdlib/json/__init__.py" "$python_stdlib/json/decoder.py" "$python_stdlib/json/encoder.py" "$python_stdlib/json/scanner.py"; do
        [[ -f "$extracted$python_file" ]] || { echo "Extracted initramfs is missing Python JSON closure $python_file." >&2; rm -rf "$work"; exit 1; }
    done
    python313_collections="$work/python313-collections"
    mkdir -p "$python313_collections/collections"
    cat "$extracted$python_stdlib/collections/__init__.py" > "$python313_collections/collections/__init__.py"
    printf '%s\n' 'import sys' 'import _collections_abc' 'abc = _collections_abc' 'sys.modules["collections.abc"] = _collections_abc' >> "$python313_collections/collections/__init__.py"

    runtime_output="$work/runtime-output"
    set +e
    PYTHONHOME="$extracted/usr" PYTHONPATH="$python313_collections" PYTHONNOUSERSITE=1 "$extracted/usr/bin/python3" "$extracted/usr/local/sbin/octessera-orange-oled-logo" invalid > "$runtime_output" 2>&1
    runtime_status=$?
    set -e
    [[ "$runtime_status" == 1 ]] || { echo "Extracted OLED Python runtime did not reject invalid input as expected." >&2; cat "$runtime_output" >&2; rm -rf "$work"; exit 1; }
    grep -qF 'usage: octessera-orange-oled-logo boot-once|boot-loop|resume|sleep|shutdown' "$runtime_output" || { echo "Extracted OLED Python runtime did not execute the installed script." >&2; cat "$runtime_output" >&2; rm -rf "$work"; exit 1; }
    ! grep -qE 'ModuleNotFoundError|ImportError' "$runtime_output" || { echo "Extracted OLED Python runtime has an incomplete import closure." >&2; cat "$runtime_output" >&2; rm -rf "$work"; exit 1; }
    rm -rf "$work"
    printf 'Orange initramfs extracted runtime validation passed\n'
}

run_extracted_runtime_test

run_initramfs_lifecycle_test() {
    local source_script="$root/userpatches/overlay/etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash"
    local setsid_path
    local work
    local marker
    local helper
    local fixture
    local mode
    local pid
    local child_pid

    setsid_path="$(command -v setsid || true)"
    [[ -n "$setsid_path" ]] || return 0
    [[ "$(id -u)" == 0 ]] || return 0
    for mode in success interruption timeout survivor marker-write-failure marker-rename-failure stale-marker; do
        work="$(mktemp -d)"
        marker="$work/marker"
        helper="$work/helper"
        fixture="$work/init-premount"
        case "$mode" in
            success|stale-marker|marker-write-failure|marker-rename-failure)
                printf '%s\n' '#!/bin/sh' 'exit 0' > "$helper"
                ;;
            interruption)
                printf '%s\n' '#!/bin/sh' 'sleep 10' > "$helper"
                ;;
            timeout|survivor)
                printf '%s\n' '#!/bin/sh' "sleep 10 & echo \"\$!\" > \"$work/child.pid\"; wait" > "$helper"
                ;;
        esac
        chmod 0755 "$helper"
        sed \
            -e "s|MARKER=/run/octessera-initramfs-splash.ready|MARKER=$marker|" \
            -e "s|TEMPORARY_MARKER=|TEMPORARY_MARKER=|" \
            -e "s|/usr/bin/setsid|$setsid_path|g" \
            -e "s|/usr/local/sbin/octessera-orange-oled-logo|$helper|g" \
            -e "s|/dev/kmsg|$work/kmsg|g" \
            "$source_script" > "$fixture"
        sed -i "s|TEMPORARY_MARKER=\"/run/.octessera-initramfs-splash.ready.tmp-\$\$\"|TEMPORARY_MARKER=\"$work/temp-marker-\$\$\"|" "$fixture"
        case "$mode" in
            marker-write-failure)
                sed -i '/printf.*bootId.*TEMPORARY_MARKER/c\  false' "$fixture"
                ;;
            marker-rename-failure)
                sed -i '/mv -f --.*TEMPORARY_MARKER.*MARKER/c\  false || return 1' "$fixture"
                ;;
            stale-marker)
                printf '%s\n' 'stale' > "$marker"
                ;;
        esac
        chmod 0755 "$fixture"
        if [[ "$mode" == interruption ]]; then
            PATH="$work/bin:$PATH" "$fixture" > "$work/output" 2>&1 &
            pid=$!
            sleep 0.2
            kill -TERM "$pid" 2>/dev/null || true
            wait "$pid" || true
        else
            PATH="$work/bin:$PATH" "$fixture" > "$work/output" 2>&1
        fi
        case "$mode" in
            success|stale-marker)
                grep -qF '"schema":1' "$marker"
                [[ -z "$(find "$work" -maxdepth 1 -name 'temp-marker-*' -print -quit)" ]]
                ;;
            timeout|survivor)
                child_pid="$(cat "$work/child.pid")"
                for _ in 1 2 3 4 5 6 7 8 9 10; do
                    kill -0 "$child_pid" 2>/dev/null || break
                    sleep 0.05
                done
                ! kill -0 "$child_pid" 2>/dev/null
                [[ ! -e "$marker" && -z "$(find "$work" -maxdepth 1 -name 'temp-marker-*' -print -quit)" ]]
                ;;
            *)
                [[ ! -e "$marker" && -z "$(find "$work" -maxdepth 1 -name 'temp-marker-*' -print -quit)" ]]
                ;;
        esac
        rm -rf "$work"
    done
    printf 'Orange initramfs lifecycle fixtures passed\n'
}

run_initramfs_lifecycle_test

printf 'Orange boot-splash hook tests passed\n'
