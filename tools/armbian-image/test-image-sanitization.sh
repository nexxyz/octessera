#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
extension="$root/userpatches/extensions/octessera_image_sanitize.sh"
customize="$root/userpatches/customize-image.sh"
inspector="$root/tools/armbian-image/inspect-built-image.sh"
authorized_key_paths_helper="$root/tools/armbian-image/authorized-key-paths.sh"
inspect_path_helper="$root/tools/armbian-image/inspect-path.sh"

[[ -f "$extension" ]] || { echo "Missing Armbian image sanitization extension." >&2; exit 1; }
[[ -f "$authorized_key_paths_helper" ]] || { echo "Missing authorized-key path helper." >&2; exit 1; }
[[ -f "$inspect_path_helper" ]] || { echo "Missing inspect-path helper." >&2; exit 1; }
bash -n "$extension"
bash -n "$authorized_key_paths_helper"
bash -n "$inspect_path_helper"
grep -qF 'function pre_umount_final_image__9999_octessera_image_sanitize' "$extension" || {
    echo "Image sanitization must use the pinned Armbian pre_umount_final_image hook contract." >&2
    exit 1
}
grep -qF "local mount_root=\"\${MOUNT:-}\"" "$extension" || {
    echo "Image sanitization must use Armbian's MOUNT root." >&2
    exit 1
}
grep -qF 'function octessera_validate_no_follow_path' "$extension" || {
    echo "Image sanitization must validate paths without following intermediate symlinks." >&2
    exit 1
}
grep -qF "cd -P -- \"\$mount_root\"" "$extension" || {
    echo "Image sanitization must canonicalize the MOUNT root." >&2
    exit 1
}
grep -qF "[[ -d \"\$current_path\" && ! -L \"\$current_path\" ]]" "$extension" || {
    echo "Image sanitization must reject symlinked parent directories." >&2
    exit 1
}
grep -qF "\"\$mount_root\"/home/*/.ssh/authorized_keys" "$extension" || {
    echo "Image sanitization must cover supported /home/* account homes." >&2
    exit 1
}
for explicit_path in '/root/.ssh/authorized_keys' '/etc/ssh/authorized_keys' '/etc/dropbear/authorized_keys'; do
    grep -qF "$explicit_path" "$extension" || {
        echo "Image sanitization is missing explicit path: $explicit_path." >&2
        exit 1
    }
done
if grep -Eq '(^|[[:space:]])(cat|read|grep|sha(1|224|256|384|512)sum|md5sum|od|hexdump|base64|strings)([[:space:]]|$)' "$extension"; then
    echo "Image sanitization must not read, hash, or inspect authorization contents." >&2
    exit 1
fi
grep -qF "rm -f -- \"\$authorized_key_path\"" "$extension" || {
    echo "Image sanitization must remove only its explicit authorization paths." >&2
    exit 1
}
grep -qF 'return 1' "$extension" || {
    echo "Image sanitization must fail closed when authorization paths remain." >&2
    exit 1
}
grep -qF "[[ -f \"\$armbian_env_path\" && ! -L \"\$armbian_env_path\" ]]" "$extension" || {
    echo "Image sanitization must require a regular non-symlink armbianEnv.txt." >&2
    exit 1
}
grep -qF "chown root:root -- \"\$armbian_env_path\"" "$extension" || {
    echo "Image sanitization must normalize armbianEnv.txt ownership." >&2
    exit 1
}
grep -qF "chmod 0644 -- \"\$armbian_env_path\"" "$extension" || {
    echo "Image sanitization must normalize armbianEnv.txt mode." >&2
    exit 1
}
grep -qF 'rm -f /root/.ssh/authorized_keys /home/octessera/.ssh/authorized_keys' "$customize" || {
    echo "Existing early customizer authorization cleanup must remain." >&2
    exit 1
}
grep -qF 'local key_paths=(root/.ssh/authorized_keys etc/ssh/authorized_keys etc/dropbear/authorized_keys)' "$inspector" || {
    echo "Built-image inspection must retain its explicit authorization paths." >&2
    exit 1
}
grep -qF "key_paths+=(\"\$key_path\")" "$inspector" || {
    echo "Built-image inspection must retain account-home authorization checks." >&2
    exit 1
}
grep -qF "octessera_stat_path \"\$target\" \"\$1\"" "$inspector" || {
    echo "Built-image inspection must use metadata-aware path inspection." >&2
    exit 1
}

# shellcheck disable=SC1090
source "$authorized_key_paths_helper"
# shellcheck disable=SC1090
source "$inspect_path_helper"
grep -qF "printf '%s\\n' \"\${home#/}/.ssh/authorized_keys\"" "$authorized_key_paths_helper" || {
    echo "The account-home helper must derive supported authorization paths." >&2
    exit 1
}
login_defs_fixture='UID_MIN 1000'
uid_min="$(octessera_uid_min "$login_defs_fixture")"
[[ "$uid_min" == 1000 ]] || {
    echo "Configured UID_MIN was not loaded." >&2
    exit 1
}
[[ "$(octessera_uid_min '')" == 1000 ]] || {
    echo "Missing UID_MIN did not use the default." >&2
    exit 1
}
passwd_fixture=$'root:x:0:0:root:/root:/bin/bash\noctessera:x:1000:1000:Octessera:/home/octessera:/bin/bash\nsystemd-network:x:996:996:systemd Network Management:/run/systemd/netif:/usr/sbin/nologin\nsync:x:4:65534:sync:/bin:/bin/sync\ndaemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin\nsystemd-coredump:x:995:995:systemd Core Dumper:/run/systemd:/usr/sbin/nologin\nsystemd-sysusers:x:994:994:systemd Users:/usr:/usr/sbin/nologin\nsystemd-root:x:993:993:systemd Root:/:/usr/sbin/nologin'
passwd_fixture+=$'\nnobody:x:65534:65534:nobody:/nonexistent:/usr/sbin/nologin'
passwd_fixture+=$'\nlocked-sbin:x:65533:65533:Locked:/opt/locked:/sbin/nologin\nlocked-false:x:65532:65532:Locked:/opt/locked-false:/bin/false'
derived_key_paths="$(octessera_derive_account_authorized_key_paths "$passwd_fixture" "$uid_min")"
[[ "$derived_key_paths" == 'home/octessera/.ssh/authorized_keys' ]] || {
    echo "System account home handling derived an unsafe or incomplete authorization path list." >&2
    exit 1
}
unsupported_passwd='human:x:1001:1001:Unknown Human:/opt/human:/bin/bash'
if unsupported_error="$(octessera_derive_account_authorized_key_paths "$unsupported_passwd" "$uid_min" 2>&1 >/dev/null)"; then
    echo "Built-image inspection accepted an unsupported interactive account home." >&2
    exit 1
fi
[[ "$unsupported_error" == 'Unsupported non-system account home for user human (UID 1001): /opt/human.' ]] || {
    echo "Unsupported interactive account home rejection was not explicit." >&2
    exit 1
}

fixture_work="$(mktemp -d)"
final_fixture="$(mktemp -d)"
stat_fixture_work="$(mktemp -d)"
symlink_fixture_work="$(mktemp -d)"
trap 'rm -rf "$fixture_work" "$final_fixture" "$stat_fixture_work" "$symlink_fixture_work"' EXIT
stat_fixture="$stat_fixture_work/rootfs.ext4"
for required_command in debugfs mkfs.ext4 truncate; do
    command -v "$required_command" >/dev/null 2>&1 || {
        echo "Missing required ext4 fixture command: $required_command." >&2
        exit 1
    }
done
truncate -s 16M "$stat_fixture"
mkfs.ext4 -q -F "$stat_fixture"
missing_status=0
if octessera_stat_path "$stat_fixture" root/.ssh/authorized_keys; then
    echo "Missing ext4 authorization path was reported as present." >&2
    exit 1
else
    missing_status=$?
fi
[[ "$missing_status" == 1 || "$missing_status" == 2 ]] || {
    echo "Missing ext4 authorization path returned an unexpected inspection status." >&2
    exit 1
}
debugfs -w -R 'mkdir /root' "$stat_fixture" >/dev/null 2>&1 || {
    echo "Could not create the ext4 fixture root directory." >&2
    exit 1
}
debugfs -w -R 'mkdir /root/.ssh' "$stat_fixture" >/dev/null 2>&1 || {
    echo "Could not create the ext4 authorization fixture directory." >&2
    exit 1
}
printf '%s\n' 'fixture key placeholder' > "$stat_fixture_work/authorized_keys"
debugfs -w -R "write $stat_fixture_work/authorized_keys /root/.ssh/authorized_keys" "$stat_fixture" >/dev/null 2>&1 || {
    echo "Could not create the ext4 authorization fixture file." >&2
    exit 1
}
if ! octessera_stat_path "$stat_fixture" root/.ssh/authorized_keys; then
    echo "Present ext4 authorization path was not reported as present." >&2
    exit 1
fi
error_status=0
if octessera_stat_path "$stat_fixture_work/authorized_keys" root/.ssh/authorized_keys 2>"$stat_fixture_work/error.stderr"; then
    echo "Invalid ext4 target was reported as inspectable." >&2
    exit 1
else
    error_status=$?
fi
[[ "$error_status" == 2 ]] || {
    echo "Invalid ext4 target did not preserve inspection-error handling." >&2
    exit 1
}
mkdir -p \
    "$fixture_work/root/.ssh" \
    "$fixture_work/home/octessera/.ssh" \
    "$fixture_work/home/other-account/.ssh" \
    "$fixture_work/home/other-account/nested/.ssh" \
    "$fixture_work/boot" \
    "$fixture_work/etc/ssh" \
    "$fixture_work/etc/dropbear" \
    "$fixture_work/etc/other"
printf '%s\n' "$passwd_fixture" > "$fixture_work/etc/passwd"
printf '%s\n' "$login_defs_fixture" > "$fixture_work/etc/login.defs"
armbian_env_path="$fixture_work/boot/armbianEnv.txt"
printf '%s\n' 'verbosity=1' > "$armbian_env_path"
chmod 0600 "$armbian_env_path"

# These represent files created after the early customizer cleanup.
: > "$fixture_work/root/.ssh/authorized_keys"
printf '%s\n' 'late fixture placeholder' > "$fixture_work/home/octessera/.ssh/authorized_keys"
: > "$fixture_work/home/other-account/.ssh/authorized_keys"
printf '%s\n' 'late fixture placeholder' > "$fixture_work/etc/ssh/authorized_keys"
: > "$fixture_work/etc/dropbear/authorized_keys"
printf '%s\n' 'must remain outside the allowlist' > "$fixture_work/home/other-account/nested/.ssh/authorized_keys"
printf '%s\n' 'must remain outside the allowlist' > "$fixture_work/etc/other/authorized_keys"
[[ ! -s "$fixture_work/root/.ssh/authorized_keys" && ! -s "$fixture_work/home/other-account/.ssh/authorized_keys" ]] || {
    echo "Zero-byte late authorization fixtures were not created." >&2
    exit 1
}
[[ -s "$fixture_work/home/octessera/.ssh/authorized_keys" && -s "$fixture_work/etc/ssh/authorized_keys" ]] || {
    echo "Nonzero late authorization fixtures were not created." >&2
    exit 1
}

export MOUNT="$fixture_work"
hook_trace="$fixture_work/pre-umount.trace"
hook_runner="$fixture_work/run-sanitization-hook.sh"
cat > "$hook_runner" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

extension="$1"
mount_root="$2"
hook_trace="$3"
# shellcheck disable=SC1090,SC1091
source "$extension"
display_alert() { :; }
export MOUNT="$mount_root"
pre_umount_final_image() {
    printf '%s\n' 'pre_umount_final_image' > "$hook_trace"
    pre_umount_final_image__9999_octessera_image_sanitize
}
pre_umount_final_image
EOF
chmod 0755 "$hook_runner"
run_as_root() {
    if [[ "$(id -u)" == 0 ]]; then
        "$@"
        return
    fi
    command -v sudo >/dev/null 2>&1 || { echo "Root privileges are required for image sanitization tests." >&2; exit 1; }
    sudo -n -- "$@"
}
run_as_root "$hook_runner" "$extension" "$fixture_work" "$hook_trace"
grep -qFx 'pre_umount_final_image' "$hook_trace" || {
    echo "The pinned pre_umount_final_image call sequence did not execute." >&2
    exit 1
}
[[ "$(stat -c '%u:%g' "$armbian_env_path")" == '0:0' && "$(stat -c '%a' "$armbian_env_path")" == 644 ]] || {
    echo "Image sanitization did not normalize armbianEnv.txt ownership and mode." >&2
    exit 1
}

for removed_path in \
    "$fixture_work/root/.ssh/authorized_keys" \
    "$fixture_work/home/octessera/.ssh/authorized_keys" \
    "$fixture_work/home/other-account/.ssh/authorized_keys" \
    "$fixture_work/etc/ssh/authorized_keys" \
    "$fixture_work/etc/dropbear/authorized_keys"; do
    [[ ! -e "$removed_path" && ! -L "$removed_path" ]] || {
        echo "Image sanitization left an allowlisted path: $removed_path." >&2
        exit 1
    }
done
for retained_path in \
    "$fixture_work/home/other-account/nested/.ssh/authorized_keys" \
    "$fixture_work/etc/other/authorized_keys"; do
    [[ -e "$retained_path" ]] || {
        echo "Image sanitization removed a path outside its allowlist: $retained_path." >&2
        exit 1
    }
done

run_outside_symlink_case() {
    local name="$1"
    local link_relative_path="$2"
    local sentinel_relative_path="$3"
    local case_work="$symlink_fixture_work/$name"
    local case_root="$case_work/rootfs"
    local outside_root="$case_work/outside"
    local link_path="$case_root/$link_relative_path"
    local sentinel_path="$outside_root/$sentinel_relative_path"
    local expected_sentinel="$case_work/expected-sentinel"
    local hook_trace="$case_root/symlink-case.trace"
    local sentinel_metadata

    mkdir -p "$case_root" "$outside_root"
    cp -a "$fixture_work/." "$case_root/"
    case "$sentinel_relative_path" in
        */*) mkdir -p "$outside_root/${sentinel_relative_path%/*}" ;;
    esac
    printf '%s\n' "outside sentinel $name" > "$expected_sentinel"
    cp "$expected_sentinel" "$sentinel_path"
    chmod 0600 "$sentinel_path"
    sentinel_metadata="$(stat -c '%u:%g %a' "$sentinel_path")"
    rm -rf "$link_path"
    ln -s "$outside_root" "$link_path"

    if run_as_root "$case_root/run-sanitization-hook.sh" "$extension" "$case_root" "$hook_trace"; then
        echo "Image sanitization accepted an outside symlinked parent: $link_relative_path." >&2
        exit 1
    fi
    [[ -f "$sentinel_path" ]] || {
        echo "Image sanitization removed an outside sentinel: $sentinel_path." >&2
        exit 1
    }
    cmp "$expected_sentinel" "$sentinel_path"
    [[ "$(stat -c '%u:%g %a' "$sentinel_path")" == "$sentinel_metadata" ]] || {
        echo "Image sanitization changed an outside sentinel: $sentinel_path." >&2
        exit 1
    }
}

symlink_probe_target="$symlink_fixture_work/symlink-probe-target"
symlink_probe="$symlink_fixture_work/symlink-probe"
if ln -s "$symlink_probe_target" "$symlink_probe" 2>/dev/null; then
    rm -f "$symlink_probe"
    run_outside_symlink_case root_ssh root/.ssh authorized_keys
    run_outside_symlink_case etc_ssh etc/ssh authorized_keys
    run_outside_symlink_case etc_dropbear etc/dropbear authorized_keys
    run_outside_symlink_case home_account home/escape .ssh/authorized_keys
    run_outside_symlink_case home_ssh home/octessera/.ssh authorized_keys
    run_outside_symlink_case boot boot armbianEnv.txt
else
    echo "Outside-mount symlink sentinel tests skipped: symlinks unavailable." >&2
fi

for missing_optional_parent in root/.ssh etc/ssh etc/dropbear; do
    rm -rf "${fixture_work:?}/$missing_optional_parent"
    if ! run_as_root "$hook_runner" "$extension" "$fixture_work" "$hook_trace" 2>"$fixture_work/missing-${missing_optional_parent//\//-}.stderr"; then
        echo "Image sanitization rejected a missing optional authorization parent: $missing_optional_parent." >&2
        exit 1
    fi
    mkdir -p "$fixture_work/$missing_optional_parent"
done

rm "$armbian_env_path"
if run_as_root "$hook_runner" "$extension" "$fixture_work" "$hook_trace" 2>"$fixture_work/missing-armbian-env.stderr"; then
    echo "Image sanitization accepted a missing armbianEnv.txt." >&2
    exit 1
fi

symlink_target="$fixture_work/boot/armbianEnv.target"
printf '%s\n' 'verbosity=1' > "$symlink_target"
if ln -s armbianEnv.target "$armbian_env_path" 2>/dev/null; then
    if run_as_root "$hook_runner" "$extension" "$fixture_work" "$hook_trace" 2>"$fixture_work/symlink-armbian-env.stderr"; then
        echo "Image sanitization accepted a symlinked armbianEnv.txt." >&2
        exit 1
    fi
    rm -f "$armbian_env_path"
else
    echo "Armbian environment symlink test skipped: symlinks unavailable." >&2
fi
rm -f "$symlink_target"
printf '%s\n' 'verbosity=1' > "$armbian_env_path"
chmod 0600 "$armbian_env_path"

cp -a "$fixture_work/." "$final_fixture/"
[[ ! -e "$final_fixture/root/.ssh/authorized_keys" && ! -L "$final_fixture/root/.ssh/authorized_keys" ]] || {
    echo "The final rootfs copy retained an allowlisted authorization path." >&2
    exit 1
}

mkdir -p "$fixture_work/root/.ssh/authorized_keys"
if run_as_root "$hook_runner" "$extension" "$fixture_work" "$hook_trace" 2>"$fixture_work/failure.stderr"; then
    echo "Image sanitization accepted a remaining authorization path." >&2
    exit 1
fi
[[ -d "$fixture_work/root/.ssh/authorized_keys" ]] || {
    echo "Image sanitization fixture did not retain the failed directory path." >&2
    exit 1
}

printf 'Armbian image sanitization fixture and static tests passed\n'
