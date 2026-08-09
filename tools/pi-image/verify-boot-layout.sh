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
        'DeviceAllow=/dev/gpiomem rw'; do
        if ! grep -qxF "$required_line" "$service"; then
            echo "constructor-required: boot splash service is missing $required_line" >&2
            return 1
        fi
    done
    for exact_line in \
        'Type=simple' \
        'ExecStart=/usr/local/bin/octessera-pi --boot-splash-loop' \
        'Environment=OCTESSERA_OLED_BOOT_HANDOFF=v1' \
        'DevicePolicy=closed' \
        'DeviceAllow=/dev/spidev0.0 rw' \
        'DeviceAllow=/dev/gpiomem rw'; do
        count="$(grep -cFx "$exact_line" "$service" || true)"
        if [ "$count" -ne 1 ]; then
            echo "constructor-required: boot splash service has an extra or missing $exact_line" >&2
            return 1
        fi
    done
    for required_line in \
        'After=systemd-modules-load.service systemd-udevd.service systemd-udev-trigger.service' \
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
    for required_entry in \
        'scripts/init-premount/octessera-boot-splash' \
        'usr/local/bin/octessera-pi' \
        'usr/bin/setsid' \
        'bin/sh' \
        'bin/sleep' \
        'bin/cat' \
        'bin/mv' \
        'bin/chmod' \
        'bin/chown' \
        'bin/rm'; do
        if ! grep -qxF "$required_entry" <<< "$listing"; then
            echo "constructor-required: selected initramfs is missing $required_entry" >&2
            return 1
        fi
    done
    for required_entry in \
        scripts/init-premount/octessera-boot-splash \
        usr/local/bin/octessera-pi; do
        if [ "$(grep -cFx "$required_entry" <<< "$listing" || true)" -ne 1 ]; then
            echo "constructor-required: selected initramfs has a missing or duplicate $required_entry" >&2
            return 1
        fi
    done
    for required_module in spi-bcm2835 spidev; do
        if ! grep -qF "$required_module" <<< "$listing"; then
            echo "constructor-required: selected initramfs is missing module $required_module" >&2
            return 1
        fi
    done
    require_octessera_initramfs_rootfs_bindings "${selected[0]}" "$image_root"
}

require_octessera_initramfs_rootfs_bindings() {
    local initramfs="$1"
    local image_root="$2"
    local script_source="$image_root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash"
    local binary_link="$image_root/usr/local/bin/octessera-pi"
    local current_link="$image_root/opt/octessera/current"
    local current_target
    local release_binary
    local extraction
    local archive_path
    local source_path
    local extracted_path
    local expected_hash
    local actual_hash

    if [ ! -f "$script_source" ] || [ -L "$script_source" ]; then
        echo "constructor-required: initramfs splash script source is not a regular rootfs file" >&2
        return 1
    fi
    if [ ! -L "$binary_link" ] || [ "$(readlink "$binary_link")" != /opt/octessera/current/octessera-pi ]; then
        echo "constructor-required: rootfs runtime binary link is not the managed current link" >&2
        return 1
    fi
    if [ ! -L "$current_link" ]; then
        echo "constructor-required: rootfs current runtime link is missing" >&2
        return 1
    fi
    current_target="$(readlink "$current_link")"
    if ! printf '%s\n' "$current_target" | grep -Eq '^/opt/octessera/releases/[0-9]+\.[0-9]+\.[0-9]+$'; then
        echo "constructor-required: rootfs current runtime link is not a shipped semver" >&2
        return 1
    fi
    release_binary="$image_root$current_target/octessera-pi"
    if [ ! -f "$release_binary" ] || [ -L "$release_binary" ]; then
        echo "constructor-required: current rootfs runtime binary is not a regular file" >&2
        return 1
    fi
    if ! command -v unmkinitramfs >/dev/null 2>&1; then
        echo "constructor-required: unmkinitramfs is required for initramfs rootfs binding proof" >&2
        return 1
    fi
    extraction="$(mktemp -d)"
    if ! unmkinitramfs "$initramfs" "$extraction" >/dev/null 2>&1; then
        rm -rf "$extraction"
        echo "constructor-required: selected initramfs extraction failed" >&2
        return 1
    fi
    for archive_path in \
        scripts/init-premount/octessera-boot-splash \
        usr/local/bin/octessera-pi; do
        if [ "$archive_path" = usr/local/bin/octessera-pi ]; then
            source_path="$release_binary"
        else
            source_path="$script_source"
        fi
        extracted_path="$extraction/$archive_path"
        if [ ! -f "$extracted_path" ] || [ -L "$extracted_path" ] || [ "$(stat -c '%h' "$extracted_path")" != 1 ]; then
            rm -rf "$extraction"
            echo "constructor-required: selected initramfs binding is not a unique regular file: $archive_path" >&2
            return 1
        fi
        if [ "$(stat -c '%s' "$extracted_path")" -gt 67108864 ]; then
            rm -rf "$extraction"
            echo "constructor-required: selected initramfs binding is oversized: $archive_path" >&2
            return 1
        fi
        expected_hash="$(sha256sum "$source_path" | cut -d' ' -f1)"
        actual_hash="$(sha256sum "$extracted_path" | cut -d' ' -f1)"
        if [ "$expected_hash" != "$actual_hash" ] || ! cmp -s "$source_path" "$extracted_path"; then
            rm -rf "$extraction"
            echo "constructor-required: selected initramfs binding differs from rootfs: $archive_path" >&2
            return 1
        fi
    done
    rm -rf "$extraction"
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
    python3 "$stager" --repository-root "$REPOSITORY_ROOT" --destination-root "$image_root" --check >/dev/null || {
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
    local boot_config="$boot_root/config.txt"
    local firmware_config="$image_root/boot/firmware/config.txt"
    local legacy_config="$image_root/boot/config.txt"
    local config
    local cmdline
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
    if [ -f "$boot_config" ] || [ -L "$boot_config" ]; then
        if [ -f "$firmware_config" ] || [ -L "$firmware_config" ] || [ -f "$legacy_config" ] || [ -L "$legacy_config" ]; then
            echo "constructor-required: Raspberry config layout is ambiguous" >&2
            return 1
        fi
        config="$boot_config"
    elif [ -f "$firmware_config" ] || [ -L "$firmware_config" ]; then
        if [ -f "$legacy_config" ] || [ -L "$legacy_config" ]; then
            echo "constructor-required: Raspberry config layout is ambiguous" >&2
            return 1
        fi
        config="$firmware_config"
    else
        config="$legacy_config"
    fi
    if [ ! -f "$config" ] || [ -L "$config" ]; then
        echo "constructor-required: Raspberry config is missing or symlinked" >&2
        return 1
    fi
    if grep -qP '\x00' "$config" || grep -qP '\r(?!\n)' "$config"; then
        echo "constructor-required: Raspberry config is malformed" >&2
        return 1
    fi
    if grep -qF '# --- Octessera UART release ---' "$config"; then
        echo "constructor-required: obsolete Raspberry UART release marker remains" >&2
        return 1
    fi
    if grep -Eq '^[[:space:]]*enable_uart[[:space:]]*=[[:space:]]*1([[:space:]]|$)' "$config"; then
        echo "constructor-required: Raspberry UART enablement remains" >&2
        return 1
    fi
    [ "$(grep -Ec '^[[:space:]]*dtoverlay=disable-bt([[:space:]]|$)' "$config")" -eq 1 ] || { echo "constructor-required: Raspberry Bluetooth disable overlay is missing or duplicated" >&2; return 1; }
    [ "$(grep -Ec '^[[:space:]]*enable_uart=0([[:space:]]|$)' "$config")" -eq 1 ] || { echo "constructor-required: Raspberry UART disable setting is missing or duplicated" >&2; return 1; }
    if grep -Eq '^[[:space:]]*enable_uart=1([[:space:]]|$)' "$config"; then
        echo "constructor-required: Raspberry UART is enabled" >&2
        return 1
    fi
    cmdline="${config%config.txt}cmdline.txt"
    if [ ! -f "$cmdline" ] || [ -L "$cmdline" ]; then
        echo "constructor-required: Raspberry cmdline is missing or symlinked" >&2
        return 1
    fi
    if grep -qP '\x00' "$cmdline" || [ "$(grep -c '' "$cmdline")" -gt 1 ]; then
        echo "constructor-required: Raspberry cmdline is multiline or contains NUL" >&2
        return 1
    fi
    read -r -a tokens < "$cmdline"
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

require_octessera_boot_config() {
    local boot_root="$1"
    local image_root="$2"
    if grep -q 'octessera additions' "$boot_root/config.txt" 2>/dev/null ||
        grep -q 'octessera additions' "$image_root/boot/firmware/config.txt" 2>/dev/null ||
        grep -q 'octessera additions' "$image_root/boot/config.txt" 2>/dev/null; then
        return
    fi
    echo "Sanitation check failed: missing octessera boot config marker" >&2
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
