#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=tools/pi-image/test-sanitized-image-boot-layout-fixture.sh
source "$script_dir/test-sanitized-image-boot-layout-fixture.sh"

reset_fixture
mkdir -p "$fixture/boot/octessera/overlays"
printf '%s\n' '# --- octessera additions ---' > "$fixture/boot/config.txt"
printf '%s\n' 'dtbo' > "$fixture/boot/octessera/overlays/i2s-dac-no20.dtbo"
require_octessera_boot_config "$fixture/boot" "$fixture/root"
require_octessera_boot_overlay "$fixture/boot" "$fixture/root"

reset_fixture
mkdir -p "$fixture/root/boot/firmware/octessera/overlays"
printf '%s\n' '# --- octessera additions ---' > "$fixture/root/boot/firmware/config.txt"
printf '%s\n' 'dtbo' > "$fixture/root/boot/firmware/octessera/overlays/i2s-dac-no20.dtbo"
if require_octessera_boot_config "$fixture/boot" "$fixture/root"; then
    echo 'Boot layout accepted a hidden rootfs config marker.' >&2
    exit 1
fi
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
        "$fixture/root/boot" \
        "$fixture/root/home/pi" "$fixture/root/home/pi/presets" \
        "$fixture/root/usr/local/lib/octessera"
    printf '%s\n' 'root:x:0:0:root:/root:/bin/bash' 'pi:x:1000:1000:Pi:/home/pi:/bin/bash' > "$fixture/root/etc/passwd"
    printf '%s\n' 'root:x:0:' 'pi:x:1000:' > "$fixture/root/etc/group"
    cp "$script_dir/stage4-octessera/files/root/etc/profile.d/octessera-welcome.sh" "$fixture/root/etc/profile.d/octessera-welcome.sh"
    chmod 0644 "$fixture/root/etc/profile.d/octessera-welcome.sh"
    cp "$script_dir/../../config/generated/pi/default.json" "$fixture/root/home/pi/presets/default.json"
    chmod 0644 "$fixture/root/home/pi/presets/default.json"
    chown 1000:1000 "$fixture/root/home/pi/presets/default.json"
    cp "$script_dir/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py" "$fixture/root/usr/local/lib/octessera/device_config.py"; chmod 0644 "$fixture/root/usr/local/lib/octessera/device_config.py"; chown 0:0 "$fixture/root/usr/local/lib/octessera/device_config.py"
    : > "$fixture/root/home/pi/.hushlogin"
    chown 1000:1000 "$fixture/root/home/pi/.hushlogin"
    write_constructor_fat_pair
    write_constructor_redirect_pair
    bash "$script_dir/stage-musical-assets.sh" "$fixture/sample-stage"
    bash "$script_dir/install-musical-assets.sh" "$fixture/sample-stage" "$fixture/root"
    chown -R 1000:1000 "$fixture/root/home/pi/samples"
    for unit in serial0 ttyAMA0 ttyS0; do
        ln -s /dev/null "$fixture/root/etc/systemd/system/serial-getty@$unit.service"
    done
    test ! -e "$fixture/root/etc/systemd/system/multi-user.target.wants/bluetooth.service"
    test ! -e "$fixture/root/etc/systemd/system/multi-user.target.wants/hciuart.service"
    python3 "$script_dir/../legal/stage_notices.py" \
        --repository-root "$script_dir/../.." \
        --destination-root "$fixture/root" >/dev/null
    rm "$fixture/root/usr/share/doc/octessera/licenses/cargo/vendored-cpal-0.15.3/LICENSE"
    ln "$fixture/root/usr/share/doc/octessera/licenses/cargo/reference/Apache-2.0.txt" \
        "$fixture/root/usr/share/doc/octessera/licenses/cargo/vendored-cpal-0.15.3/LICENSE"
    rm "$fixture/root/usr/share/doc/octessera/licenses/pnpm/react/18.3.1/LICENSE" \
        "$fixture/root/usr/share/doc/octessera/licenses/pnpm/scheduler/0.23.2/LICENSE"
    ln "$fixture/root/usr/share/doc/octessera/licenses/pnpm/react-dom/18.3.1/LICENSE" \
        "$fixture/root/usr/share/doc/octessera/licenses/pnpm/react/18.3.1/LICENSE"
    ln "$fixture/root/usr/share/doc/octessera/licenses/pnpm/react-dom/18.3.1/LICENSE" \
        "$fixture/root/usr/share/doc/octessera/licenses/pnpm/scheduler/0.23.2/LICENSE"
    mkdir -p "$fixture/root/usr/share/common-licenses" "$fixture/root/usr/share/doc/base-files"
    printf '%s\n' 'fixture GPL license' > "$fixture/root/usr/share/common-licenses/GPL-3"
    printf '%s\n' 'fixture base-files copyright' > "$fixture/root/usr/share/doc/base-files/copyright"
    require_octessera_legal_notices "$fixture/root"
    ln "$fixture/root/usr/share/doc/octessera/licenses/cargo/reference/Apache-2.0.txt" \
        "$fixture/root/usr/share/doc/external-octessera-legal-alias"
    if require_octessera_legal_notices "$fixture/root"; then
        echo 'Finalized legal verification accepted an external hardlink alias.' >&2
        exit 1
    fi
    rm "$fixture/root/usr/share/doc/external-octessera-legal-alias"
    require_octessera_raspberry_identity "$fixture/boot" "$fixture/root"
    validator_path="$fixture/root/usr/local/lib/octessera/device_config.py"
    for validator_case in stale size; do
        cp "$script_dir/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py" "$validator_path"; chmod 0644 "$validator_path"; chown 0:0 "$validator_path"
        if [ "$validator_case" = stale ]; then python3 -c 'from pathlib import Path; import sys; p=Path(sys.argv[1]); b=bytearray(p.read_bytes()); b[0] ^= 1; p.write_bytes(b)' "$validator_path"; else truncate -s -1 "$validator_path"; fi
        if require_octessera_raspberry_identity "$fixture/boot" "$fixture/root"; then echo "Constructor identity accepted a $validator_case device config validator." >&2; exit 1; fi
    done
    cp "$script_dir/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py" "$validator_path"; chmod 0644 "$validator_path"; chown 0:0 "$validator_path"
    write_constructor_fat_pair
    printf '\r' >> "$fixture/boot/config.txt"
    if require_octessera_raspberry_identity "$fixture/boot" "$fixture/root"; then
        echo 'Boot layout accepted a lone carriage return in config.txt.' >&2
        exit 1
    fi
    write_constructor_fat_pair
    for token in console=serial01 console=ttyAMA0-debug console=ttyS0foo; do
        printf '%s\n' "console=tty1 $token root=/dev/mmcblk0p2" > "$fixture/boot/cmdline.txt"
        require_octessera_raspberry_identity "$fixture/boot" "$fixture/root"
    done
    printf '%s\n' 'console=tty1 console=serial0,115200 root=/dev/mmcblk0p2' > "$fixture/boot/cmdline.txt"
    if require_octessera_raspberry_identity "$fixture/boot" "$fixture/root"; then
        echo 'Boot layout accepted an exact serial console token.' >&2
        exit 1
    fi
    printf '%s\n' 'console=tty1 root=/dev/mmcblk0p2' > "$fixture/boot/cmdline.txt"
    export OCTESSERA_BOOT_LAYER_CLASSIFICATION=constructor-required
    require_octessera_raspberry_identity_for_boot_layer "$fixture/boot" "$fixture/root"

    mkdir -p "$fixture/root/boot/firmware"
    ln "$fixture/boot/config.txt" "$fixture/root/boot/firmware/config.txt"
    ln "$fixture/boot/cmdline.txt" "$fixture/root/boot/firmware/cmdline.txt"
    require_octessera_raspberry_identity "$fixture/boot" "$fixture/root"
    rm -rf "$fixture/root/boot/firmware"
    mkdir -p "$fixture/root/boot/firmware"
    cp "$fixture/boot/config.txt" "$fixture/root/boot/firmware/config.txt"
    cp "$fixture/boot/cmdline.txt" "$fixture/root/boot/firmware/cmdline.txt"
    expect_constructor_identity_failure 'Constructor identity accepted independent same-byte firmware duplicates.'
    rm -rf "$fixture/root/boot/firmware"
    mkdir -p "$fixture/root/boot/firmware"
    ln "$fixture/boot/config.txt" "$fixture/root/boot/firmware/config.txt"
    expect_constructor_identity_failure 'Constructor identity accepted a partial raw firmware pair.'
    rm -rf "$fixture/root/boot/firmware"
    mkdir -p "$fixture/root/boot/firmware"
    ln "$fixture/boot/config.txt" "$fixture/root/boot/firmware/config.txt"
    cp "$fixture/boot/cmdline.txt" "$fixture/root/boot/firmware/cmdline.txt"
    expect_constructor_identity_failure 'Constructor identity accepted mixed raw firmware objects.'
    rm -rf "$fixture/root/boot/firmware"

    for path in config.txt cmdline.txt; do
        rm "$fixture/root/boot/$path"
        expect_constructor_identity_failure "Constructor identity accepted a missing rootfs redirect notice: $path"
        write_constructor_redirect_pair
    done
    printf '%s\n\n%s\n' 'DO NOT EDIT THIS FILE' 'The file you are looking for has moved to /boot/firmware/config.txt altered' > "$fixture/root/boot/config.txt"
    expect_constructor_identity_failure 'Constructor identity accepted an altered rootfs config redirect notice.'
    write_constructor_redirect_pair
    chmod 0600 "$fixture/root/boot/config.txt"
    expect_constructor_identity_failure 'Constructor identity accepted a rootfs redirect notice with the wrong mode.'
    chmod 0644 "$fixture/root/boot/config.txt"
    rm "$fixture/root/boot/config.txt"
    mkdir "$fixture/root/boot/config.txt"
    expect_constructor_identity_failure 'Constructor identity accepted a non-regular rootfs config redirect notice.'
    rmdir "$fixture/root/boot/config.txt"
    write_constructor_redirect_pair
    rm "$fixture/root/boot/config.txt"
    ln -s "$fixture/boot/config.txt" "$fixture/root/boot/config.txt"
    expect_constructor_identity_failure 'Constructor identity accepted a symlinked rootfs config redirect notice.'
    rm "$fixture/root/boot/config.txt"
    write_constructor_redirect_pair

    rm "$fixture/boot/config.txt"
    mkdir -p "$fixture/root/boot/firmware"
    printf '%s\n' '# --- octessera additions ---' > "$fixture/root/boot/firmware/config.txt"
    expect_boot_config_failure 'Boot config accepted a hidden marker for a missing FAT config.'
    expect_constructor_identity_failure 'Constructor identity accepted a missing FAT config with a hidden marker.'
    rm -rf "$fixture/root/boot/firmware"
    write_constructor_fat_pair
    mkdir -p "$fixture/root/boot/firmware"
    cp "$fixture/boot/cmdline.txt" "$fixture/root/boot/firmware/cmdline.txt"
    rm "$fixture/boot/cmdline.txt"
    expect_constructor_identity_failure 'Constructor identity paired a FAT config with a raw firmware cmdline.'
    rm -rf "$fixture/root/boot/firmware"
    write_constructor_fat_pair
    rm "$fixture/boot/config.txt"
    mkdir "$fixture/boot/config.txt"
    expect_constructor_identity_failure 'Constructor identity accepted a non-regular FAT config.'
    rmdir "$fixture/boot/config.txt"
    write_constructor_fat_pair
    rm "$fixture/boot/config.txt"
    ln -s "$fixture/boot/cmdline.txt" "$fixture/boot/config.txt"
    expect_constructor_identity_failure 'Constructor identity accepted a symlinked FAT config.'
    rm "$fixture/boot/config.txt"
    write_constructor_fat_pair

    printf '%s\n' '# octessera hardware configuration' '[all]' 'dtoverlay=disable-bt' 'enable_uart=0' > "$fixture/boot/config.txt"
    mkdir -p "$fixture/root/boot/firmware"
    printf '%s\n' '# --- octessera additions ---' > "$fixture/root/boot/firmware/config.txt"
    expect_boot_config_failure 'Boot config accepted a hidden marker for a marker-less FAT config.'
    expect_constructor_identity_failure 'Constructor identity accepted a marker-less FAT config with hidden firmware content.'
    rm -rf "$fixture/root/boot/firmware"
    printf '%s\n' '# octessera hardware configuration' '[all]' 'dtoverlay=disable-bt' 'enable_uart=0' > "$fixture/boot/config.txt"
    expect_boot_config_failure 'Boot config accepted a marker-less FAT config.'
    write_constructor_fat_pair

    create_trusted_parent_fixture() {
        reset_fixture
        mkdir -p \
            "$fixture/boot/octessera/overlays" \
            "$fixture/root/etc/profile.d" \
            "$fixture/root/etc/systemd/system/sysinit.target.wants" \
            "$fixture/root/etc/systemd/system/getty.target.wants" \
            "$fixture/root/etc/systemd/system/multi-user.target.wants" \
            "$fixture/root/home/pi" "$fixture/root/home/pi/presets" \
            "$fixture/root/boot/firmware" \
            "$fixture/root/usr/local/lib/octessera"
        cp "$script_dir/fixtures/trusted-parent-v0.7.5/boot/config.txt" "$fixture/boot/config.txt"
        cp "$script_dir/fixtures/trusted-parent-v0.7.5/boot/cmdline.txt" "$fixture/boot/cmdline.txt"
        cp "$script_dir/fixtures/trusted-parent-v0.7.5/root/boot/config.txt" "$fixture/root/boot/config.txt"
        chmod 0755 "$fixture/boot/config.txt" "$fixture/boot/cmdline.txt"
        chmod 0644 "$fixture/root/boot/config.txt"
        printf '%s\n' 'dtbo' > "$fixture/boot/octessera/overlays/i2s-dac-no20.dtbo"
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
        chmod 0644 "$fixture/root/etc/systemd/system/octessera-boot-splash.service" "$fixture/root/etc/systemd/system/octessera.service"
        ln -s ../octessera-boot-splash.service "$fixture/root/etc/systemd/system/sysinit.target.wants/octessera-boot-splash.service"
        printf '%s\n' 'root:x:0:0:root:/root:/bin/bash' 'pi:x:1000:1000:Pi:/home/pi:/bin/bash' > "$fixture/root/etc/passwd"
        printf '%s\n' 'root:x:0:' 'pi:x:1000:' > "$fixture/root/etc/group"
        printf '%s\n' 'legacy parent welcome' > "$fixture/root/etc/profile.d/octessera-welcome.sh"
        chmod 0644 "$fixture/root/etc/profile.d/octessera-welcome.sh"
        python3 "$script_dir/../legal/stage_notices.py" --repository-root "$script_dir/../.." --destination-root "$fixture/root" >/dev/null
        mkdir -p "$fixture/root/usr/share/common-licenses" "$fixture/root/usr/share/doc/base-files"
        printf '%s\n' 'fixture GPL license' > "$fixture/root/usr/share/common-licenses/GPL-3"
        printf '%s\n' 'fixture base-files copyright' > "$fixture/root/usr/share/doc/base-files/copyright"
    }

    create_trusted_parent_fixture
    require_octessera_boot_config "$fixture/boot" "$fixture/root"
    require_octessera_boot_overlay "$fixture/boot" "$fixture/root"
    require_octessera_boot_layer "$fixture/boot" "$fixture/root"
    require_octessera_raspberry_identity_for_boot_layer "$fixture/boot" "$fixture/root"
fi
