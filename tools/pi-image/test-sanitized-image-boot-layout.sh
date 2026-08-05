#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$script_dir/verify-boot-layout.sh"

fixture="$(mktemp -d)"
trap 'rm -rf "$fixture"' EXIT

reset_fixture() {
    rm -rf "$fixture"
    mkdir -p "$fixture/boot" "$fixture/root"
}

reset_fixture
mkdir -p "$fixture/boot/octessera/overlays"
printf '%s\n' '# octessera additions' > "$fixture/boot/config.txt"
printf '%s\n' 'dtbo' > "$fixture/boot/octessera/overlays/i2s-dac-no20.dtbo"
require_octessera_boot_config "$fixture/boot" "$fixture/root"
require_octessera_boot_overlay "$fixture/boot" "$fixture/root"

reset_fixture
mkdir -p "$fixture/root/boot/firmware/octessera/overlays"
printf '%s\n' '# octessera additions' > "$fixture/root/boot/firmware/config.txt"
printf '%s\n' 'dtbo' > "$fixture/root/boot/firmware/octessera/overlays/i2s-dac-no20.dtbo"
require_octessera_boot_config "$fixture/boot" "$fixture/root"
require_octessera_boot_overlay "$fixture/boot" "$fixture/root"

reset_fixture
if require_octessera_boot_config "$fixture/boot" "$fixture/root"; then
    echo 'Boot layout accepted a missing config marker.' >&2
    exit 1
fi
if require_octessera_boot_overlay "$fixture/boot" "$fixture/root"; then
    echo 'Boot layout accepted a missing overlay.' >&2
    exit 1
fi

if [ "$(id -u)" -eq 0 ]; then
    reset_fixture
    mkdir -p \
        "$fixture/root/etc/profile.d" \
        "$fixture/root/etc/systemd/system/getty.target.wants" \
        "$fixture/root/etc/systemd/system" \
        "$fixture/root/etc/systemd/system/multi-user.target.wants" \
        "$fixture/root/boot/firmware" \
        "$fixture/root/home/pi" \
        "$fixture/root/usr/local/lib/octessera"
    printf '%s\n' 'root:x:0:0:root:/root:/bin/bash' 'pi:x:1000:1000:Pi:/home/pi:/bin/bash' > "$fixture/root/etc/passwd"
    printf '%s\n' 'root:x:0:' 'pi:x:1000:' > "$fixture/root/etc/group"
    cp "$script_dir/stage4-octessera/files/root/etc/profile.d/octessera-welcome.sh" "$fixture/root/etc/profile.d/octessera-welcome.sh"
    cp "$script_dir/stage4-octessera/files/root/usr/local/lib/octessera/rpi_uart_release.py" "$fixture/root/usr/local/lib/octessera/rpi_uart_release.py"
    chmod 0644 "$fixture/root/etc/profile.d/octessera-welcome.sh"
    chmod 0755 "$fixture/root/usr/local/lib/octessera/rpi_uart_release.py"
    : > "$fixture/root/home/pi/.hushlogin"
    chown 1000:1000 "$fixture/root/home/pi/.hushlogin"
    printf '%s\n' '# --- Octessera UART release ---' '[all]' 'dtoverlay=disable-bt' 'enable_uart=0' > "$fixture/root/boot/firmware/config.txt"
    printf '%s\n' 'console=tty1 root=/dev/mmcblk0p2' > "$fixture/root/boot/firmware/cmdline.txt"
    for unit in serial0 ttyAMA0 ttyS0; do
        ln -s /dev/null "$fixture/root/etc/systemd/system/serial-getty@$unit.service"
    done
    python3 "$script_dir/../legal/stage_notices.py" \
        --repository-root "$script_dir/../.." \
        --destination-root "$fixture/root" >/dev/null
    mkdir -p "$fixture/root/usr/share/common-licenses" "$fixture/root/usr/share/doc/base-files"
    printf '%s\n' 'fixture GPL license' > "$fixture/root/usr/share/common-licenses/GPL-3"
    printf '%s\n' 'fixture base-files copyright' > "$fixture/root/usr/share/doc/base-files/copyright"
    require_octessera_raspberry_identity "$fixture/boot" "$fixture/root"
    for token in console=serial01 console=ttyAMA0-debug console=ttyS0foo; do
        printf '%s\n' "console=tty1 $token root=/dev/mmcblk0p2" > "$fixture/root/boot/firmware/cmdline.txt"
        require_octessera_raspberry_identity "$fixture/boot" "$fixture/root"
    done
    printf '%s\n' 'console=tty1 console=serial0,115200 root=/dev/mmcblk0p2' > "$fixture/root/boot/firmware/cmdline.txt"
    if require_octessera_raspberry_identity "$fixture/boot" "$fixture/root"; then
        echo 'Boot layout accepted an exact serial console token.' >&2
        exit 1
    fi
    printf '%s\n' 'console=tty1 root=/dev/mmcblk0p2' > "$fixture/root/boot/firmware/cmdline.txt"
    OCTESSERA_BOOT_LAYER_CLASSIFICATION=constructor-required
    require_octessera_raspberry_identity_for_boot_layer "$fixture/boot" "$fixture/root"

    create_trusted_parent_fixture() {
        reset_fixture
        mkdir -p \
            "$fixture/boot/octessera/overlays" \
            "$fixture/root/etc/profile.d" \
            "$fixture/root/etc/systemd/system/sysinit.target.wants" \
            "$fixture/root/etc/systemd/system/getty.target.wants" \
            "$fixture/root/etc/systemd/system/multi-user.target.wants" \
            "$fixture/root/home/pi" \
            "$fixture/root/usr/local/lib/octessera"
        printf '%s\n' '# --- octessera additions ---' 'dtoverlay=disable-bt' 'enable_uart=0' > "$fixture/boot/config.txt"
        printf '%s\n' 'dtbo' > "$fixture/boot/octessera/overlays/i2s-dac-no20.dtbo"
        printf '%s\n' 'console=tty1 console=serial0,115200 root=/dev/mmcblk0p2' > "$fixture/boot/cmdline.txt"
        cat > "$fixture/root/etc/systemd/system/octessera-boot-splash.service" <<'EOF'
[Unit]
Description=legacy boot splash
After=systemd-modules-load.service systemd-udevd.service
Before=sysinit.target octessera.service

[Service]
Type=oneshot
ExecStart=-/usr/local/bin/octessera-pi --boot-splash-once
TimeoutStartSec=2

[Install]
WantedBy=sysinit.target
EOF
        cat > "$fixture/root/etc/systemd/system/octessera.service" <<'EOF'
[Unit]
Description=legacy Octessera runtime

[Service]
Type=simple
ExecStart=/usr/local/bin/octessera-pi
EOF
        chmod 0644 \
            "$fixture/root/etc/systemd/system/octessera-boot-splash.service" \
            "$fixture/root/etc/systemd/system/octessera.service"
        ln -s ../octessera-boot-splash.service "$fixture/root/etc/systemd/system/sysinit.target.wants/octessera-boot-splash.service"
        printf '%s\n' 'root:x:0:0:root:/root:/bin/bash' 'pi:x:1000:1000:Pi:/home/pi:/bin/bash' > "$fixture/root/etc/passwd"
        printf '%s\n' 'root:x:0:' 'pi:x:1000:' > "$fixture/root/etc/group"
        printf '%s\n' 'legacy parent welcome' > "$fixture/root/etc/profile.d/octessera-welcome.sh"
        chmod 0644 "$fixture/root/etc/profile.d/octessera-welcome.sh"
        python3 "$script_dir/../legal/stage_notices.py" \
            --repository-root "$script_dir/../.." \
            --destination-root "$fixture/root" >/dev/null
        mkdir -p "$fixture/root/usr/share/common-licenses" "$fixture/root/usr/share/doc/base-files"
        printf '%s\n' 'fixture GPL license' > "$fixture/root/usr/share/common-licenses/GPL-3"
        printf '%s\n' 'fixture base-files copyright' > "$fixture/root/usr/share/doc/base-files/copyright"
    }

    require_trusted_parent_fixture() {
        require_octessera_boot_config "$fixture/boot" "$fixture/root"
        require_octessera_boot_overlay "$fixture/boot" "$fixture/root"
        require_octessera_boot_layer "$fixture/boot" "$fixture/root"
        [ "$OCTESSERA_BOOT_LAYER_CLASSIFICATION" = trusted-parent-v0.7.5 ]
        require_octessera_raspberry_identity_for_boot_layer "$fixture/boot" "$fixture/root"
    }

    expect_trusted_parent_rejected() {
        local name="$1"
        require_octessera_boot_layer "$fixture/boot" "$fixture/root"
        if require_octessera_raspberry_identity_for_boot_layer "$fixture/boot" "$fixture/root"; then
            echo "Trusted parent fixture accepted tamper: $name" >&2
            exit 1
        fi
    }

    create_trusted_parent_fixture
    require_trusted_parent_fixture

    create_trusted_parent_fixture
    : > "$fixture/root/etc/profile.d/octessera-welcome.sh"
    expect_trusted_parent_rejected empty-legacy-welcome

    create_trusted_parent_fixture
    : > "$fixture/root/home/pi/.hushlogin"
    expect_trusted_parent_rejected mixed-current-hushlogin

    create_trusted_parent_fixture
    printf '%s\n' 'current UART release utility' > "$fixture/root/usr/local/lib/octessera/rpi_uart_release.py"
    expect_trusted_parent_rejected mixed-current-uart-utility

    create_trusted_parent_fixture
    printf '%s\n' '# --- Octessera UART release ---' >> "$fixture/boot/config.txt"
    expect_trusted_parent_rejected mixed-current-uart-marker

    create_trusted_parent_fixture
    printf '%s\n' 'console=tty1 root=/dev/mmcblk0p2' > "$fixture/boot/cmdline.txt"
    expect_trusted_parent_rejected missing-legacy-serial-console

    create_trusted_parent_fixture
    ln -s /dev/null "$fixture/root/etc/systemd/system/serial-getty@serial0.service"
    expect_trusted_parent_rejected mixed-current-serial-mask

    create_trusted_parent_fixture
    ln -s ../hciuart.service "$fixture/root/etc/systemd/system/multi-user.target.wants/hciuart.service"
    expect_trusted_parent_rejected mixed-current-bluetooth-enablement
fi

reset_fixture
mkdir -p \
    "$fixture/root/etc/initramfs-tools/scripts/init-premount" \
    "$fixture/root/opt/octessera/releases/1.2.3" \
    "$fixture/root/usr/local/bin" \
    "$fixture/archive/scripts/init-premount" \
    "$fixture/archive/usr/local/bin"
cp "$script_dir/stage4-octessera/files/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash" \
    "$fixture/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash"
cp "$fixture/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash" \
    "$fixture/archive/scripts/init-premount/octessera-boot-splash"
printf '%s\n' 'constructor-runtime-binary' > "$fixture/root/opt/octessera/releases/1.2.3/octessera-pi"
cp "$fixture/root/opt/octessera/releases/1.2.3/octessera-pi" "$fixture/archive/usr/local/bin/octessera-pi"
ln -s /opt/octessera/releases/1.2.3 "$fixture/root/opt/octessera/current"
ln -s /opt/octessera/current/octessera-pi "$fixture/root/usr/local/bin/octessera-pi"
make_initramfs() {
    local output="$fixture/boot/octessera-bound.img"
    mkdir -p "$fixture/boot"
    (cd "$fixture/archive" && find . -print | cpio -o -H newc --quiet | gzip -n > "$output")
}
make_initramfs
require_octessera_initramfs_rootfs_bindings "$fixture/boot/octessera-bound.img" "$fixture/root"
printf '%s\n' 'stale-script' > "$fixture/archive/scripts/init-premount/octessera-boot-splash"
make_initramfs
if require_octessera_initramfs_rootfs_bindings "$fixture/boot/octessera-bound.img" "$fixture/root"; then
    echo 'Boot layout accepted a stale initramfs script.' >&2
    exit 1
fi
cp "$fixture/root/etc/initramfs-tools/scripts/init-premount/octessera-boot-splash" "$fixture/archive/scripts/init-premount/octessera-boot-splash"
printf '%s\n' 'stale-binary' > "$fixture/archive/usr/local/bin/octessera-pi"
make_initramfs
if require_octessera_initramfs_rootfs_bindings "$fixture/boot/octessera-bound.img" "$fixture/root"; then
    echo 'Boot layout accepted a stale initramfs binary.' >&2
    exit 1
fi

printf '%s\n' 'Sanitized image boot layout tests passed'
