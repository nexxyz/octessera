#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$root/tools/armbian-image/validation-assertions.sh"
hook="$root/userpatches/overlay/etc/initramfs-tools/hooks/octessera-orange-boot-splash"
premount_script="$root/userpatches/overlay/etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash"
boot_service="$root/userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service"
shutdown_service="$root/userpatches/overlay/etc/systemd/system/octessera-orange-oled-shutdown.service"
suspend_service="$root/userpatches/overlay/etc/systemd/system/octessera-orange-oled-suspend.service"
python313_files="$root/tools/armbian-image/fixtures/python313-initramfs-closure-files.txt"
python313_fixture="$root/tools/armbian-image/fixtures/python313-initramfs-closure"
oled_logo="$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo"
oled_handoff="$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-handoff.py"
oled_lifecycle="$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-lifecycle.py"

[[ -f "$hook" ]] || { echo "Missing Orange initramfs boot-splash hook." >&2; exit 1; }
grep -qF '/usr/bin/setsid /usr/bin/python3 /usr/local/sbin/octessera-orange-oled-logo boot-static' "$premount_script" || { echo "Orange initramfs must invoke the renderer through /usr/bin/python3." >&2; exit 1; }
octessera_reject_file_match "Orange initramfs must not execute the renderer through its env shebang." -qF '/usr/bin/setsid /usr/local/sbin/octessera-orange-oled-logo boot-static' "$premount_script"
octessera_reject_file_match "Orange initramfs must not depend on /usr/bin/env." -qF '/usr/bin/env' "$premount_script"
for required_line in \
    'Type=notify' \
    'ConditionPathExists=/opt/octessera/current' \
    'NotifyAccess=main' \
    'Environment=OCTESSERA_OLED_READY_NOTIFY_REQUIRED=1' \
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
octessera_reject_file_match 'Orange boot splash must not restart always.' -q '^Restart=always$' "$root/userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service"
grep -qFx 'Restart=no' "$root/userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service" || { echo 'Orange boot splash restart policy changed.' >&2; exit 1; }
for required_line in \
    'DevicePolicy=closed' \
    'DeviceAllow=/dev/spidev1.0 rw' \
    'DeviceAllow=/dev/gpiochip1 rw'; do
    grep -qFx "$required_line" "$boot_service" || { echo "Orange boot service is missing: $required_line" >&2; exit 1; }
done
octessera_reject_file_match "Orange boot service must not conflict with runtime." -q '^Conflicts=' "$boot_service"
grep -qFx 'After=systemd-udev-trigger.service systemd-modules-load.service systemd-udevd.service local-fs.target' "$boot_service"
grep -qFx 'Before=sysinit.target octessera.service' "$boot_service"
for required_line in \
    'Type=oneshot' \
    'User=octessera-runtime' \
    'Group=octessera-runtime' \
    'ProtectSystem=strict' \
    'ReadWritePaths=/run/octessera-boot' \
    'DevicePolicy=closed' \
    'DeviceAllow=/dev/spidev1.0 rw' \
    'DeviceAllow=/dev/gpiochip1 rw' \
    'ExecStart=/bin/true' \
    "ExecStop=/bin/sh -c 'sleep 4; /usr/local/sbin/octessera-orange-oled-logo off || true'" \
    'RemainAfterExit=yes' \
    'TimeoutStopSec=8'; do
    grep -qFx "$required_line" "$shutdown_service" || { echo "Orange shutdown service is missing: $required_line" >&2; exit 1; }
done
octessera_reject_file_match 'Orange shutdown service must not use target ordering.' -q '^Before=' "$shutdown_service"
octessera_reject_file_match 'Orange shutdown service must not be enabled at shutdown targets.' -qE '^WantedBy=(shutdown|reboot|halt)\.target$' "$shutdown_service"
grep -qFx 'WantedBy=multi-user.target' "$shutdown_service" || { echo 'Orange shutdown service is not a multi-user service.' >&2; exit 1; }
octessera_reject_file_match 'Orange shutdown service must not write a logo.' -qE 'orange-oled-logo (shutdown|boot)' "$shutdown_service"
[[ ! -e "$root/userpatches/overlay/lib/systemd/system-sleep/octessera-orange-oled" ]] || { echo 'Orange system-sleep hook must be removed.' >&2; exit 1; }
grep -qF '["gpioset", "--chip", self.chip, f"{offset}={value}"]' "$oled_logo" || { echo 'Orange OLED GPIO control must use the fixed libgpiod v2 syntax.' >&2; exit 1; }
octessera_reject_file_match 'Orange OLED GPIO control must not use the removed libgpiod v1 mode option.' -qF -- '--mode=wait' "$oled_logo"
octessera_reject_file_match 'Orange OLED GPIO control must retain process-held ownership.' -qE -- '--(daemonize|toggle|hold-period)' "$oled_logo"
grep -qF 'def unlock_preserving' "$oled_handoff" || { echo 'Orange OLED handoff must preserve descriptors while unlocking.' >&2; exit 1; }
grep -qF 'def reacquire_nonblocking' "$oled_handoff" || { echo 'Orange OLED handoff must support nonblocking reclaim.' >&2; exit 1; }
grep -qF 'def _stream_frame' "$oled_lifecycle" || { echo 'Orange OLED lifecycle must gate readiness on a physical frame.' >&2; exit 1; }
grep -qF 'logo["notify_systemd_ready"]()' "$oled_lifecycle" || { echo 'Orange OLED lifecycle readiness notification is missing.' >&2; exit 1; }
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
octessera_reject_file_match 'Orange suspend service must use the runtime account without named supplementary groups.' -qFx 'SupplementaryGroups=audio i2c spi gpio' "$suspend_service"
octessera_reject_file_match 'Orange suspend service contains a forbidden lifecycle dependency.' -qE '(^|[[:space:]])(systemctl|dbus|Conflicts=)' "$suspend_service"

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
    mkdir -p "$systemd_work/etc/systemd/system/multi-user.target.wants" "$systemd_work/usr/local/sbin"
    cp "$boot_service" "$systemd_work/etc/systemd/system/octessera-orange-boot-splash.service"
    cp "$shutdown_service" "$systemd_work/etc/systemd/system/octessera-orange-oled-shutdown.service"
    cp "$suspend_service" "$systemd_work/etc/systemd/system/octessera-orange-oled-suspend.service"
    chmod 0644 "$systemd_work/etc/systemd/system/octessera-orange-boot-splash.service"
    chmod 0644 "$systemd_work/etc/systemd/system/octessera-orange-oled-shutdown.service"
    chmod 0644 "$systemd_work/etc/systemd/system/octessera-orange-oled-suspend.service"
    ln -s ../octessera-orange-oled-shutdown.service "$systemd_work/etc/systemd/system/multi-user.target.wants/octessera-orange-oled-shutdown.service"
    printf '%s\n' '#!/bin/sh' 'exit 0' > "$systemd_work/usr/local/sbin/octessera-orange-oled-logo"
    chmod 0755 "$systemd_work/usr/local/sbin/octessera-orange-oled-logo"
    printf '%s\n' '#!/bin/sh' 'exit 0' > "$systemd_work/usr/local/sbin/octessera-orange-oled-suspend"
    chmod 0755 "$systemd_work/usr/local/sbin/octessera-orange-oled-suspend"
    for unit in local-fs.target sysinit.target multi-user.target sleep.target; do
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
    printf '%s\n' '#!/bin/sh' 'exit 0' > "$systemd_work/bin/sh"
    chmod 0755 "$systemd_work/bin/true" "$systemd_work/bin/sh"
    systemd-analyze --root="$systemd_work" verify octessera-orange-boot-splash.service octessera-orange-oled-shutdown.service octessera-orange-oled-suspend.service
    rm -rf "$systemd_work"
fi
bash -n "$hook"
[[ "$(sh "$hook" prereqs)" == "" ]] || { echo "Orange boot-splash hook has unexpected prerequisites." >&2; exit 1; }

octessera_reject_file_match "Orange initramfs hook uses unavailable copy_dir." -qF 'copy_dir' "$hook"
octessera_reject_file_match "Orange initramfs hook must use the explicit Python runtime closure." -qF 'find ' "$hook"
grep -qF 'for python_dir in /usr/lib/python3.*;' "$hook" || { echo "Python standard-library path enumeration is missing." >&2; exit 1; }
grep -qF "\"\$python_dir/encodings/__init__.py\"" "$hook" || { echo "Python encoding runtime path is missing." >&2; exit 1; }
grep -qF "\"\$python_dir/re/_parser.py\"" "$hook" || { echo "Python regular-expression runtime path is missing." >&2; exit 1; }
grep -qF "copy_file binary \"\$python_file\" \"\$python_file\"" "$hook" || { echo "Python files are not copied as initramfs binaries." >&2; exit 1; }
grep -qF 'Orange initramfs hook missing Python target directory:' "$hook" || { echo "Missing fail-closed Python standard-library check." >&2; exit 1; }
grep -qF 'for python_module in fcntl math _json _posixsubprocess select _struct zlib; do' "$hook" || { echo "Python extension-module closure is missing." >&2; exit 1; }
grep -qF "/usr/bin/python3 -I -S - \"\$python_module\" \"\$python_dir\"" "$hook" || { echo "Python extension discovery must use isolated target Python." >&2; exit 1; }
grep -qF 'importlib.util.find_spec' "$hook" || { echo "Python extension discovery must require find_spec." >&2; exit 1; }
grep -qF 'importlib.machinery.ExtensionFileLoader' "$hook" || { echo "Python extension discovery must require an extension loader." >&2; exit 1; }
grep -qF 'importlib.machinery.EXTENSION_SUFFIXES' "$hook" || { echo "Python extension discovery must require an exact extension suffix." >&2; exit 1; }
grep -qF 'or origin.is_symlink()' "$hook" || { echo "Python extension discovery must reject symlinked origins." >&2; exit 1; }
grep -qF 'or origin.parent != dynload' "$hook" || { echo "Python extension discovery must keep origins in lib-dynload." >&2; exit 1; }
grep -qF 'or origin.name not in {module_name + suffix for suffix in importlib.machinery.EXTENSION_SUFFIXES}' "$hook" || { echo "Python extension discovery must reject wrong suffixes." >&2; exit 1; }
grep -qF "copy_exec \"\$python_origin\" \"\$python_origin\"" "$hook" || { echo "Validated Python extensions are not copied with dependencies." >&2; exit 1; }
octessera_reject_file_match "Python extension discovery must not use a glob fallback." -qF "for python_file in \"\$python_dir/lib-dynload" "$hook"
grep -qF "\"\$python_dir/_collections_abc.py\"" "$hook" || { echo "Python 3.13 _collections_abc.py closure path is missing." >&2; exit 1; }
octessera_reject_file_match "Removed Python 3.13 collections/abc.py path is still required." -qF "\"\$python_dir/collections/abc.py\"" "$hook"
grep -qF 'missing Python target file:' "$hook" || { echo "Missing Python target-file diagnostic is absent." >&2; exit 1; }
grep -qF 'rejected Python target module:' "$hook" || { echo "Rejected Python target-module diagnostic is absent." >&2; exit 1; }

[[ -f "$python313_files" ]] || { echo "Missing Python 3.13 initramfs closure fixture." >&2; exit 1; }
[[ -f "$python313_fixture/imports.py" ]] || { echo "Missing Python 3.13 closure import fixture." >&2; exit 1; }

for required_line in \
    'copy_exec /usr/local/sbin/octessera-orange-oled-logo /usr/local/sbin/octessera-orange-oled-logo' \
    'copy_file binary /usr/local/sbin/octessera-orange-oled-handoff.py /usr/local/sbin/octessera-orange-oled-handoff.py' \
    'copy_file binary /usr/local/sbin/octessera-orange-oled-lifecycle.py /usr/local/sbin/octessera-orange-oled-lifecycle.py' \
    'copy_exec /usr/bin/setsid /usr/bin/setsid' \
    'copy_exec /usr/bin/gpioset /usr/bin/gpioset' \
    'copy_file asset /usr/share/octessera/oled/octessera-pi-booting.rgb565' \
    'copy_file asset /usr/share/octessera/oled/octessera-pi-shutdown.rgb565'; do
    grep -qF "$required_line" "$hook" || { echo "Orange initramfs dependency was removed: $required_line" >&2; exit 1; }
done
octessera_reject_file_match "Obsolete Orange SVG initramfs dependency returned." -qF 'copy_file asset /usr/share/octessera/oled/octessera-mark.svg' "$hook"
octessera_reject_file_match "Obsolete Orange SVG initramfs dependency returned." -qF 'copy_file asset /usr/share/octessera/oled/octessera-wordmark.svg' "$hook"
for removed_line in \
    'manual_add_modules spi-sun6i' \
    'manual_add_modules spidev' \
    'manual_add_modules pinctrl-sunxi'; do
    octessera_reject_file_match "Obsolete Orange initramfs module addition returned: $removed_line" -qF "$removed_line" "$hook"
done
for removed_line in \
    'modprobe spi-sun6i' \
    'modprobe spidev' \
    'modprobe pinctrl-sunxi'; do
    octessera_reject_file_match "Obsolete Orange initramfs module load returned: $removed_line" -qF "$removed_line" "$root/userpatches/overlay/etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash"
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
    local python_runner
    local python_file
    local extension

    work="$(mktemp -d)"
    python_dir="$work/usr/lib/python3.13"
    hook_functions="$work/hook-functions"
    copy_log="$work/copy.log"
    fixture_hook="$work/hook"
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
    extension="$python_dir/lib-dynload/_json$(python3 -c 'import importlib.machinery; print(importlib.machinery.EXTENSION_SUFFIXES[0])')"
    : > "$extension"
    make_python_runner() {
        python_runner="$1"
        cat > "$python_runner" <<'EOF'
#!/bin/sh
exec python3 -c 'import importlib,importlib.machinery as m,importlib.util as u,os,sys;from pathlib import Path;mode=os.environ["OCTESSERA_PYTHON_FIXTURE_MODE"];module,python_dir=sys.argv[-2:];dynload=Path(python_dir)/"lib-dynload";suffix=m.EXTENSION_SUFFIXES[0];outside=Path(python_dir).parent.parent/"outside"/(module+suffix);origin=outside if mode=="outside" else dynload/(module+(".cpython-999-fixture.so" if mode=="wrong-suffix" else suffix));outside.parent.mkdir(parents=True,exist_ok=True);outside.touch();Path(origin).parent.mkdir(parents=True,exist_ok=True);Path(origin).unlink(missing_ok=True) if mode=="symlink" else Path(origin).touch();Path(origin).symlink_to(outside) if mode=="symlink" else None;spec=None if mode=="missing" else m.ModuleSpec(module,None,origin="built-in") if mode=="built-in" else m.ModuleSpec(module,m.ExtensionFileLoader(module,str(origin)),origin=str(origin));importlib.import_module=lambda _:None;u.find_spec=lambda _:spec;sys.argv=[sys.argv[0],module,python_dir];exec(compile(sys.stdin.read(),"<fixture-resolver>","exec"),{"__name__":"__main__"})' "$@"
EOF
        chmod 0755 "$python_runner"
    }
    make_fixture_hook() {
        local output="$1"
        local python_source="$2"
        sed \
            -e "s|^\. /usr/share/initramfs-tools/hook-functions$|. \"$hook_functions\"|" \
            -e "s|copy_exec /usr/bin/python3 /usr/bin/python3|copy_exec \"$python_source\" /usr/bin/python3|" \
            -e "s|/usr/bin/python3 -I -S|\"$python_source\" -I -S|" \
            -e "s|^for python_dir in /usr/lib/python3\.\*; do$|for python_dir in \"$python_dir\"; do|" \
            "$hook" > "$output"
        chmod 0755 "$output"
    }
    make_python_runner "$work/python3-built-in"
    make_fixture_hook "$fixture_hook" "$work/python3-built-in"
    if ! OCTESSERA_PYTHON_FIXTURE_MODE=built-in OCTESSERA_COPY_LOG="$copy_log" sh "$fixture_hook" >"$work/hook.out" 2>&1; then
        cat "$work/hook.out" >&2
        rm -rf "$work"
        exit 1
    fi
    grep -qF "copy_file binary $python_dir/_collections_abc.py $python_dir/_collections_abc.py" "$copy_log" || {
        echo "Python 3.13 fixture hook did not copy _collections_abc.py." >&2
        rm -rf "$work"
        exit 1
    }
    octessera_reject_file_match "Built-in Python fixture copied an extension." -qF 'lib-dynload' "$copy_log" || { rm -rf "$work"; exit 1; }
    octessera_reject_file_match "Python 3.13 fixture hook copied the removed collections/abc.py path." -qF 'collections/abc.py' "$copy_log"

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

    make_python_runner "$work/python3-extension"
    make_fixture_hook "$work/extension-hook" "$work/python3-extension"
    : > "$copy_log"
    if ! OCTESSERA_PYTHON_FIXTURE_MODE=extension OCTESSERA_COPY_LOG="$copy_log" sh "$work/extension-hook" >"$work/extension.out" 2>&1; then
        cat "$work/extension.out" >&2
        rm -rf "$work"
        exit 1
    fi
    grep -qF "copy_exec $extension $extension" "$copy_log" || { echo "In-tree Python extension fixture was not copied." >&2; rm -rf "$work"; exit 1; }
    for rejection in missing outside symlink wrong-suffix; do
        make_python_runner "$work/python3-$rejection"
        make_fixture_hook "$work/$rejection-hook" "$work/python3-$rejection"
        if OCTESSERA_PYTHON_FIXTURE_MODE="$rejection" OCTESSERA_COPY_LOG="$copy_log" sh "$work/$rejection-hook" >"$work/$rejection.out" 2>&1; then
            echo "Python $rejection-origin fixture was accepted." >&2
            rm -rf "$work"
            exit 1
        fi
        grep -qF 'rejected Python target module: fcntl' "$work/$rejection.out" || { echo "Python $rejection-origin rejection was not reported." >&2; rm -rf "$work"; exit 1; }
    done
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
    local boot_source
    local shutdown_source
    local handoff_source
    local fake_gpioset
    local python_stdlib
    local python_file
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
    boot_source="$root/userpatches/overlay/usr/local/share/octessera/oled/octessera-pi-booting.rgb565"
    shutdown_source="$root/userpatches/overlay/usr/local/share/octessera/oled/octessera-pi-shutdown.rgb565"
    handoff_source="$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-handoff.py"
    lifecycle_source="$root/userpatches/overlay/usr/local/sbin/octessera-orange-oled-lifecycle.py"
    fake_gpioset="$work/gpioset"
    mkdir -p "$destination" "$extracted" "$work/boot"
    printf '%s\n' '#!/bin/sh' 'exit 0' > "$fake_gpioset"
    chmod 0755 "$fake_gpioset"
    sed \
        -e "s|copy_exec /usr/local/sbin/octessera-orange-oled-logo /usr/local/sbin/octessera-orange-oled-logo|copy_exec \"$oled_source\" /usr/local/sbin/octessera-orange-oled-logo|" \
        -e "s|copy_file binary /usr/local/sbin/octessera-orange-oled-handoff.py /usr/local/sbin/octessera-orange-oled-handoff.py|copy_file binary \"$handoff_source\" /usr/local/sbin/octessera-orange-oled-handoff.py|" \
        -e "s|copy_file binary /usr/local/sbin/octessera-orange-oled-lifecycle.py /usr/local/sbin/octessera-orange-oled-lifecycle.py|copy_file binary \"$lifecycle_source\" /usr/local/sbin/octessera-orange-oled-lifecycle.py|" \
        -e "s|copy_exec /usr/bin/gpioset /usr/bin/gpioset|copy_exec \"$fake_gpioset\" /usr/bin/gpioset|" \
        -e "s|copy_file asset /usr/share/octessera/oled/octessera-pi-booting.rgb565|copy_file asset \"$boot_source\" /usr/share/octessera/oled/octessera-pi-booting.rgb565|" \
        -e "s|copy_file asset /usr/share/octessera/oled/octessera-pi-shutdown.rgb565|copy_file asset \"$shutdown_source\" /usr/share/octessera/oled/octessera-pi-shutdown.rgb565|" \
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
    grep -qF 'usr/local/sbin/octessera-orange-oled-lifecycle.py' "$work/contents" || { echo "Extracted initramfs is missing the OLED lifecycle module." >&2; rm -rf "$work"; exit 1; }
    octessera_reject_file_match "Extracted initramfs contains the obsolete mark SVG." -qF 'usr/share/octessera/oled/octessera-mark.svg' "$work/contents" || { rm -rf "$work"; exit 1; }
    octessera_reject_file_match "Extracted initramfs contains the obsolete wordmark SVG." -qF 'usr/share/octessera/oled/octessera-wordmark.svg' "$work/contents" || { rm -rf "$work"; exit 1; }
    grep -qF 'usr/share/octessera/oled/octessera-pi-booting.rgb565' "$work/contents" || { echo "Extracted initramfs is missing the boot frame asset." >&2; rm -rf "$work"; exit 1; }
    grep -qF 'usr/share/octessera/oled/octessera-pi-shutdown.rgb565' "$work/contents" || { echo "Extracted initramfs is missing the shutdown frame asset." >&2; rm -rf "$work"; exit 1; }
    unmkinitramfs "$archive" "$extracted"
    cmp "$oled_source" "$extracted/usr/local/sbin/octessera-orange-oled-logo"
    cmp "$handoff_source" "$extracted/usr/local/sbin/octessera-orange-oled-handoff.py"
    cmp "$lifecycle_source" "$extracted/usr/local/sbin/octessera-orange-oled-lifecycle.py"
    cmp "$boot_source" "$extracted/usr/share/octessera/oled/octessera-pi-booting.rgb565"
    cmp "$shutdown_source" "$extracted/usr/share/octessera/oled/octessera-pi-shutdown.rgb565"

    python_stdlib="$(python3 -c 'import sysconfig; print(sysconfig.get_path("stdlib"))')"
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
    grep -qF 'usage: octessera-orange-oled-logo boot-once|boot-static|boot-loop|resume|sleep|shutdown' "$runtime_output" || { echo "Extracted OLED Python runtime did not execute the installed script." >&2; cat "$runtime_output" >&2; rm -rf "$work"; exit 1; }
    octessera_reject_file_match "Extracted OLED Python runtime has an incomplete import closure." -qE 'ModuleNotFoundError|ImportError' "$runtime_output" || { cat "$runtime_output" >&2; rm -rf "$work"; exit 1; }
    rm -rf "$work"
    printf 'Orange initramfs extracted runtime validation passed\n'
}

run_extracted_runtime_test

printf 'Orange boot-splash hook tests passed\n'
