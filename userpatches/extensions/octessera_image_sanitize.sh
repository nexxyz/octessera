#!/usr/bin/env bash

function octessera_validate_no_follow_path() {
    local mount_root="$1"
    local path="$2"
    local require_directory="${3:-false}"
    local allow_missing="${4:-false}"
    local relative_path
    local current_path="$mount_root"
    local component

    if [[ "$mount_root" == '/' ]]; then
        relative_path="${path#/}"
    else
        [[ "$path" == "$mount_root/"* ]] || return 1
        relative_path="${path#"$mount_root/"}"
    fi
    [[ -n "$relative_path" && "$relative_path" != /* ]] || return 1

    while [[ "$relative_path" == */* ]]; do
        component="${relative_path%%/*}"
        relative_path="${relative_path#*/}"
        [[ -n "$component" && "$component" != '.' && "$component" != '..' ]] || return 1
        current_path="$current_path/$component"
        if [[ ! -e "$current_path" && ! -L "$current_path" ]]; then
            [[ "$allow_missing" == true ]] && return 0
            return 1
        fi
        [[ -d "$current_path" && ! -L "$current_path" ]] || return 1
    done
    [[ -n "$relative_path" && "$relative_path" != '.' && "$relative_path" != '..' ]] || return 1

    if [[ "$require_directory" == true ]]; then
        current_path="$current_path/$relative_path"
        if [[ ! -e "$current_path" && ! -L "$current_path" ]]; then
            [[ "$allow_missing" == true ]] && return 0
            return 1
        fi
        [[ -d "$current_path" && ! -L "$current_path" ]] || return 1
    fi
}

function octessera_ensure_account_home() {
    local mount_root="$1"
    local passwd_path="$mount_root/etc/passwd"
    local group_path="$mount_root/etc/group"
    local home_path="$mount_root/home"
    local account_home_path="$home_path/octessera"
    local hushlogin_path="$account_home_path/.hushlogin"
    local account_record
    local account_pair
    local account_uid
    local account_gid
    local group_record
    local account_database_path

    for account_database_path in "$passwd_path" "$group_path"; do
        if ! octessera_validate_no_follow_path "$mount_root" "$account_database_path"; then
            printf '%s\n' "Octessera image sanitization found an unsafe or missing parent for: $account_database_path" >&2
            return 1
        fi
    done
    [[ -f "$passwd_path" && ! -L "$passwd_path" ]] || {
        printf '%s\n' "Octessera image sanitization requires a regular non-symlink file: $passwd_path" >&2
        return 1
    }
    [[ -f "$group_path" && ! -L "$group_path" ]] || {
        printf '%s\n' "Octessera image sanitization requires a regular non-symlink file: $group_path" >&2
        return 1
    }

    account_record="$(awk -F: '
        $1 == "octessera" {
            account_count++
            if (NF == 7 && $3 ~ /^[[:digit:]]+$/ && $4 ~ /^[[:digit:]]+$/ && $6 == "/home/octessera" && $7 == "/bin/bash") {
                valid_count++
                uid = $3
                gid = $4
            }
        }
        END { printf "%d:%d:%s:%s\n", account_count, valid_count, uid, gid }
    ' "$passwd_path")"
    [[ "$account_record" =~ ^1:1:[0-9]+:[0-9]+$ ]] || {
        printf '%s\n' 'Octessera image sanitization requires exactly one valid octessera passwd account.' >&2
        return 1
    }
    account_pair="${account_record#1:1:}"
    account_uid="${account_pair%%:*}"
    account_gid="${account_pair#*:}"

    group_record="$(awk -F: -v expected_gid="$account_gid" '
        $1 == "octessera" {
            group_count++
            if (NF == 4 && $3 ~ /^[[:digit:]]+$/ && $3 == expected_gid) {
                matching_count++
            }
        }
        END { printf "%d:%d\n", group_count, matching_count }
    ' "$group_path")"
    [[ "$group_record" == '1:1' ]] || {
        printf '%s\n' 'Octessera image sanitization requires exactly one matching octessera primary group.' >&2
        return 1
    }

    if ! octessera_validate_no_follow_path "$mount_root" "$home_path" true true; then
        printf '%s\n' "Octessera image sanitization found an unsafe or missing home directory: $home_path" >&2
        return 1
    fi
    if [[ ! -e "$home_path" && ! -L "$home_path" ]]; then
        if ! mkdir -m 0755 -- "$home_path" || ! chown 0:0 -- "$home_path" || ! chmod 0755 -- "$home_path"; then
            printf '%s\n' "Octessera image sanitization could not create: $home_path" >&2
            return 1
        fi
    fi
    if ! octessera_validate_no_follow_path "$mount_root" "$home_path" true; then
        printf '%s\n' "Octessera image sanitization found an unsafe home directory: $home_path" >&2
        return 1
    fi
    [[ "$(stat -c '%u:%g %a' -- "$home_path")" == '0:0 755' ]] || {
        printf '%s\n' "Octessera image sanitization requires exact metadata for: $home_path" >&2
        return 1
    }

    if ! octessera_validate_no_follow_path "$mount_root" "$account_home_path" true true; then
        printf '%s\n' "Octessera image sanitization found an unsafe or missing account home: $account_home_path" >&2
        return 1
    fi
    if [[ ! -e "$account_home_path" && ! -L "$account_home_path" ]]; then
        if ! mkdir -m 0755 -- "$account_home_path" || ! chown "$account_uid:$account_gid" -- "$account_home_path" || ! chmod 0755 -- "$account_home_path"; then
            printf '%s\n' "Octessera image sanitization could not create: $account_home_path" >&2
            return 1
        fi
    fi
    if ! octessera_validate_no_follow_path "$mount_root" "$account_home_path" true; then
        printf '%s\n' "Octessera image sanitization found an unsafe account home: $account_home_path" >&2
        return 1
    fi
    [[ "$(stat -c '%u:%g %a' -- "$account_home_path")" == "$account_uid:$account_gid 755" ]] || {
        printf '%s\n' "Octessera image sanitization requires exact metadata for: $account_home_path" >&2
        return 1
    }

    if ! octessera_validate_no_follow_path "$mount_root" "$hushlogin_path"; then
        printf '%s\n' "Octessera image sanitization found an unsafe or missing parent for: $hushlogin_path" >&2
        return 1
    fi
    if [[ ! -e "$hushlogin_path" && ! -L "$hushlogin_path" ]]; then
        if ! : > "$hushlogin_path" || ! chown "$account_uid:$account_gid" -- "$hushlogin_path" || ! chmod 0644 -- "$hushlogin_path"; then
            printf '%s\n' "Octessera image sanitization could not create: $hushlogin_path" >&2
            return 1
        fi
    fi
    [[ -f "$hushlogin_path" && ! -L "$hushlogin_path" ]] || {
        printf '%s\n' "Octessera image sanitization requires a regular non-symlink file: $hushlogin_path" >&2
        return 1
    }
    [[ "$(stat -c '%u:%g %a %s' -- "$hushlogin_path")" == "$account_uid:$account_gid 644 0" ]] || {
        printf '%s\n' "Octessera image sanitization requires exact metadata for: $hushlogin_path" >&2
        return 1
    }
}

function pre_umount_final_image__9999_octessera_image_sanitize() {
    local authorized_key_path
    local mount_root="${MOUNT:-}"
    local removal_failed=0
    local armbian_env_path
    local home_path
    local home_ssh_path

    display_alert 'Octessera image sanitization' 'pre_umount_final_image__9999_octessera_image_sanitize' 'info'

    [[ -n "$mount_root" && -d "$mount_root" ]] || {
        printf '%s\n' 'Octessera image sanitization requires the Armbian MOUNT root.' >&2
        return 1
    }
    if ! mount_root="$(cd -P -- "$mount_root" 2>/dev/null && pwd -P)" || [[ -z "$mount_root" || ! -d "$mount_root" || -L "$mount_root" ]]; then
        printf '%s\n' 'Octessera image sanitization could not canonicalize the Armbian MOUNT root.' >&2
        return 1
    fi

    if ! octessera_ensure_account_home "$mount_root"; then
        return 1
    fi

    armbian_env_path="$mount_root/boot/armbianEnv.txt"
    if ! octessera_validate_no_follow_path "$mount_root" "$armbian_env_path"; then
        printf '%s\n' "Octessera image sanitization found an unsafe or missing parent for: $armbian_env_path" >&2
        return 1
    fi
    [[ -f "$armbian_env_path" && ! -L "$armbian_env_path" ]] || {
        printf '%s\n' "Octessera image sanitization requires a regular non-symlink file: $armbian_env_path" >&2
        return 1
    }

    local -a authorized_key_paths=(
        "$mount_root/root/.ssh/authorized_keys"
        "$mount_root/etc/ssh/authorized_keys"
        "$mount_root/etc/dropbear/authorized_keys"
    )

    for authorized_key_path in "${authorized_key_paths[@]}"; do
        if ! octessera_validate_no_follow_path "$mount_root" "$authorized_key_path" false true; then
            printf '%s\n' "Octessera image sanitization found an unsafe or missing parent for: $authorized_key_path" >&2
            return 1
        fi
    done

    if ! octessera_validate_no_follow_path "$mount_root" "$mount_root/home" true true; then
        printf '%s\n' "Octessera image sanitization found an unsafe or missing home directory: $mount_root/home" >&2
        return 1
    fi
    for home_path in "$mount_root"/home/*; do
        if [[ -e "$home_path" || -L "$home_path" ]]; then
            if ! octessera_validate_no_follow_path "$mount_root" "$home_path" true; then
                printf '%s\n' "Octessera image sanitization found an unsafe or missing home directory: $home_path" >&2
                return 1
            fi
            home_ssh_path="$home_path/.ssh"
            if [[ -e "$home_ssh_path" || -L "$home_ssh_path" ]] && ! octessera_validate_no_follow_path "$mount_root" "$home_ssh_path" true; then
                printf '%s\n' "Octessera image sanitization found an unsafe or missing SSH directory: $home_ssh_path" >&2
                return 1
            fi
        fi
    done

    for authorized_key_path in "$mount_root"/home/*/.ssh/authorized_keys; do
        if [[ -e "$authorized_key_path" || -L "$authorized_key_path" ]]; then
            if ! octessera_validate_no_follow_path "$mount_root" "$authorized_key_path"; then
                printf '%s\n' "Octessera image sanitization found an unsafe or missing parent for: $authorized_key_path" >&2
                return 1
            fi
            authorized_key_paths+=("$authorized_key_path")
        fi
    done

    if ! chown root:root -- "$armbian_env_path" || ! chmod 0644 -- "$armbian_env_path"; then
        printf '%s\n' "Octessera image sanitization could not normalize: $armbian_env_path" >&2
        return 1
    fi

    for authorized_key_path in "${authorized_key_paths[@]}"; do
        if ! rm -f -- "$authorized_key_path" 2>/dev/null; then
            removal_failed=1
        fi
    done

    for authorized_key_path in "${authorized_key_paths[@]}"; do
        if [[ "$removal_failed" != 0 || -e "$authorized_key_path" || -L "$authorized_key_path" ]]; then
            printf '%s\n' 'Octessera image sanitization left an authorization path.' >&2
            printf '%s\n' "$authorized_key_path" >&2
            return 1
        fi
    done
}
