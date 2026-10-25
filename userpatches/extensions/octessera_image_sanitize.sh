#!/usr/bin/env bash

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

    if [[ ! -e "$home_path" ]]; then
        if ! mkdir -m 0755 -- "$home_path" || ! chown 0:0 -- "$home_path" || ! chmod 0755 -- "$home_path"; then
            printf '%s\n' "Octessera image sanitization could not create: $home_path" >&2
            return 1
        fi
    fi
    [[ "$(stat -c '%u:%g %a' -- "$home_path")" == '0:0 755' ]] || {
        printf '%s\n' "Octessera image sanitization requires exact metadata for: $home_path" >&2
        return 1
    }

    if [[ ! -e "$account_home_path" ]]; then
        if ! mkdir -m 0755 -- "$account_home_path" || ! chown "$account_uid:$account_gid" -- "$account_home_path" || ! chmod 0755 -- "$account_home_path"; then
            printf '%s\n' "Octessera image sanitization could not create: $account_home_path" >&2
            return 1
        fi
    fi
    [[ "$(stat -c '%u:%g %a' -- "$account_home_path")" == "$account_uid:$account_gid 755" ]] || {
        printf '%s\n' "Octessera image sanitization requires exact metadata for: $account_home_path" >&2
        return 1
    }

    if [[ ! -e "$hushlogin_path" ]]; then
        if ! : > "$hushlogin_path" || ! chown "$account_uid:$account_gid" -- "$hushlogin_path" || ! chmod 0644 -- "$hushlogin_path"; then
            printf '%s\n' "Octessera image sanitization could not create: $hushlogin_path" >&2
            return 1
        fi
    fi
    [[ "$(stat -c '%u:%g %a %s' -- "$hushlogin_path")" == "$account_uid:$account_gid 644 0" ]] || {
        printf '%s\n' "Octessera image sanitization requires exact metadata for: $hushlogin_path" >&2
        return 1
    }
}

function pre_umount_final_image__9999_octessera_image_sanitize() {
    local authorized_key_path
    local mount_root="${MOUNT:-}"
    local armbian_env_path
    local hostname_path
    local hosts_path
    local identity_path

    display_alert 'Octessera image sanitization' 'pre_umount_final_image__9999_octessera_image_sanitize' 'info'

    [[ -n "$mount_root" && -d "$mount_root" ]] || {
        printf '%s\n' 'Octessera image sanitization requires the Armbian MOUNT root.' >&2
        return 1
    }

    if ! octessera_ensure_account_home "$mount_root"; then
        return 1
    fi

    hostname_path="$mount_root/etc/hostname"
    hosts_path="$mount_root/etc/hosts"
    if ! printf '%s\n' 'octessera-opi' > "$hostname_path"; then
        printf '%s\n' "Octessera image sanitization could not write: $hostname_path" >&2
        return 1
    fi
    if ! printf '%s\n' \
        '127.0.0.1   localhost' \
        '127.0.1.1   octessera-opi' \
        '::1         localhost octessera-opi ip6-localhost ip6-loopback' \
        'fe00::0     ip6-localnet' \
        'ff00::0     ip6-mcastprefix' \
        'ff02::1     ip6-allnodes' \
        'ff02::2     ip6-allrouters' > "$hosts_path"; then
        printf '%s\n' "Octessera image sanitization could not write: $hosts_path" >&2
        return 1
    fi
    for identity_path in "$hostname_path" "$hosts_path"; do
        if ! chown root:root -- "$identity_path" || ! chmod 0644 -- "$identity_path"; then
            printf '%s\n' "Octessera image sanitization could not normalize: $identity_path" >&2
            return 1
        fi
    done

    armbian_env_path="$mount_root/boot/armbianEnv.txt"
    if ! chown root:root -- "$armbian_env_path" || ! chmod 0644 -- "$armbian_env_path"; then
        printf '%s\n' "Octessera image sanitization could not normalize: $armbian_env_path" >&2
        return 1
    fi

    for authorized_key_path in \
        "$mount_root/root/.ssh/authorized_keys" \
        "$mount_root/etc/ssh/authorized_keys" \
        "$mount_root/etc/dropbear/authorized_keys" \
        "$mount_root"/home/*/.ssh/authorized_keys; do
        rm -f -- "$authorized_key_path"
        if [[ -e "$authorized_key_path" || -L "$authorized_key_path" ]]; then
            printf '%s\n' "Octessera image sanitization left an authorization path: $authorized_key_path" >&2
            return 1
        fi
    done
}
