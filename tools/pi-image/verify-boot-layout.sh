#!/bin/bash

REPOSITORY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

require_octessera_boot_service_layout() {
    local image_root="$1"
    local service="$image_root/etc/systemd/system/octessera-boot-splash.service"
    local runtime="$image_root/etc/systemd/system/octessera.service"
    local wants_dir="$image_root/etc/systemd/system/sysinit.target.wants"
    local desired_link="$wants_dir/octessera-boot-splash.service"
    local links=()
    local link
    local metadata
    local count

    if [ ! -f "$service" ] || [ ! -f "$runtime" ]; then
        echo "constructor-required: Raspberry v1 boot service output is missing" >&2
        return 1
    fi
    metadata="$(stat -c '%u:%g:%a' "$service")"
    if [ "$metadata" != 0:0:644 ]; then
        echo "constructor-required: boot splash service must be root:root 0644" >&2
        return 1
    fi
    metadata="$(stat -c '%u:%g:%a' "$runtime")"
    if [ "$metadata" != 0:0:644 ]; then
        echo "constructor-required: runtime service must be root:root 0644" >&2
        return 1
    fi
    for required_line in \
        'Type=simple' \
        'User=pi' \
        'Group=pi' \
        'Wants=systemd-udev-settle.service' \
        'Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1' \
        'RuntimeDirectory=octessera-boot' \
        'RuntimeDirectoryMode=0750' \
        'RuntimeDirectoryPreserve=yes' \
        'UMask=0027' \
        'KillMode=control-group' \
        'TimeoutStopSec=2' \
        'Restart=no' \
        'ExecStart=/usr/local/bin/octessera-pi --boot-splash-loop' \
        'DevicePolicy=closed' \
        'DeviceAllow=/dev/spidev0.0 rw' \
        'DeviceAllow=/dev/gpiomem rw' \
        'DeviceAllow=/dev/gpiochip0 rw'; do
        if ! grep -qxF "$required_line" "$service"; then
            echo "constructor-required: boot splash service is missing $required_line" >&2
            return 1
        fi
    done
    for exact_line in \
        'Type=simple' \
        'Wants=systemd-udev-settle.service' \
        'ExecStart=/usr/local/bin/octessera-pi --boot-splash-loop' \
        'Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1' \
        'DevicePolicy=closed' \
        'DeviceAllow=/dev/spidev0.0 rw' \
        'DeviceAllow=/dev/gpiomem rw' \
        'DeviceAllow=/dev/gpiochip0 rw'; do
        count="$(grep -cFx "$exact_line" "$service" || true)"
        if [ "$count" -ne 1 ]; then
            echo "constructor-required: boot splash service has an extra or missing $exact_line" >&2
            return 1
        fi
    done
    for required_line in \
        'After=systemd-modules-load.service systemd-udevd.service systemd-udev-trigger.service systemd-udev-settle.service' \
        'Before=sysinit.target octessera.service'; do
        if ! grep -qxF "$required_line" "$service"; then
            echo "constructor-required: boot splash ordering is missing $required_line" >&2
            return 1
        fi
    done
    for required_line in \
        'Wants=octessera-boot-splash.service' \
        'After=octessera-boot-splash.service' \
        'Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1'; do
        if ! grep -qxF "$required_line" "$runtime"; then
            echo "constructor-required: runtime service is missing $required_line" >&2
            return 1
        fi
    done
    count="$(grep -cFx 'Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1' "$runtime" || true)"
    if [ "$count" -ne 1 ]; then
        echo "constructor-required: runtime service has an extra or missing OLED handoff environment" >&2
        return 1
    fi
    if grep -Eq '^Conflicts=' "$service" "$runtime"; then
        echo "constructor-required: boot handoff services must not declare Conflicts" >&2
        return 1
    fi
    if [ ! -L "$desired_link" ] || [ "$(readlink "$desired_link")" != ../octessera-boot-splash.service ]; then
        echo "constructor-required: the sysinit boot splash link is not exact" >&2
        return 1
    fi
    while IFS= read -r link; do
        [ -n "$link" ] && links+=("$link")
    done < <(find "$image_root/etc/systemd/system" -type l -name '*splash*.service' -print)
    if [ "${#links[@]}" -ne 1 ] || [ "${links[0]}" != "$desired_link" ]; then
        echo "constructor-required: Raspberry image has more than one enabled early splash writer" >&2
        return 1
    fi
    if find "$image_root/etc/systemd/system" -name '*cellsymphony-boot-splash*' -print -quit | grep -q .; then
        echo "constructor-required: legacy early splash writer remains installed" >&2
        return 1
    fi
}

require_octessera_initramfs_boot_layer() {
    local boot_root="$1"
    local image_root="$2"
    local selected=()
    local initramfs
    local listing
    local contract_path="$REPOSITORY_ROOT/resources/image-construction/boot-layers/raspberry-pi-zero-2w.json"
    if [ ! -d "$boot_root/octessera" ]; then
        echo "constructor-required: Raspberry v1 boot output directory is missing" >&2
        return 1
    fi
    while IFS= read -r initramfs; do
        [ -n "$initramfs" ] && selected+=("$initramfs")
    done < <(find "$boot_root/octessera" -maxdepth 1 -type f -name 'initrd.img-*' -print | sort)
    if [ "${#selected[@]}" -ne 1 ]; then
        echo "constructor-required: selected Raspberry initramfs output is not exact" >&2
        return 1
    fi
    if ! command -v lsinitramfs >/dev/null 2>&1; then
        echo "constructor-required: lsinitramfs is required for the Raspberry v1 boot proof" >&2
        return 1
    fi
    listing="$(lsinitramfs "${selected[0]}")"
    if [ -z "$listing" ]; then
        echo "constructor-required: selected initramfs listing is empty" >&2
        return 1
    fi
    require_octessera_initramfs_rootfs_bindings "${selected[0]}" "$image_root" "$contract_path"
}

require_octessera_initramfs_rootfs_bindings() {
    local initramfs="$1"
    local image_root="$2"
    local contract_path="$3"
    python3 "$REPOSITORY_ROOT/tools/pi-image/rpi_initramfs_proof.py" \
        --validate-command-layout "$initramfs" \
        --contract "$contract_path" \
        --root "$image_root"
}

require_octessera_boot_layer() {
    local boot_root="$1"
    local image_root="$2"
    local service="$image_root/etc/systemd/system/octessera-boot-splash.service"
    export OCTESSERA_BOOT_LAYER_CLASSIFICATION=unknown
    if grep -qxF 'Type=simple' "$service" 2>/dev/null; then
        require_octessera_boot_service_layout "$image_root"
        require_octessera_initramfs_boot_layer "$boot_root" "$image_root"
        export OCTESSERA_BOOT_LAYER_CLASSIFICATION=constructor-required
        return
    fi
    if grep -qxF 'Type=oneshot' "$service" 2>/dev/null; then
        require_octessera_trusted_parent_boot_layout "$image_root"
        export OCTESSERA_BOOT_LAYER_CLASSIFICATION=trusted-parent-v0.7.5
        return
    fi
    echo "boot-layer: Raspberry service is neither current constructor output nor the v0.7.5 trusted parent" >&2
    return 1
}

require_octessera_legal_notices() {
    local image_root="$1"
    local stager="$REPOSITORY_ROOT/tools/legal/stage_notices.py"
    if [ ! -f "$stager" ] || [ -L "$stager" ]; then
        echo "constructor-required: legal notice stager is missing" >&2
        return 1
    fi
    python3 "$stager" --repository-root "$REPOSITORY_ROOT" --destination-root "$image_root" --check-finalized >/dev/null || {
        echo "constructor-required: installed legal notice tree is not canonical" >&2
        return 1
    }
    for sentinel in \
        "$image_root/usr/share/common-licenses/GPL-3" \
        "$image_root/usr/share/doc/base-files/copyright"; do
        if [ ! -f "$sentinel" ] || [ -L "$sentinel" ] || [ ! -s "$sentinel" ]; then
            echo "constructor-required: preserved parent legal sentinel is missing or empty: $sentinel" >&2
            return 1
        fi
    done
}

require_octessera_raspberry_identity() {
    local boot_root="$1"
    local image_root="$2"
    local welcome_source="$REPOSITORY_ROOT/tools/pi-image/stage4-octessera/files/root/etc/profile.d/octessera-welcome.sh"
    local welcome="$image_root/etc/profile.d/octessera-welcome.sh"
    local default_source="$REPOSITORY_ROOT/config/generated/pi/default.json"
    local default_config="$image_root/home/pi/presets/default.json"
    local validator_source="$REPOSITORY_ROOT/tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py"
    local validator="$image_root/usr/local/lib/octessera/device_config.py"
    local validator_source_hash
    local validator_hash
    local boot_config="$boot_root/config.txt"
    local boot_cmdline="$boot_root/cmdline.txt"
    local token
    local pi_record
    local pi_user
    local pi_uid
    local pi_gid
    local pi_home
    local pi_shell
    local hushlogin
    local mask
    local enablement
    local tokens=()
    require_octessera_legal_notices "$image_root"

    if [ ! -f "$welcome" ] || [ -L "$welcome" ] || [ "$(stat -c '%u:%g:%a' "$welcome")" != 0:0:644 ] || ! cmp -s "$welcome_source" "$welcome"; then
        echo "constructor-required: Raspberry welcome file is not exact" >&2
        return 1
    fi
    pi_record="$(awk -F: '$1 == "pi" { print; count++ } END { if (count != 1) exit 1 }' "$image_root/etc/passwd")" || {
        echo "constructor-required: Raspberry pi passwd entry is not exact" >&2
        return 1
    }
    IFS=: read -r pi_user _ pi_uid pi_gid _ pi_home pi_shell <<< "$pi_record"
    if [ "$pi_user" != pi ] || [ "$pi_home" != /home/pi ] || [ "$pi_shell" != /bin/bash ] || [ ! -d "$image_root$pi_home" ] || [ -L "$image_root$pi_home" ]; then
        echo "constructor-required: Raspberry pi home or shell is not exact" >&2
        return 1
    fi
    if [ ! -f "$default_config" ] || [ -L "$default_config" ] || [ "$(stat -c '%u:%g:%a' "$default_config")" != "$pi_uid:$pi_gid:644" ] || ! cmp -s "$default_source" "$default_config"; then
        echo "constructor-required: Raspberry default config is not exact" >&2
        return 1
    fi
    python3 "$REPOSITORY_ROOT/tools/pi-image/verify-rpi-samples.py" --root "$image_root" --repository-root "$REPOSITORY_ROOT" || return 1
    if [ ! -f "$validator_source" ] || [ -L "$validator_source" ] || [ ! -f "$validator" ] || [ -L "$validator" ] || [ "$(stat -c '%u:%g:%a' "$validator")" != 0:0:644 ]; then
        echo "constructor-required: Raspberry device config validator metadata is not exact" >&2
        return 1
    fi
    validator_source_hash="$(sha256sum "$validator_source" | awk '{print $1}')"
    validator_hash="$(sha256sum "$validator" | awk '{print $1}')"
    if [ "$(stat -c '%s' "$validator")" != "$(stat -c '%s' "$validator_source")" ] || [ "$validator_hash" != "$validator_source_hash" ]; then
        echo "constructor-required: Raspberry device config validator bytes are not canonical" >&2
        return 1
    fi
    if ! awk -F: -v gid="$pi_gid" '$1 == "pi" && $3 == gid { count++ } END { exit count != 1 }' "$image_root/etc/group"; then
        echo "constructor-required: Raspberry pi group is not exact" >&2
        return 1
    fi
    hushlogin="$image_root$pi_home/.hushlogin"
    if [ ! -f "$hushlogin" ] || [ -L "$hushlogin" ] || [ "$(stat -c '%u:%g:%a:%s' "$hushlogin")" != "$pi_uid:$pi_gid:644:0" ] || [ -s "$hushlogin" ]; then
        echo "constructor-required: Raspberry .hushlogin is not exact" >&2
        return 1
    fi
    for directory in "$image_root/etc/pam.d" "$image_root/etc/update-motd.d"; do
        if [ -d "$directory" ] && find -P "$directory" -type f -iname '*octessera*' -print -quit | grep -q .; then
            echo "constructor-required: Raspberry repository PAM or update-motd override remains" >&2
            return 1
        fi
    done
    require_octessera_raspberry_fat_pair "$boot_config" "$boot_cmdline" || return 1
    require_octessera_bookworm_redirect_pair "$image_root" || return 1
    require_octessera_raspberry_firmware_pair "$boot_config" "$boot_cmdline" "$image_root" || return 1
    if grep -qP '\x00' "$boot_config" || grep -qP '\r(?!\n)' "$boot_config"; then
        echo "constructor-required: Raspberry config is malformed" >&2
        return 1
    fi
    if grep -qF '# --- Octessera UART release ---' "$boot_config"; then
        echo "constructor-required: obsolete Raspberry UART release marker remains" >&2
        return 1
    fi
    if grep -Eq '^[[:space:]]*enable_uart[[:space:]]*=[[:space:]]*1([[:space:]]|$)' "$boot_config"; then
        echo "constructor-required: Raspberry UART enablement remains" >&2
        return 1
    fi
    [ "$(grep -Ec '^[[:space:]]*dtoverlay=disable-bt([[:space:]]|$)' "$boot_config")" -eq 1 ] || { echo "constructor-required: Raspberry Bluetooth disable overlay is missing or duplicated" >&2; return 1; }
    [ "$(grep -Ec '^[[:space:]]*enable_uart=0([[:space:]]|$)' "$boot_config")" -eq 1 ] || { echo "constructor-required: Raspberry UART disable setting is missing or duplicated" >&2; return 1; }
    if grep -Eq '^[[:space:]]*enable_uart=1([[:space:]]|$)' "$boot_config"; then
        echo "constructor-required: Raspberry UART is enabled" >&2
        return 1
    fi
    if grep -qP '\x00' "$boot_cmdline" || [ "$(grep -c '' "$boot_cmdline")" -gt 1 ]; then
        echo "constructor-required: Raspberry cmdline is multiline or contains NUL" >&2
        return 1
    fi
    read -r -a tokens < "$boot_cmdline"
    for token in "${tokens[@]}"; do
        if [[ "$token" =~ ^console=(serial0|ttyAMA0|ttyS0)(,[^[:space:]]+)?$ ]]; then
            echo "constructor-required: forbidden serial console remains: $token" >&2
            return 1
        fi
    done
    for unit in serial0 ttyAMA0 ttyS0; do
        mask="$image_root/etc/systemd/system/serial-getty@$unit.service"
        if [ ! -L "$mask" ] || [ "$(readlink "$mask")" != /dev/null ]; then
            echo "constructor-required: serial-getty mask is not /dev/null: $unit" >&2
            return 1
        fi
        enablement="$image_root/etc/systemd/system/getty.target.wants/serial-getty@$unit.service"
        if [ -e "$enablement" ] || [ -L "$enablement" ]; then
            echo "constructor-required: serial-getty enablement remains: $unit" >&2
            return 1
        fi
    done
    for unit in hciuart bluetooth; do
        enablement="$image_root/etc/systemd/system/multi-user.target.wants/$unit.service"
        if [ -e "$enablement" ] || [ -L "$enablement" ]; then
            echo "constructor-required: Bluetooth service remains enabled: $unit" >&2
            return 1
        fi
    done
    if [ -e "$image_root/usr/local/lib/octessera/rpi_uart_release.py" ] || [ -L "$image_root/usr/local/lib/octessera/rpi_uart_release.py" ]; then
        echo "constructor-required: removed Raspberry UART release utility remains" >&2
        return 1
    fi
}

require_octessera_raspberry_identity_for_boot_layer() {
    case "${OCTESSERA_BOOT_LAYER_CLASSIFICATION:-unknown}" in
        constructor-required)
            require_octessera_raspberry_identity "$@"
            ;;
        trusted-parent-v0.7.5)
            require_octessera_trusted_parent_raspberry_identity "$@"
            ;;
        *)
            echo "Raspberry identity: boot layer classification is unknown" >&2
            return 1
            ;;
    esac
}

require_octessera_raspberry_fat_pair() {
    local boot_config="$1"
    local boot_cmdline="$2"
    for path in "$boot_config" "$boot_cmdline"; do
        if [ ! -f "$path" ] || [ -L "$path" ]; then
            echo "constructor-required: Raspberry FAT config pair is missing or symlinked" >&2
            return 1
        fi
    done
}

require_octessera_bookworm_redirect_notice() {
    local path="$1"
    local expected="$2"
    local label="$3"
    if [ ! -f "$path" ] || [ -L "$path" ] || [ "$(stat -c '%u:%g:%a' "$path")" != 0:0:644 ]; then
        echo "constructor-required: $label is not an exact root:root 0644 regular file" >&2
        return 1
    fi
    if ! printf '%s' "$expected" | cmp -s "$path" -; then
        echo "constructor-required: $label bytes are not exact" >&2
        return 1
    fi
}

require_octessera_bookworm_redirect_pair() {
    local image_root="$1"
    local config_notice="$image_root/boot/config.txt"
    local cmdline_notice="$image_root/boot/cmdline.txt"
    require_octessera_bookworm_redirect_notice \
        "$config_notice" \
        $'DO NOT EDIT THIS FILE\n\nThe file you are looking for has moved to /boot/firmware/config.txt\n' \
        "rootfs Raspberry config redirect notice" || return 1
    require_octessera_bookworm_redirect_notice \
        "$cmdline_notice" \
        $'DO NOT EDIT THIS FILE\n\nThe file you are looking for has moved to /boot/firmware/cmdline.txt\n' \
        "rootfs Raspberry cmdline redirect notice" || return 1
}

require_octessera_raspberry_firmware_pair() {
    local boot_config="$1"
    local boot_cmdline="$2"
    local image_root="$3"
    local firmware_config="$image_root/boot/firmware/config.txt"
    local firmware_cmdline="$image_root/boot/firmware/cmdline.txt"
    local config_present=false
    local cmdline_present=false
    if [ -e "$firmware_config" ] || [ -L "$firmware_config" ]; then
        config_present=true
    fi
    if [ -e "$firmware_cmdline" ] || [ -L "$firmware_cmdline" ]; then
        cmdline_present=true
    fi
    if [ "$config_present" = false ] && [ "$cmdline_present" = false ]; then
        return
    fi
    if [ "$config_present" = false ] || [ "$cmdline_present" = false ] ||
        [ ! "$firmware_config" -ef "$boot_config" ] || [ ! "$firmware_cmdline" -ef "$boot_cmdline" ]; then
        echo "constructor-required: Raspberry raw firmware pair is not absent or the same FAT objects" >&2
        return 1
    fi
}

require_octessera_boot_config() {
    local boot_root="$1"
    local config="$boot_root/config.txt"
    local marker_count
    marker_count="$(grep -cFx '# --- octessera additions ---' "$config" 2>/dev/null || true)"
    if [ -f "$config" ] && [ ! -L "$config" ] && [ "${marker_count:-0}" -eq 1 ]; then
        return
    fi
    echo "Sanitation check failed: missing exact octessera FAT boot config marker" >&2
    return 1
}

require_octessera_boot_overlay() {
    local boot_root="$1"
    local image_root="$2"
    local relative=overlays/i2s-dac-no20.dtbo
    local path
    for path in \
        "$boot_root/octessera/$relative" \
        "$image_root/boot/firmware/octessera/$relative" \
        "$boot_root/$relative" \
        "$image_root/boot/firmware/$relative"; do
        if [ -f "$path" ]; then
            return
        fi
    done
    echo "Sanitation check failed: missing i2s-dac-no20 boot overlay" >&2
    return 1
}

# shellcheck disable=SC1091
source "$REPOSITORY_ROOT/tools/pi-image/verify-trusted-parent-v0.7.5.sh"
