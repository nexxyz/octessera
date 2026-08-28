from __future__ import annotations

import json
import os
import shutil
import struct
import subprocess
from pathlib import Path

from test_orange_image_proof_support import (
    FIRSTRUN_DEFAULTS_RELATIVE,
    FIRSTRUN_EXECUTABLE,
    FIRSTRUN_EXECUTABLE_RELATIVE,
    FIRSTRUN_ENABLE_RELATIVE,
    FIRSTRUN_SERVICE_RELATIVE,
    RESIZE_ENABLE_RELATIVE,
    RESIZE_SERVICE,
    RESIZE_SERVICE_RELATIVE,
    SSH_MASKED_UNITS,
    copy_fixture_root,
    run_proof,
    run_proof_failure,
    sha256,
    verifier_args,
    write,
)


def run_runtime_proof(work: Path, image: Path, dtb: Path, evidence: Path, provenance: Path) -> None:
    diagnostic = work / "final-root"
    assert "ConditionPathExists=/opt/octessera/current" in (diagnostic / "etc/systemd/system/octessera-orange-boot-splash.service").read_text()
    assert (diagnostic / "etc/systemd/system/sysinit.target.wants/octessera-orange-boot-splash.service").is_symlink()
    assert not (diagnostic / "opt/octessera/current").exists()
    production = work / "production"
    shutil.copytree(work / "final-root", production, symlinks=True)
    write(production / RESIZE_SERVICE_RELATIVE, RESIZE_SERVICE)
    (production / RESIZE_ENABLE_RELATIVE).parent.mkdir(parents=True, exist_ok=True)
    (production / RESIZE_ENABLE_RELATIVE).symlink_to("../../../usr/lib/systemd/system/armbian-resize-filesystem.service")
    os.chown(production / "home/octessera/.hushlogin", 1000, 1000)  # type: ignore[attr-defined]
    binary = b"\x7fELF\x02\x01\x01" + bytes(11) + struct.pack("<H", 183) + bytes(64)
    version = "0.5.0"
    release_dir = production / f"opt/octessera/releases/{version}"
    release_dir.mkdir(parents=True)
    write(release_dir / "octessera-pi", binary)
    binary_hash = sha256(release_dir / "octessera-pi")
    write(release_dir / "SHA256SUMS", f"{binary_hash}  octessera-pi\n")
    runtime_metadata = {"artifact_kind": "production-runtime", "binary_sha256": binary_hash, "name": "octessera-pi", "profile": "orange-pi-zero-2w", "runtime_ready": True, "version": version}
    write(release_dir / "octessera-runtime.json", json.dumps(runtime_metadata, sort_keys=True, indent=2) + "\n")
    updater_manifest = {"schema_version": 2, "updater_protocol": 2, "candidate_health_protocol": 1, "updater_supported": True, "distribution": "runtime-updater", "tag": f"v{version}", "version": version, "board_profile": "orange-pi-zero-2w", "arch": "aarch64-unknown-linux-gnu", "binary": "octessera-pi", "platforms": ["orange-pi-zero-2w", "linux-aarch64-device"]}
    write(release_dir / "update-manifest.json", json.dumps(updater_manifest) + "\n")
    (production / "opt/octessera/current").symlink_to(f"/opt/octessera/releases/{version}")
    (production / "usr/local/bin").mkdir(parents=True)
    (production / "usr/local/bin/octessera-pi").symlink_to("/opt/octessera/current/octessera-pi")
    write(production / "etc/octessera/image-contract.json", '{"schema_version": 1, "image_kind": "production", "runtime_enabled_default": true}\n')
    write(production / "etc/passwd", "octessera:x:1000:1000:Octessera:/home/octessera:/bin/bash\n" "octessera-runtime:x:990:990:Octessera runtime:/nonexistent:/usr/sbin/nologin\n")
    write(production / "etc/shadow", "octessera:*:1:0:99999:7:::\noctessera-runtime:!:1:0:99999:7:::\n")
    write(production / "etc/group", "octessera:x:1000:\noctessera-runtime:x:990:\naudio:x:29:octessera-runtime\ni2c:x:998:octessera-runtime\nspi:x:997:octessera-runtime\ngpio:x:996:octessera-runtime\nvideo:x:44:octessera-runtime\n")
    write(production / "opt/octessera/update-state.json", json.dumps({"schema_version": 2, "phase": "committed", "current": version, "previous": None, "updated_at": "1970-01-01T00:00:00Z", "release": updater_manifest, "asset": None}) + "\n")
    os.chown(production / "opt/octessera/update-state.json", 0, 0)  # type: ignore[attr-defined]
    os.chmod(production / "opt/octessera/update-state.json", 0o644)
    (production / "var/lib/octessera/presets").mkdir(parents=True)
    (production / "var/lib/octessera/samples").mkdir(parents=True, exist_ok=True)
    write(production / "etc/systemd/system/octessera.service", "[Unit]\nStartLimitIntervalSec=30s\nStartLimitBurst=3\nRequires=octessera-device-apply-reboot.socket\nRequires=octessera-provision-musical-default.service\nRequires=octessera-update-recovery.service\nAfter=octessera-device-apply-reboot.socket\n[Service]\nUser=octessera-runtime\nGroup=octessera-runtime\nEnvironment=OCTESSERA_EXPECTED_BOARD_PROFILE=orange-pi-zero-2w\nEnvironment=OCTESSERA_PI_STORE_DIR=/var/lib/octessera/presets\nEnvironment=OCTESSERA_PI_SAMPLES_DIR=/var/lib/octessera/samples\nEnvironment=OCTESSERA_CANDIDATE_HEALTH_PATH=/run/octessera/candidate-ready.json\nEnvironment=OCTESSERA_OLED_BOOT_HANDOFF=v1\nTTYPath=/dev/tty1\nTTYReset=yes\nSupplementaryGroups=audio i2c spi gpio tty video\nNoNewPrivileges=yes\nAmbientCapabilities=CAP_SYS_TTY_CONFIG\nCapabilityBoundingSet=CAP_SYS_TTY_CONFIG\nProtectSystem=strict\nReadWritePaths=/var/lib/octessera /run/octessera /run/octessera-boot /run/octessera-setup-request/inbox\nPrivateTmp=yes\nProtectHome=yes\nRuntimeDirectory=octessera\nLimitRTPRIO=70\nLimitMEMLOCK=infinity\nExecStart=/usr/local/bin/octessera-pi\nRestart=on-failure\nRestartPreventExitStatus=78\nRestartSec=5s\n")
    write(production / "etc/udev/rules.d/70-octessera-orange-runtime.rules", "KERNEL==\"i2c-2\", GROUP=\"octessera-runtime\", MODE=\"0660\"\nKERNEL==\"spidev1.0\", GROUP=\"octessera-runtime\", MODE=\"0660\"\nKERNEL==\"gpiochip1\", GROUP=\"octessera-runtime\", MODE=\"0660\"\n")
    write(production / "etc/udev/rules.d/10-wifi-disable-powermanagement.rules", 'KERNEL=="wlan*", ACTION=="add", RUN+="/sbin/iw dev %k set power_save off"\n')
    (production / "etc/udev/rules.d/09-disabled.rules").symlink_to("/dev/null")
    (production / "etc/systemd/system/multi-user.target.wants").mkdir(parents=True, exist_ok=True)
    (production / "etc/systemd/system/multi-user.target.wants/octessera.service").symlink_to("/etc/systemd/system/octessera.service")
    runtime_metadata_hash = sha256(release_dir / "octessera-runtime.json")
    sums_hash = sha256(release_dir / "SHA256SUMS")
    metadata_lines = (production / "etc/octessera/build-metadata.env").read_text().splitlines()
    sd_metadata = "\n".join(line for line in metadata_lines if line.startswith("OCTESSERA_SPI1_OLED_SD2_"))
    audio_metadata = "\n".join(line for line in metadata_lines if line.startswith("OCTESSERA_AHUB0_PCM5102_"))
    write(production / "etc/octessera/build-metadata.env", f"OCTESSERA_IMAGE_MODE=production\nOCTESSERA_RUNTIME_ENABLED_DEFAULT=true\nOCTESSERA_RUNTIME_VERSION={version}\nOCTESSERA_RUNTIME_BINARY_SHA256={binary_hash}\nOCTESSERA_RUNTIME_METADATA_SHA256={runtime_metadata_hash}\nOCTESSERA_RUNTIME_MANIFEST_SHA256={sums_hash}\n{sd_metadata}\n{audio_metadata}\n")
    can_privilege = shutil.which("sudo") is not None and subprocess.run(["sudo", "-n", "true"], check=False, capture_output=True).returncode == 0
    if can_privilege:
        try:
            subprocess.run(["sudo", "-n", "chown", "-R", "root:root", str(release_dir)], check=True)
            subprocess.run(["sudo", "-n", "chmod", "0555", str(release_dir), str(release_dir / "octessera-pi")], check=True)
            subprocess.run(["sudo", "-n", "chmod", "0444", str(release_dir / "octessera-runtime.json"), str(release_dir / "SHA256SUMS"), str(release_dir / "update-manifest.json")], check=True)
            subprocess.run(["sudo", "-n", "chown", "-R", "990:990", str(production / "var/lib/octessera")], check=True)
            subprocess.run(["sudo", "-n", "chown", "0:0", str(production / "etc/udev/rules.d/70-octessera-orange-runtime.rules")], check=True)
            subprocess.run(["sudo", "-n", "chmod", "0644", str(production / "etc/udev/rules.d/70-octessera-orange-runtime.rules")], check=True)
            run_proof(verifier_args(production, image, dtb, evidence, provenance, "production", True), True)
            for name, mutate, reason in (
                ("onboarding-marker", lambda root: write(root / "root/.not_logged_in_yet", b"first login\n"), "Orange Armbian onboarding marker remains"),
                ("missing-firstrun-service", lambda root: (root / FIRSTRUN_SERVICE_RELATIVE).unlink(), "Orange Armbian firstrun service is missing or symlinked"),
                ("missing-firstrun-executable", lambda root: (root / FIRSTRUN_EXECUTABLE_RELATIVE).unlink(), "Orange Armbian firstrun executable is missing, unsafe, or not executable"),
                ("symlinked-firstrun-executable", lambda root: ((root / FIRSTRUN_EXECUTABLE_RELATIVE).unlink(), (root / FIRSTRUN_EXECUTABLE_RELATIVE).symlink_to("/bin/sh")), "Orange Armbian firstrun executable is missing, unsafe, or not executable"),
                ("non-executable-firstrun", lambda root: os.chmod(root / FIRSTRUN_EXECUTABLE_RELATIVE, 0o644), "Orange Armbian firstrun executable is missing, unsafe, or not executable"),
                ("missing-host-key-regeneration", lambda root: write(root / FIRSTRUN_EXECUTABLE_RELATIVE, FIRSTRUN_EXECUTABLE.replace("dpkg-reconfigure openssh-server >/dev/null 2>&1", "echo no regeneration")), "Orange Armbian firstrun host-key regeneration behavior is missing"),
                ("missing-firstrun-enable", lambda root: (root / FIRSTRUN_ENABLE_RELATIVE).unlink(), "Orange Armbian firstrun service is not enabled"),
                ("disabled-host-key-regeneration", lambda root: write(root / FIRSTRUN_DEFAULTS_RELATIVE, "OPENSSHD_REGENERATE_HOST_KEYS=false\n"), "Orange Armbian host-key regeneration is not enabled"),
                ("duplicate-host-key-regeneration", lambda root: write(root / FIRSTRUN_DEFAULTS_RELATIVE, (root / FIRSTRUN_DEFAULTS_RELATIVE).read_text() + "OPENSSHD_REGENERATE_HOST_KEYS=true\n"), "Orange Armbian host-key regeneration assignment is missing or duplicated"),
                ("baked-host-key", lambda root: write(root / "etc/ssh/ssh_host_ed25519_key", b"baked host key\n"), "Orange image contains baked SSH host keys"),
                ("missing-ssh-mask", lambda root: (root / "etc/systemd/system" / SSH_MASKED_UNITS[0]).unlink(), "Orange SSH unit is not masked: ssh.service"),
            ):
                negative = work / name
                copy_fixture_root(production, negative)
                mutate(negative)
                run_proof_failure(verifier_args(negative, image, dtb, evidence, provenance, "production", True), reason)
            for name, mutate, reason in (
                ("missing-resize-service", lambda root: (root / RESIZE_SERVICE_RELATIVE).unlink(), "Orange resize service is missing or symlinked"),
                ("wrong-resize-order", lambda root: write(root / RESIZE_SERVICE_RELATIVE, (root / RESIZE_SERVICE_RELATIVE).read_text().replace("Before=basic.target", "Before=multi-user.target")), "Orange resize service directive is wrong: Unit.Before"),
                ("missing-resize-enable", lambda root: (root / RESIZE_ENABLE_RELATIVE).unlink(), "Orange resize service is not enabled for basic.target"),
                ("wrong-resize-enable", lambda root: ((root / RESIZE_ENABLE_RELATIVE).unlink(), (root / RESIZE_ENABLE_RELATIVE).symlink_to("/etc/systemd/system/armbian-resize-filesystem.service")), "Orange resize service is not enabled for basic.target"),
                ("symlinked-resize-service", lambda root: ((root / RESIZE_SERVICE_RELATIVE).unlink(), (root / RESIZE_SERVICE_RELATIVE).symlink_to("/etc/systemd/system/octessera.service")), "Orange resize service is missing or symlinked"),
            ):
                negative = work / name
                copy_fixture_root(production, negative)
                mutate(negative)
                run_proof_failure(verifier_args(negative, image, dtb, evidence, provenance, "production", True), reason)
            enabled = production / "etc/systemd/system/multi-user.target.wants/octessera.service"
            enabled.unlink()
            run_proof(verifier_args(production, image, dtb, evidence, provenance, "production", True), False)
        finally:
            owner = work.stat()
            subprocess.run(["sudo", "-n", "chown", "-R", f"{owner.st_uid}:{owner.st_gid}", str(work)], check=False)
