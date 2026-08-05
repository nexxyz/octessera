#!/bin/bash

require_octessera_trusted_parent_boot_layout() {
    local image_root="$1"
    local service="$image_root/etc/systemd/system/octessera-boot-splash.service"
    local runtime="$image_root/etc/systemd/system/octessera.service"
    local desired_link="$image_root/etc/systemd/system/sysinit.target.wants/octessera-boot-splash.service"
    local links=()
    local link
    local metadata
    local count

    metadata="$(stat -c '%u:%g:%a' "$service")"
    if [ "$metadata" != 0:0:644 ]; then
        echo "trusted-parent-v0.7.5: boot splash service must be root:root 0644" >&2
        return 1
    fi
    metadata="$(stat -c '%u:%g:%a' "$runtime")"
    if [ "$metadata" != 0:0:644 ]; then
        echo "trusted-parent-v0.7.5: runtime service must be root:root 0644" >&2
        return 1
    fi
    for required_line in \
        'Type=oneshot' \
        'After=systemd-modules-load.service systemd-udevd.service' \
        'Before=sysinit.target octessera.service' \
        'ExecStart=-/usr/local/bin/octessera-pi --boot-splash-once' \
        'TimeoutStartSec=2' \
        'WantedBy=sysinit.target'; do
        if ! grep -qxF "$required_line" "$service"; then
            echo "trusted-parent-v0.7.5: legacy boot service is missing $required_line" >&2
            return 1
        fi
    done
    for exact_line in \
        'Type=oneshot' \
        'ExecStart=-/usr/local/bin/octessera-pi --boot-splash-once'; do
        count="$(grep -cFx "$exact_line" "$service" || true)"
        if [ "$count" -ne 1 ]; then
            echo "trusted-parent-v0.7.5: legacy boot service has an extra or missing $exact_line" >&2
            return 1
        fi
    done
    if grep -Eq '^(Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1|ExecStart=/usr/local/bin/octessera-pi --boot-splash-loop)$' "$service" "$runtime"; then
        echo "trusted-parent-v0.7.5: legacy parent unexpectedly contains the v1 boot layer" >&2
        return 1
    fi
    if grep -Eq '^(Wants=octessera-boot-splash.service|After=octessera-boot-splash.service|Environment=OCTESSERA_OLED_BOOT_HANDOFF=)' "$runtime"; then
        echo "trusted-parent-v0.7.5: legacy runtime service unexpectedly contains the v1 handoff" >&2
        return 1
    fi
    if grep -Eq '^Conflicts=' "$service" "$runtime"; then
        echo "trusted-parent-v0.7.5: boot handoff services must not declare Conflicts" >&2
        return 1
    fi
    if [ ! -L "$desired_link" ] || [ "$(readlink "$desired_link")" != ../octessera-boot-splash.service ]; then
        echo "trusted-parent-v0.7.5: the legacy sysinit boot splash link is not exact" >&2
        return 1
    fi
    while IFS= read -r link; do
        [ -n "$link" ] && links+=("$link")
    done < <(find "$image_root/etc/systemd/system" -type l -name '*splash*.service' -print)
    if [ "${#links[@]}" -ne 1 ] || [ "${links[0]}" != "$desired_link" ]; then
        echo "trusted-parent-v0.7.5: legacy parent has more than one enabled early splash writer" >&2
        return 1
    fi
}

require_octessera_trusted_parent_file_identity() {
    local path="$1"
    local metadata="$2"
    local expected_hash="$3"
    local label="$4"
    local actual_hash
    if [ ! -f "$path" ] || [ -L "$path" ]; then
        echo "trusted-parent-v0.7.5: $label is missing, non-regular, or symlinked" >&2
        return 1
    fi
    if [ "$(stat -c '%u:%g:%a:%s' "$path")" != "$metadata" ]; then
        echo "trusted-parent-v0.7.5: $label metadata is not exact" >&2
        return 1
    fi
    actual_hash="$(sha256sum "$path" | cut -d' ' -f1)"
    if [ "$actual_hash" != "$expected_hash" ]; then
        echo "trusted-parent-v0.7.5: $label hash is not exact" >&2
        return 1
    fi
}

require_octessera_trusted_parent_raspberry_identity() {
    local boot_root="$1"
    local image_root="$2"
    local welcome="$image_root/etc/profile.d/octessera-welcome.sh"
    local utility="$image_root/usr/local/lib/octessera/rpi_uart_release.py"
    local boot_config="$boot_root/config.txt"
    local boot_cmdline="$boot_root/cmdline.txt"
    local firmware_config="$image_root/boot/firmware/config.txt"
    local legacy_config="$image_root/boot/config.txt"
    local config
    local cmdline
    local token
    local pi_record
    local pi_user
    local pi_gid
    local pi_home
    local pi_shell
    local hushlogin
    local mask
    local enablement
    local legacy_serial_console_count=0
    local tokens=()
    require_octessera_legal_notices "$image_root" || return 1

    if [ ! -f "$welcome" ] || [ -L "$welcome" ] || [ "$(stat -c '%u:%g:%a' "$welcome")" != 0:0:644 ] || [ ! -s "$welcome" ]; then
        echo "trusted-parent-v0.7.5: Raspberry legacy welcome file is not a nonempty root:root 0644 regular file" >&2
        return 1
    fi
    pi_record="$(awk -F: '$1 == "pi" { print; count++ } END { if (count != 1) exit 1 }' "$image_root/etc/passwd")" || {
        echo "trusted-parent-v0.7.5: Raspberry pi passwd entry is not exact" >&2
        return 1
    }
    IFS=: read -r pi_user _ _ pi_gid _ pi_home pi_shell <<< "$pi_record"
    if [ "$pi_user" != pi ] || [ "$pi_home" != /home/pi ] || [ "$pi_shell" != /bin/bash ] || [ ! -d "$image_root$pi_home" ] || [ -L "$image_root$pi_home" ]; then
        echo "trusted-parent-v0.7.5: Raspberry pi home or shell is not exact" >&2
        return 1
    fi
    if ! awk -F: -v gid="$pi_gid" '$1 == "pi" && $3 == gid { count++ } END { exit count != 1 }' "$image_root/etc/group"; then
        echo "trusted-parent-v0.7.5: Raspberry pi group is not exact" >&2
        return 1
    fi
    hushlogin="$image_root$pi_home/.hushlogin"
    if [ -e "$hushlogin" ] || [ -L "$hushlogin" ]; then
        echo "trusted-parent-v0.7.5: legacy Raspberry parent must not contain .hushlogin" >&2
        return 1
    fi
    if [ -e "$utility" ] || [ -L "$utility" ]; then
        echo "trusted-parent-v0.7.5: legacy Raspberry parent must not contain the UART release utility" >&2
        return 1
    fi
    for directory in "$image_root/etc/pam.d" "$image_root/etc/update-motd.d"; do
        if [ -d "$directory" ] && find -P "$directory" -type f -iname '*octessera*' -print -quit | grep -q .; then
            echo "trusted-parent-v0.7.5: Raspberry repository PAM or update-motd override remains" >&2
            return 1
        fi
    done
    if [ -e "$firmware_config" ] || [ -L "$firmware_config" ]; then
        echo "trusted-parent-v0.7.5: Raspberry firmware config must be absent" >&2
        return 1
    fi
    require_octessera_trusted_parent_file_identity \
        "$boot_config" \
        0:0:755:1847 \
        1018cf257f0b22c1dde87770d0433d0e3e2f442461db33f847307d427642fd9e \
        "selected FAT Raspberry config" || return 1
    require_octessera_trusted_parent_file_identity \
        "$legacy_config" \
        0:0:644:91 \
        c39b0866eec314a741f6cba65f10937b914408d6660d5a81f6b3a9ce81471010 \
        "rootfs Raspberry legacy config" || return 1
    require_octessera_trusted_parent_file_identity \
        "$boot_cmdline" \
        0:0:755:154 \
        284c0fe29f0f60cff7e0b9c370756f083148a6274e8cb445dcc5294e0a88bcd4 \
        "selected FAT Raspberry cmdline" || return 1
    config="$boot_config"
    cmdline="$boot_cmdline"
    if grep -qP '\x00' "$config" || grep -qP '\r(?!\n)' "$config"; then
        echo "trusted-parent-v0.7.5: Raspberry config is malformed" >&2
        return 1
    fi
    if grep -qF '# --- Octessera UART release ---' "$config"; then
        echo "trusted-parent-v0.7.5: legacy Raspberry parent contains the current UART release marker" >&2
        return 1
    fi
    for required_line in 'dtoverlay=disable-bt' 'enable_uart=0'; do
        if [ "$(grep -cFx "$required_line" "$config" || true)" -ne 1 ]; then
            echo "trusted-parent-v0.7.5: legacy Raspberry config is missing or duplicating $required_line" >&2
            return 1
        fi
    done
    if grep -Eq '^[[:space:]]*enable_uart[[:space:]]*=[[:space:]]*1([[:space:]]|$)' "$config"; then
        echo "trusted-parent-v0.7.5: Raspberry UART enablement remains" >&2
        return 1
    fi
    if [ ! -f "$cmdline" ] || [ -L "$cmdline" ]; then
        echo "trusted-parent-v0.7.5: Raspberry cmdline is missing or symlinked" >&2
        return 1
    fi
    if grep -qP '\x00' "$cmdline" || [ "$(grep -c '' "$cmdline")" -gt 1 ]; then
        echo "trusted-parent-v0.7.5: Raspberry cmdline is multiline or contains NUL" >&2
        return 1
    fi
    read -r -a tokens < "$cmdline"
    for token in "${tokens[@]}"; do
        if [ "$token" = console=serial0,115200 ]; then
            legacy_serial_console_count=$((legacy_serial_console_count + 1))
        elif [[ "$token" =~ ^console=(serial0|ttyAMA0|ttyS0)(,[^[:space:]]+)?$ ]]; then
            echo "trusted-parent-v0.7.5: unexpected legacy serial console token: $token" >&2
            return 1
        fi
    done
    if [ "$legacy_serial_console_count" -ne 1 ]; then
        echo "trusted-parent-v0.7.5: legacy Raspberry cmdline must contain exactly console=serial0,115200" >&2
        return 1
    fi
    for unit in serial0 ttyAMA0 ttyS0; do
        mask="$image_root/etc/systemd/system/serial-getty@$unit.service"
        if [ -e "$mask" ] || [ -L "$mask" ]; then
            echo "trusted-parent-v0.7.5: legacy Raspberry parent contains a current serial-getty mask: $unit" >&2
            return 1
        fi
    done
    for unit in hciuart bluetooth; do
        enablement="$image_root/etc/systemd/system/multi-user.target.wants/$unit.service"
        if [ -e "$enablement" ] || [ -L "$enablement" ]; then
            echo "trusted-parent-v0.7.5: legacy Raspberry parent contains Bluetooth service enablement: $unit" >&2
            return 1
        fi
    done
}
