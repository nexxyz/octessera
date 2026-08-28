from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import shutil
import struct
import subprocess
import sys
from pathlib import Path

from orange_boot_contract import verify_runtime
from stage_notices import stage_notices  # type: ignore[import-not-found]

TOOLS = Path(__file__).resolve().parent
REPOSITORY = TOOLS.parents[1]
CONSTRUCTION = json.loads((REPOSITORY / "resources/image-construction/boot-layers/orange-pi-zero-2w.json").read_text())
VERIFY_SPEC = importlib.util.spec_from_file_location("orange_image_verifier", TOOLS / "verify-orange-image.py")
assert VERIFY_SPEC is not None and VERIFY_SPEC.loader is not None
VERIFY = importlib.util.module_from_spec(VERIFY_SPEC)
VERIFY_SPEC.loader.exec_module(VERIFY)

RELEASE = "6.18.46-current-sunxi64"
REVISION = "26.11.0-trunk.22"
IMAGE_NAME = "linux-image-current-sunxi64"
DTB_NAME = "linux-dtb-current-sunxi64"
CANONICAL_IMAGE = f"{IMAGE_NAME}_{REVISION}_arm64.deb"
CANONICAL_DTB = f"{DTB_NAME}_{REVISION}_arm64.deb"
ARTIFACT_SUFFIX = "6.18.46-S1f99-D7115-P6bf8-C4e0c-H5530-HK01ba-Vc222-Bb84f-R448a"
NATIVE_IMAGE = f"{IMAGE_NAME}_{REVISION}_arm64__{ARTIFACT_SUFFIX}.deb"
NATIVE_DTB = f"{DTB_NAME}_{REVISION}_arm64__{ARTIFACT_SUFFIX}.deb"
DTB_RELATIVE = f"usr/lib/linux-image-{RELEASE}/allwinner/sun50i-h618-orangepi-zero2w.dtb"
MODULE_RELATIVE = f"lib/modules/{RELEASE}/kernel/drivers/usb/gadget/function/usb_f_midi.ko"
BUILTIN_CONFIG_LINES = ("CONFIG_SPI_SUN6I=y", "CONFIG_SPI_SPIDEV=y", "CONFIG_PINCTRL_SUNXI=y", "CONFIG_MMC=y", "CONFIG_MMC_BLOCK=y", "CONFIG_SOUND=y", "CONFIG_SND=y", "CONFIG_SND_SOC=y", "CONFIG_REGMAP_MMIO=y", "CONFIG_SND_SOC_GENERIC_DMAENGINE_PCM=y", "CONFIG_SND_SOC_SUNXI_AHUB=y", "CONFIG_SND_SOC_SUNXI_AHUB_DAM=y", "CONFIG_SND_SOC_SUNXI_MACH=y", "CONFIG_NVMEM_SUNXI_SID=y")
RESIZE_SERVICE_RELATIVE = "usr/lib/systemd/system/armbian-resize-filesystem.service"
RESIZE_ENABLE_RELATIVE = "etc/systemd/system/basic.target.wants/armbian-resize-filesystem.service"
FIRSTRUN_SERVICE_RELATIVE = "lib/systemd/system/armbian-firstrun.service"
FIRSTRUN_EXECUTABLE_RELATIVE = "usr/lib/armbian/armbian-firstrun"
FIRSTRUN_ENABLE_RELATIVE = "etc/systemd/system/multi-user.target.wants/armbian-firstrun.service"
FIRSTRUN_DEFAULTS_RELATIVE = "etc/default/armbian-firstrun"
SSH_MASKED_UNITS = ("ssh.service", "ssh.socket", "sshd.service", "sshd.socket")
FIRSTRUN_SERVICE = """# Armbian firstrun service
[Unit]
Description=Armbian first run tasks
Before=getty.target system-getty.slice
After=ssh.service

[Service]
Type=simple
RemainAfterExit=yes
EnvironmentFile=/etc/default/armbian-firstrun
ExecStart=/usr/lib/armbian/armbian-firstrun start
TimeoutStartSec=2min

[Install]
WantedBy=multi-user.target
"""
FIRSTRUN_EXECUTABLE = """#!/bin/bash
if [[ "${OPENSSHD_REGENERATE_HOST_KEYS}" = true ]]; then
    rm -f /etc/ssh/ssh_host*
    dpkg-reconfigure openssh-server >/dev/null 2>&1
    service ssh restart
else
    echo "SSH host keys unchanged"
fi
"""
RESIZE_SERVICE = """# Armbian resize filesystem service
# Resizes partition and filesystem on first/second boot
# This service may block the boot process for up to 3 minutes

[Unit]
Description=Armbian filesystem resize
Before=basic.target
After=sysinit.target local-fs.target
DefaultDependencies=no

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/lib/armbian/armbian-resize-filesystem start
TimeoutStartSec=6min

[Install]
WantedBy=basic.target
"""


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path: Path, content: bytes | str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content if isinstance(content, bytes) else content.encode())


def copy_fixture_root(source: Path, destination: Path) -> None:
    shutil.copytree(source, destination, symlinks=True)
    os.chown(destination / "home/octessera/.hushlogin", 1000, 1000)  # type: ignore[attr-defined]
    os.chown(destination / "var/lib/octessera/samples", 990, 990)  # type: ignore[attr-defined]


def make_uboot_initramfs(payload: bytes) -> bytes:
    header = bytearray(64)
    struct.pack_into(">I", header, 0, 0x27051956)
    struct.pack_into(">I", header, 12, len(payload))
    return bytes(header) + payload


def make_cpio_initramfs(work: Path, source_root: Path) -> bytes:
    source = work / "initramfs-source"
    write(source / "init", b"#!/bin/sh\n")
    requirements = CONSTRUCTION["selected_initramfs"]
    for item in requirements["installed_output_matches"]:
        target = source / item["initramfs_path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source_root / item["installed_path"], target)
    for tool in requirements["required_tools"]:
        write(source / tool, b"synthetic-tool\n")
    write(source / "usr/bin/python3", b"synthetic-python\n")
    for relative in requirements["python_files"]:
        write(source / f"usr/lib/python3.13/{relative}", b"synthetic-python-closure\n")
    for module in requirements["required_python_modules"]:
        extension = next(source_root.glob(f"usr/lib/python3.13/lib-dynload/{module}*.so"))
        target = source / extension.relative_to(source_root)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(extension, target)
    return subprocess.run(
        ["cpio", "--quiet", "-o", "-H", "newc"],
        cwd=source,
        input=subprocess.run(["find", ".", "-print"], cwd=source, capture_output=True, check=True).stdout,
        capture_output=True,
        check=True,
    ).stdout


def make_fixture(work: Path) -> tuple[Path, Path, Path, Path, Path]:
    image_root = work / "image-package"
    dtb_root = work / "dtb-package"
    image_root.mkdir()
    dtb_root.mkdir()
    write(
        image_root / "DEBIAN/control",
        f"Package: {IMAGE_NAME}\nVersion: {REVISION}\nSource: linux-6.18.46\nArchitecture: arm64\n"
        f"Armbian-Kernel-Version: 6.18.46\nArmbian-Kernel-Version-Family: {RELEASE}\n",
    )
    write(dtb_root / "DEBIAN/control", f"Package: {DTB_NAME}\nVersion: {REVISION}\nArchitecture: arm64\n")
    kernel = b"synthetic-orange-kernel-" + RELEASE.encode()
    config = b"# CONFIG_RT_GROUP_SCHED is not set\n" + b"\n".join(line.encode() for line in BUILTIN_CONFIG_LINES) + b"\nCONFIG_MMC_SPI=m\nCONFIG_SND_SEQUENCER=m\n"
    dtb = b"\xd0\x0d\xfe\xedsynthetic-zero2w-dtb"
    base_dts = REPOSITORY / "tools/armbian-image/fixtures/h618-orange-ahub-base.dts"
    stock_dts = REPOSITORY / "tools/armbian-image/fixtures/h618-stock-i2c1-pi.dts"
    base_dtb = work / "base.dtb"
    stock_dtbo = work / "stock-i2c1-pi.dtbo"
    subprocess.run(["dtc", "-@", "-I", "dts", "-O", "dtb", "-o", str(base_dtb), str(base_dts)], check=True, capture_output=True)
    subprocess.run(["dtc", "-@", "-I", "dts", "-O", "dtb", "-o", str(stock_dtbo), str(stock_dts)], check=True, capture_output=True)
    overlay_sources = {
        "spi": REPOSITORY / "userpatches/overlay/usr/local/share/octessera/device-tree/octessera-h618-spi1-oled-sd2.dts",
        "input": REPOSITORY / "userpatches/overlay/usr/local/share/octessera/device-tree/octessera-h618-input-routing.dts",
        "audio": REPOSITORY / "userpatches/overlay/usr/local/share/octessera/device-tree/octessera-ahub0-pcm5102.dts",
    }
    overlay_dtb = {}
    for name, source in overlay_sources.items():
        output = work / f"{name}.dtbo"
        subprocess.run(["dtc", "-@", "-I", "dts", "-O", "dtb", "-o", str(output), str(source)], check=True, capture_output=True)
        overlay_dtb[name] = output
    subprocess.run(["fdtoverlay", "-i", str(base_dtb), "-o", str(work / "stock-merged.dtb"), str(stock_dtbo)], check=True, capture_output=True)
    subprocess.run(["fdtoverlay", "-i", str(work / "stock-merged.dtb"), "-o", str(work / "spi-merged.dtb"), str(overlay_dtb["spi"])], check=True, capture_output=True)
    subprocess.run(["fdtoverlay", "-i", str(work / "spi-merged.dtb"), "-o", str(work / "spi-input-merged.dtb"), str(overlay_dtb["input"])], check=True, capture_output=True)
    subprocess.run(["fdtoverlay", "-i", str(work / "spi-input-merged.dtb"), "-o", str(work / "merged.dtb"), str(overlay_dtb["audio"])], check=True, capture_output=True)
    dtb = base_dtb.read_bytes()
    module = b"\x7fELF" + b"\x02\x01\x01" + bytes(11) + struct.pack("<H", 183) + b"vermagic=" + RELEASE.encode() + b" SMP\ninterface_string\ninterface_string\nf_midi_opts_attr_interface_string\nmidi_interface_string\n"
    write(image_root / f"usr/lib/linux-image-{RELEASE}/Image", kernel)
    write(image_root / f"boot/config-{RELEASE}", config)
    write(image_root / DTB_RELATIVE, dtb)
    write(image_root / MODULE_RELATIVE, module)
    write(image_root / f"lib/modules/{RELEASE}/modules.dep", b"kernel/drivers/usb/gadget/function/usb_f_midi.ko:\n")
    for module_name in ("snd-seq.ko", "snd-seq-midi.ko", "snd-rawmidi.ko", "snd-usb-audio.ko"):
        write(image_root / f"lib/modules/{RELEASE}/kernel/sound/{module_name}", b"synthetic-module")
    write(image_root / f"lib/modules/{RELEASE}/kernel/drivers/mmc/host/mmc_spi.ko", b"synthetic-module")
    write(dtb_root / f"boot/dtb-{RELEASE}/allwinner/sun50i-h618-orangepi-zero2w.dtb", dtb)
    write(dtb_root / f"boot/dtb-{RELEASE}/allwinner/overlay/sun50i-h616-i2c1-pi.dtbo", stock_dtbo.read_bytes())
    packages = work / "packages"
    packages.mkdir()
    subprocess.run(["dpkg-deb", "--build", str(image_root), str(packages / NATIVE_IMAGE)], check=True, capture_output=True)
    subprocess.run(["dpkg-deb", "--build", str(dtb_root), str(packages / NATIVE_DTB)], check=True, capture_output=True)
    final_root = work / "final-root"
    final_root.mkdir()
    subprocess.run(["dpkg-deb", "-x", str(packages / NATIVE_IMAGE), str(final_root)], check=True, capture_output=True)
    subprocess.run(["dpkg-deb", "-x", str(packages / NATIVE_DTB), str(final_root)], check=True, capture_output=True)
    stage_notices(REPOSITORY, final_root)
    write(final_root / "usr/share/common-licenses/GPL-3", b"fixture GPL license\n")
    write(final_root / "usr/share/doc/base-files/copyright", b"fixture base-files copyright\n")
    (final_root / "boot").mkdir(exist_ok=True)
    (final_root / "boot/Image").symlink_to(f"../usr/lib/linux-image-{RELEASE}/Image")
    phase5_outputs = {
        "etc/profile.d/octessera-welcome.sh": "tools/pi-image/stage4-octessera/files/root/etc/profile.d/octessera-welcome.sh",
        "etc/initramfs-tools/hooks/octessera-orange-boot-splash": "userpatches/overlay/etc/initramfs-tools/hooks/octessera-orange-boot-splash",
        "etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash": "userpatches/overlay/etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash",
        "usr/local/sbin/octessera-orange-oled-logo": "userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo",
        "usr/local/sbin/octessera-orange-oled-handoff.py": "userpatches/overlay/usr/local/sbin/octessera-orange-oled-handoff.py",
        "usr/local/sbin/octessera-orange-oled-lifecycle.py": "userpatches/overlay/usr/local/sbin/octessera-orange-oled-lifecycle.py",
        "usr/local/sbin/octessera-orange-oled-suspend": "userpatches/overlay/usr/local/sbin/octessera-orange-oled-suspend",
        "usr/local/lib/octessera/device_config.py": "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py",
        "usr/local/sbin/octessera-device-apply-reboot": "userpatches/overlay/usr/local/sbin/octessera-device-apply-reboot",
        "etc/systemd/system/octessera-device-apply-reboot.socket": "userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot.socket",
        "etc/systemd/system/octessera-device-apply-reboot@.service": "userpatches/overlay/etc/systemd/system/octessera-device-apply-reboot@.service",
        "usr/local/sbin/octessera-update": "userpatches/overlay/usr/local/sbin/octessera-update",
        "usr/local/sbin/octessera-update-broker": "userpatches/overlay/usr/local/sbin/octessera-update-broker",
        "usr/local/sbin/octessera-update-guard": "userpatches/overlay/usr/local/sbin/octessera-update-guard",
        "usr/local/sbin/octessera-update-recovery": "userpatches/overlay/usr/local/sbin/octessera-update-recovery",
        "usr/local/lib/octessera/updater_protocol.py": "tools/device-update/updater_protocol.py",
        "usr/local/lib/octessera/updater_contract.py": "tools/device-update/updater_contract.py",
        "usr/local/lib/octessera/updater_state.py": "tools/device-update/updater_state.py",
        "usr/local/lib/octessera/updater_assets.py": "tools/device-update/updater_assets.py",
        "usr/local/lib/octessera/updater_guard.py": "tools/device-update/updater_guard.py",
        "usr/local/lib/octessera/updater_cli.py": "tools/device-update/updater_cli.py",
        "usr/local/lib/octessera/updater_profiles.py": "tools/device-update/updater_profiles.py",
        "etc/systemd/system/octessera-update-guard.service": "userpatches/overlay/etc/systemd/system/octessera-update-guard.service",
        "etc/systemd/system/octessera-update-recovery.service": "userpatches/overlay/etc/systemd/system/octessera-update-recovery.service",
        "etc/systemd/system/octessera-update.socket": "userpatches/overlay/etc/systemd/system/octessera-update.socket",
        "etc/systemd/system/octessera-update@.service": "userpatches/overlay/etc/systemd/system/octessera-update@.service",
        "etc/sudoers.d/octessera-update": "userpatches/overlay/etc/sudoers.d/octessera-update",
        "usr/share/octessera/defaults/pi-default.json": "config/generated/pi/default.json",
        "usr/share/octessera/oled/octessera-mark.svg": "userpatches/overlay/usr/local/share/octessera-setup-ui/img/octessera-mark.svg",
        "usr/share/octessera/oled/octessera-wordmark.svg": "userpatches/overlay/usr/local/share/octessera-setup-ui/img/octessera-wordmark.svg",
        "usr/share/octessera/oled/octessera-pi-booting.rgb565": "userpatches/overlay/usr/local/share/octessera/oled/octessera-pi-booting.rgb565",
        "usr/share/octessera/oled/octessera-pi-shutdown.rgb565": "userpatches/overlay/usr/local/share/octessera/oled/octessera-pi-shutdown.rgb565",
        "etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf": "userpatches/overlay/etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf",
    }
    for installed_path, source_path in phase5_outputs.items():
        target = final_root / installed_path
        write(target, (REPOSITORY / source_path).read_bytes())
        managed = next(item for item in CONSTRUCTION["managed_outputs"] if item["path"] == installed_path)
        os.chmod(target, managed["mode"])
        os.chown(target, 0, 0)  # type: ignore[attr-defined]
    for installed_path, source_path in {
        "usr/local/sbin/octessera-sd-card": "tools/storage/octessera-sd-card",
        "usr/local/lib/octessera/octessera-sd-card-lib.sh": "tools/storage/octessera-sd-card-lib.sh",
        "usr/local/sbin/octessera-orange-storage": "tools/storage/octessera-orange-storage",
        "usr/local/sbin/octessera-orange-storage-control": "tools/storage/octessera-orange-storage-control",
        "etc/systemd/system/octessera-orange-sd-card.service": "userpatches/overlay/etc/systemd/system/octessera-orange-sd-card.service",
        "etc/udev/rules.d/99-octessera-orange-sd-card.rules": "userpatches/overlay/etc/udev/rules.d/99-octessera-orange-sd-card.rules",
        "etc/systemd/system/octessera-orange-storage-control.socket": "userpatches/overlay/etc/systemd/system/octessera-orange-storage-control.socket",
        "etc/systemd/system/octessera-orange-storage-control@.service": "userpatches/overlay/etc/systemd/system/octessera-orange-storage-control@.service",
    }.items():
        target = final_root / installed_path
        write(target, (REPOSITORY / source_path).read_bytes())
        os.chmod(target, 0o755 if installed_path.endswith(("octessera-sd-card", "octessera-orange-storage", "octessera-orange-storage-control")) else 0o644)
        os.chown(target, 0, 0)  # type: ignore[attr-defined]
    write(final_root / "etc/modules-load.d/octessera-orange-sd-card.conf", "mmc_spi\n")
    write(final_root / "etc/modules-load.d/octessera-orange-usb-gadget.conf", "musb_hdrc\nlibcomposite\nusb_f_uac2\nusb_f_midi\nusb_f_mass_storage\n")
    os.chown(final_root / "etc/modules-load.d/octessera-orange-sd-card.conf", 0, 0)  # type: ignore[attr-defined]
    os.chmod(final_root / "etc/modules-load.d/octessera-orange-sd-card.conf", 0o644)
    (final_root / "etc/systemd/system/multi-user.target.wants").mkdir(parents=True, exist_ok=True)
    (final_root / "etc/systemd/system/multi-user.target.wants/octessera-orange-sd-card.service").symlink_to("../octessera-orange-sd-card.service")
    (final_root / "etc/systemd/system/sockets.target.wants").mkdir(parents=True, exist_ok=True)
    (final_root / "etc/systemd/system/sockets.target.wants/octessera-orange-storage-control.socket").symlink_to("../octessera-orange-storage-control.socket")
    overlay_names = {"spi": "octessera-h618-spi1-oled-sd2", "input": "octessera-h618-input-routing", "audio": "octessera-ahub0-pcm5102"}
    for name, source in overlay_sources.items():
        overlay_name = overlay_names[name]
        source_target = final_root / f"usr/local/share/octessera/device-tree/{overlay_name}.dts"
        dtbo_target = final_root / f"boot/overlay-user/{overlay_name}.dtbo"
        write(source_target, source.read_bytes())
        write(dtbo_target, overlay_dtb[name].read_bytes())
        for target in (source_target, dtbo_target):
            managed = next(item for item in CONSTRUCTION["managed_outputs"] if item["path"] == target.relative_to(final_root).as_posix())
            os.chmod(target, managed["mode"])
            os.chown(target, 0, 0)  # type: ignore[attr-defined]
    for module_name in CONSTRUCTION["selected_initramfs"]["required_python_modules"]:
        write(final_root / f"usr/lib/python3.13/lib-dynload/{module_name}.cpython-313-aarch64-linux-gnu.so", b"synthetic-python-extension")
    write(final_root / "etc/systemd/system/octessera-orange-boot-splash.service", (REPOSITORY / "userpatches/overlay/etc/systemd/system/octessera-orange-boot-splash.service").read_bytes())
    write(final_root / "etc/systemd/system/octessera-orange-oled-shutdown.service", (REPOSITORY / "userpatches/overlay/etc/systemd/system/octessera-orange-oled-shutdown.service").read_bytes())
    write(final_root / "etc/systemd/system/octessera-orange-oled-suspend.service", (REPOSITORY / "userpatches/overlay/etc/systemd/system/octessera-orange-oled-suspend.service").read_bytes())
    write(final_root / FIRSTRUN_SERVICE_RELATIVE, FIRSTRUN_SERVICE)
    write(final_root / FIRSTRUN_EXECUTABLE_RELATIVE, FIRSTRUN_EXECUTABLE)
    os.chmod(final_root / FIRSTRUN_EXECUTABLE_RELATIVE, 0o755)
    write(final_root / FIRSTRUN_DEFAULTS_RELATIVE, "# configuration values for the armbian-firstrun service\nOPENSSHD_REGENERATE_HOST_KEYS=true\n")
    (final_root / "etc/ssh").mkdir(parents=True, exist_ok=True)
    (final_root / "etc/systemd/system/sysinit.target.wants").mkdir(parents=True, exist_ok=True)
    (final_root / "etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service").symlink_to("../octessera-orange-boot-splash.service")
    (final_root / "etc/systemd/system/multi-user.target.wants").mkdir(parents=True, exist_ok=True)
    (final_root / "etc/systemd/system/multi-user.target.wants/octessera-orange-oled-shutdown.service").symlink_to("../octessera-orange-oled-shutdown.service")
    (final_root / "etc/systemd/system/sleep.target.requires").mkdir(parents=True, exist_ok=True)
    (final_root / "etc/systemd/system/sleep.target.requires/octessera-orange-oled-suspend.service").symlink_to("../octessera-orange-oled-suspend.service")
    (final_root / "etc/systemd/system/sockets.target.wants").mkdir(parents=True, exist_ok=True)
    (final_root / "etc/systemd/system/sockets.target.wants/octessera-device-apply-reboot.socket").symlink_to("../octessera-device-apply-reboot.socket")
    (final_root / "etc/systemd/system/sockets.target.wants/octessera-update.socket").symlink_to("../octessera-update.socket")
    (final_root / "etc/systemd/system/multi-user.target.wants/octessera-update-recovery.service").symlink_to("../octessera-update-recovery.service")
    (final_root / FIRSTRUN_ENABLE_RELATIVE).parent.mkdir(parents=True, exist_ok=True)
    (final_root / FIRSTRUN_ENABLE_RELATIVE).symlink_to("/usr/lib/systemd/system/armbian-firstrun.service")
    for unit in SSH_MASKED_UNITS:
        mask = final_root / "etc/systemd/system" / unit
        mask.parent.mkdir(parents=True, exist_ok=True)
        mask.symlink_to("/dev/null")
    initramfs = make_cpio_initramfs(work, final_root)
    compressed_initramfs = subprocess.run(["zstd", "-q", "-c"], input=initramfs, capture_output=True, check=True).stdout
    write(final_root / f"boot/initrd.img-{RELEASE}", make_uboot_initramfs(compressed_initramfs))
    (final_root / "boot/uInitrd").symlink_to(f"initrd.img-{RELEASE}")
    write(final_root / "boot/armbianEnv.txt", "verbosity=1\nconsole=display\nuser_overlays=octessera-h618-spi1-oled-sd2 octessera-h618-input-routing octessera-ahub0-pcm5102\noverlays=i2c1-pi\n")
    (final_root / "etc/systemd/system").mkdir(parents=True, exist_ok=True)
    (final_root / "etc/systemd/system/serial-getty@ttyS0.service").symlink_to("/dev/null")
    write(final_root / "etc/os-release", "ID=armbian\n")
    write(final_root / "etc/octessera/build-metadata.env", f"OCTESSERA_IMAGE_MODE=diagnostic\nOCTESSERA_RUNTIME_ENABLED_DEFAULT=false\nOCTESSERA_SPI1_OLED_SD2_DTS_SHA256={sha256(overlay_sources['spi'])}\nOCTESSERA_SPI1_OLED_SD2_DTBO_SHA256={sha256(overlay_dtb['spi'])}\nOCTESSERA_AHUB0_PCM5102_DTS_SHA256={sha256(overlay_sources['audio'])}\nOCTESSERA_AHUB0_PCM5102_DTBO_SHA256={sha256(overlay_dtb['audio'])}\n")
    write(final_root / "etc/octessera/image-contract.json", '{"schema_version": 1, "image_kind": "diagnostic", "runtime_enabled_default": false}\n')
    write(final_root / "etc/passwd", "octessera:x:1000:1000:Octessera:/home/octessera:/bin/bash\noctessera-runtime:x:990:990:Octessera runtime:/nonexistent:/usr/sbin/nologin\n")
    write(final_root / "etc/sudoers", "octessera ALL=(root) NOPASSWD: /sbin/shutdown\n")
    (final_root / "home/octessera").mkdir(parents=True, exist_ok=True)
    write(final_root / "home/octessera/.hushlogin", b"")
    os.chown(final_root / "home/octessera/.hushlogin", 1000, 1000)  # type: ignore[attr-defined]
    write(final_root / "etc/pam.d/20-vendor-login", b"vendor\n")
    write(final_root / "etc/update-motd.d/20-vendor-status", b"vendor\n")
    write(final_root / "etc/shadow", "octessera:!:1:0:99999:7:::\noctessera-runtime:!:1:0:99999:7:::\n")
    write(final_root / "etc/group", "octessera:x:1000:\noctessera-runtime:x:990:\naudio:x:29:octessera-runtime\ni2c:x:998:octessera-runtime\nspi:x:997:octessera-runtime\ngpio:x:996:octessera-runtime\nvideo:x:44:octessera-runtime\n")
    (final_root / "var/lib/octessera/samples").mkdir(parents=True, exist_ok=True)
    os.chown(final_root / "var/lib/octessera/samples", 990, 990)  # type: ignore[attr-defined]
    write(final_root / "var/lib/dpkg/status", f"Package: {IMAGE_NAME}\nStatus: install ok installed\nVersion: {REVISION}\nArchitecture: arm64\n\n" f"Package: {DTB_NAME}\nStatus: install ok installed\nVersion: {REVISION}\nArchitecture: arm64\n")
    evidence_path = work / "evidence.env"
    evidence_values = {
        "image_package_native_basename": NATIVE_IMAGE,
        "dtb_package_native_basename": NATIVE_DTB,
        "artifact_suffix": ARTIFACT_SUFFIX,
        "image_package_sha256": sha256(packages / NATIVE_IMAGE),
        "dtb_package_sha256": sha256(packages / NATIVE_DTB),
        "image_dtb_sha256": hashlib.sha256(dtb).hexdigest(),
        "dtb_package_dtb_sha256": hashlib.sha256(dtb).hexdigest(),
        "dtb_byte_equal": "true",
        "stock_i2c1_dtbo_path": f"boot/dtb-{RELEASE}/allwinner/overlay/sun50i-h616-i2c1-pi.dtbo",
        "stock_i2c1_dtbo_sha256": sha256(stock_dtbo),
        "audio_dts_path": "userpatches/overlay/usr/local/share/octessera/device-tree/octessera-ahub0-pcm5102.dts",
        "audio_dts_sha256": sha256(overlay_sources["audio"]),
        "audio_dtbo_forbidden": "octessera-ahub0-pcm5102.dtbo",
        "packaged_config_expected_sha256": "922e8037090e2202afdf70d46ea50c29790dcece17b62155c28212e7b6554cbc",
        "final_config_sha256": hashlib.sha256(config).hexdigest(),
        "module_relative_path": MODULE_RELATIVE,
        "module_compressed_sha256": hashlib.sha256(module).hexdigest(),
        "module_decompressed_sha256": hashlib.sha256(module).hexdigest(),
        "module_vermagic": f"{RELEASE} SMP",
        "module_interface_string_marker": "interface_string",
        "module_interface_options_marker": "f_midi_opts_attr_interface_string",
        "module_interface_runtime_marker": "midi_interface_string",
    }
    evidence_path.write_text("\n".join(f"{key}={value}" for key, value in evidence_values.items()) + "\n")
    provenance_path = work / "kernel-provenance.txt"
    provenance_values = {
        "schema": "1",
        "image_package": CANONICAL_IMAGE,
        "image_package_native": NATIVE_IMAGE,
        "image_package_sha256": evidence_values["image_package_sha256"],
        "dtb_package": CANONICAL_DTB,
        "dtb_package_native": NATIVE_DTB,
        "dtb_package_sha256": evidence_values["dtb_package_sha256"],
        "artifact_suffix": ARTIFACT_SUFFIX,
        "evidence_sha256": sha256(evidence_path),
        "armbian_build_ref": "3da49cffcb8ac58a919d86816fec4659c410ff1e",
        "armbian_build_tag": "v26.11.0-trunk.22",
        "kernel_source_repository": "https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git",
        "kernel_source_branch": "linux-6.18.y",
        "kernel_source_commit": "1f99e9ab748fc5c32120de9c4eca31abfe54a4d5",
        "kernel_release": RELEASE,
        "source_lock_path": "userpatches/config/sources/git_sources.json",
        "source_lock_sha256": "e8550bd50d61630518a2470b8e9793cd71653ae0732bc6c1c87726b222529e30",
        "source_lock_source": "https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git",
        "source_lock_branch": "linux-6.18.y",
        "source_lock_commit": "1f99e9ab748fc5c32120de9c4eca31abfe54a4d5",
        "source_lock_effective_path": "config/sources/git_sources.json",
        "source_lock_effective_sha256": "e8550bd50d61630518a2470b8e9793cd71653ae0732bc6c1c87726b222529e30",
    }
    provenance_path.write_text("\n".join(f"{key}={value}" for key, value in provenance_values.items()) + "\n")
    return final_root, packages / NATIVE_IMAGE, packages / NATIVE_DTB, evidence_path, provenance_path


def verifier_args(root: Path, image: Path, dtb: Path, evidence: Path, provenance: Path, mode: str = "diagnostic", privileged: bool = False) -> list[str]:
    command = [sys.executable, str(TOOLS / "verify-orange-image.py")]
    if privileged:
        command = ["sudo", "-n", *command]
    return [*command, "--root", str(root), "--image-sha256", "a" * 64, "--linux-image", str(image), "--linux-dtb", str(dtb), "--evidence", str(evidence), "--provenance", str(provenance), "--manifest", str(REPOSITORY / "tools/kernel-patches/orange-midi-interface-manifest.json"), "--construction-contract", str(REPOSITORY / "resources/image-construction/boot-layers/orange-pi-zero-2w.json"), "--boot-proof-mode", "phase5-constructor", "--mode", mode]


def run_proof(args: list[str], expected: bool, cwd: Path | None = None) -> None:
    result = subprocess.run(args, capture_output=True, text=True, cwd=cwd)
    if (result.returncode == 0) != expected:
        raise AssertionError(result.stdout + result.stderr)


def run_proof_failure(args: list[str], expected_reason: str) -> None:
    result = subprocess.run(args, capture_output=True, text=True)
    if result.returncode == 0 or expected_reason not in result.stderr:
        raise AssertionError(f"Expected image proof failure {expected_reason!r}:\n{result.stdout}{result.stderr}")


def root_args(args: list[str], root: Path) -> list[str]:
    result = list(args)
    result[result.index("--root") + 1] = str(root)
    return result


def without_option(args: list[str], option: str) -> list[str]:
    index = args.index(option)
    return [*args[:index], *args[index + 2 :]]


def replace_option(args: list[str], option: str, value: Path) -> list[str]:
    result = list(args)
    result[result.index(option) + 1] = str(value)
    return result


def make_missing_builtin_fixture(work: Path, root: Path, image: Path, evidence: Path, provenance: Path) -> tuple[Path, Path, Path, Path]:
    negative_root = work / "negative-missing-builtin"
    copy_fixture_root(root, negative_root)
    config_path = negative_root / f"boot/config-{RELEASE}"
    config = config_path.read_bytes().replace(b"CONFIG_SPI_SPIDEV=y\n", b"", 1)
    write(config_path, config)
    image_root = work / "negative-image-root"
    subprocess.run(["dpkg-deb", "-R", str(image), str(image_root)], check=True, capture_output=True)
    write(image_root / f"boot/config-{RELEASE}", config)
    negative_image = work / "negative-packages" / NATIVE_IMAGE
    negative_image.parent.mkdir()
    subprocess.run(["dpkg-deb", "--build", str(image_root), str(negative_image)], check=True, capture_output=True)
    negative_evidence = work / "negative-evidence.env"
    evidence_lines = []
    for line in evidence.read_text().splitlines():
        key, _, value = line.partition("=")
        value = {"image_package_sha256": sha256(negative_image), "final_config_sha256": hashlib.sha256(config).hexdigest()}.get(key, value)
        evidence_lines.append(f"{key}={value}")
    negative_evidence.write_text("\n".join(evidence_lines) + "\n")
    negative_provenance = work / "negative-provenance.txt"
    provenance_lines = []
    for line in provenance.read_text().splitlines():
        key, _, value = line.partition("=")
        value = {"image_package_sha256": sha256(negative_image), "evidence_sha256": sha256(negative_evidence)}.get(key, value)
        provenance_lines.append(f"{key}={value}")
    negative_provenance.write_text("\n".join(provenance_lines) + "\n")
    return negative_root, negative_image, negative_evidence, negative_provenance
