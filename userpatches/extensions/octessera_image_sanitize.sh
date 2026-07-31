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
