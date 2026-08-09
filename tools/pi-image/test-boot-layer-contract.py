#!/usr/bin/env python3
from __future__ import annotations

import copy
import hashlib
import json
import re
import unittest
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
CONTRACT_PATH = ROOT / "resources/image-construction/boot-layers/raspberry-pi-zero-2w.json"
SHA256 = re.compile(r"^[0-9a-f]{64}$")
TOP_KEYS = {
    "schema",
    "schema_version",
    "contract_kind",
    "board_profile",
    "classification",
    "trusted_parent_finalization",
    "mutation_authority",
    "selected_initramfs_regeneration",
    "source_inputs",
    "live_parity_inputs",
    "notice_bundle",
    "managed_outputs",
    "selected_initramfs",
    "uart_invariants",
    "proofs",
    "expected_changes",
}


def validate(document: dict[str, Any], root: Path) -> None:
    if set(document) != TOP_KEYS:
        raise ValueError("boot-layer top-level keys are not exact")
    if document["schema"] != "octessera.image-construction/boot-layer/v1" or document["schema_version"] != 1:
        raise ValueError("boot-layer schema is not v1")
    if document["contract_kind"] != "boot-layer" or document["board_profile"] != "raspberry-pi-zero-2w":
        raise ValueError("boot-layer identity is invalid")
    if document["classification"] != "constructor-required" or document["trusted_parent_finalization"] != "forbidden":
        raise ValueError("boot-layer parent classification is invalid")
    if document["mutation_authority"] != "none" or document["selected_initramfs_regeneration"] != "required":
        raise ValueError("boot-layer authority or regeneration is invalid")

    source_inputs = document["source_inputs"]
    if not isinstance(source_inputs, list) or not source_inputs:
        raise ValueError("source inputs are empty")
    paths: set[str] = set()
    for source in source_inputs:
        if set(source) != {"path", "sha256", "size"} or source["path"] in paths:
            raise ValueError("source input shape is not exact")
        paths.add(source["path"])
        if not isinstance(source["path"], str) or source["path"].startswith("/") or ".." in Path(source["path"]).parts:
            raise ValueError("source input path is unsafe")
        if SHA256.fullmatch(source["sha256"]) is None or not isinstance(source["size"], int) or source["size"] < 0:
            raise ValueError("source input digest or size is invalid")
        actual = root / source["path"]
        if not actual.is_file() or hashlib.sha256(actual.read_bytes()).hexdigest() != source["sha256"] or actual.stat().st_size != source["size"]:
            raise ValueError(f"source input digest is stale: {source['path']}")

    live_inputs = document["live_parity_inputs"]
    if live_inputs != [
        {"path": "tools/pi/deploy-pi.sh", "sha256": "f6b0adeb72e2e0d23a979b092aab1ffa45f5fb4e44ae0bf9084cb666ebcf127d", "size": 17225},
        {"path": "tools/pi/provision/provision.sh", "sha256": "d9fa9729603cae621e9fde76ee2df32fed4d3036871637273ee3a7cfddb47ac3", "size": 13697},
    ]:
        raise ValueError("Raspberry live parity input identities are not exact")
    for source in live_inputs:
        actual = root / source["path"]
        if not actual.is_file() or hashlib.sha256(actual.read_bytes()).hexdigest() != source["sha256"] or actual.stat().st_size != source["size"]:
            raise ValueError(f"live parity input digest is stale: {source['path']}")
    deploy = (root / "tools/pi/deploy-pi.sh").read_text(encoding="utf-8")
    provision = (root / "tools/pi/provision/provision.sh").read_text(encoding="utf-8")
    setup = (root / "tools/pi-image/stage4-octessera/02-setup-service/00-run.sh").read_text(encoding="utf-8")
    boot_config = (root / "tools/pi-image/stage4-octessera/03-boot-config/00-run.sh").read_text(encoding="utf-8")
    console_pattern = r"(^|[[:space:]])console=(serial0|ttyAMA0|ttyS0)(,[^[:space:]]+)?([[:space:]]|$)"
    for text in (deploy, provision):
        if re.search(r"(?:cat|tee)[^\n]*octessera-welcome\.sh[^\n]*<<", text):
            raise ValueError("Raspberry live parity contains a welcome heredoc")
        for required in ("dtoverlay=disable-bt", "enable_uart=0", "serial-getty@serial0.service", "serial-getty@ttyAMA0.service", "serial-getty@ttyS0.service", "bluetooth.service", "hciuart.service"):
            if required not in text:
                raise ValueError(f"Raspberry live parity does not establish {required}")
        if console_pattern not in text or "while grep -Eq" not in text or "sed -i -E" not in text:
            raise ValueError("Raspberry live parity does not use exact serial-console token handling")
        if text.count("/usr/local/lib/octessera/rpi_uart_release.py") != 2:
            raise ValueError("Raspberry live parity does not remove and prove absence of the stale UART utility")
    if "sudo rm -f /usr/local/lib/octessera/rpi_uart_release.py" not in deploy or "test ! -e /usr/local/lib/octessera/rpi_uart_release.py" not in deploy:
        raise ValueError("Raspberry deploy parity does not remove and prove absence of the stale UART utility")
    if 'sudo rm -f "$(target_path /usr/local/lib/octessera/rpi_uart_release.py)"' not in provision or 'test ! -e "$(target_path /usr/local/lib/octessera/rpi_uart_release.py)"' not in provision:
        raise ValueError("Raspberry provision parity does not use SYSROOT-aware stale UART cleanup")
    if console_pattern not in boot_config or "grep -qxF 'dtoverlay=disable-bt'" not in boot_config or "grep -qxF 'enable_uart=0'" not in boot_config:
        raise ValueError("Raspberry constructor does not enforce exact inactive-UART boot state")
    if 'getty.target.wants"/serial-getty@*.service' not in setup:
        raise ValueError("Raspberry constructor does not remove serial-getty enablement links")
    for required in ("bluetooth.service", "hciuart.service", "ln -s /dev/null", "rpi_uart_release.py"):
        if required not in setup:
            raise ValueError(f"Raspberry constructor does not establish {required}")
    if "tools/pi-image/stage4-octessera/files/root/etc/profile.d/octessera-welcome.sh" not in deploy or "IMAGE_ROOT/etc/profile.d/octessera-welcome.sh" not in provision:
        raise ValueError("Raspberry live parity does not use the canonical welcome source")
    if document["notice_bundle"] != {
        "manifest": "resources/legal/notice-bundle.json",
        "stager": "tools/legal/stage_notices.py",
        "installed_root": "usr/share/doc/octessera",
        "installed_outputs": "manifest-files",
        "proof": "tools/pi-image/verify-boot-layout.sh",
        "parent_sentinels": ["usr/share/common-licenses/GPL-3", "usr/share/doc/base-files/copyright"],
        "firmware_license_path": None,
        "firmware_license_gate": "full-constructor",
    }:
        raise ValueError("Raspberry legal notice contract is not exact")
    protected = ("/etc/motd", "/etc/issue", "/usr/share/doc", "/usr/share/common-licenses", "/usr/share/doc/base-files/copyright")
    protected_scripts = [
        root / "tools/pi-image/stage4-octessera/02-setup-service/00-run.sh",
        root / "tools/pi-image/stage4-octessera/04-sanitize-release-image/00-run.sh",
        root / "tools/pi/provision/provision.sh",
        root / "tools/pi/deploy-pi.sh",
    ]
    for script in protected_scripts:
        for line in script.read_text(encoding="utf-8").splitlines():
            if any(path in line for path in protected) and "/usr/share/doc/octessera" not in line:
                raise ValueError(f"constructor mutates a parent legal path: {script}")
    stage_setup = (root / "tools/pi-image/stage4-octessera/02-setup-service/00-run.sh").read_text(encoding="utf-8")
    if "tools/legal" not in stage_setup or "--destination-root \"$STAGE_FILES/root\"" not in stage_setup or "/usr/share/doc/octessera" not in stage_setup:
        raise ValueError("Raspberry constructor does not require and install the staged legal tree")

    outputs = document["managed_outputs"]
    if not isinstance(outputs, list) or not outputs:
        raise ValueError("managed outputs are empty")
    output_paths: set[str] = set()
    for output in outputs:
        output_type = output.get("type")
        expected = {"classification", "path", "type", "mode", "uid", "gid"} if output_type == "file" else {"classification", "path", "type", "target"}
        if set(output) != expected or output["path"] in output_paths or output_type not in {"file", "symlink"}:
            raise ValueError("managed output shape is not exact")
        output_paths.add(output["path"])
        if output_type == "file" and any(not isinstance(output[key], int) or output[key] < 0 for key in ("mode", "uid", "gid")):
            raise ValueError("managed output metadata is invalid")

    selected = document["selected_initramfs"]
    if set(selected) != {
        "path",
        "byte_bindings",
        "required_symlinks",
        "required_regular_executables",
        "forbidden_entry_prefixes",
        "size_limits",
        "required_module_names",
    } or selected["path"] != "octessera/initrd.img-6.12.93-octessera-rpi-v8-0.7.5":
        raise ValueError("selected initramfs contract is not exact")
    if selected["byte_bindings"] != [
        {"role": "splash-script", "archive_path": "scripts/init-premount/octessera-boot-splash", "rootfs_path": "etc/initramfs-tools/scripts/init-premount/octessera-boot-splash", "rootfs_type": "regular-executable"},
        {"role": "runtime", "archive_path": "usr/local/bin/octessera-pi", "rootfs_path": "usr/local/bin/octessera-pi", "rootfs_type": "symlink", "rootfs_target": "/opt/octessera/current/octessera-pi", "rootfs_resolution": {"current_path": "opt/octessera/current", "current_target_pattern": r"^/opt/octessera/releases/[0-9]+\.[0-9]+\.[0-9]+$", "resolved_path": "opt/octessera/current/octessera-pi", "resolved_type": "regular-executable"}},
    ] or selected["required_symlinks"] != [
        {"path": "bin", "target": "usr/bin"},
        {"path": "usr/bin/sh", "target": "dash"},
    ] or selected["required_regular_executables"] != [
        "usr/bin/dash",
        "usr/bin/setsid",
        "usr/bin/sleep",
        "usr/bin/cat",
        "usr/bin/mv",
        "usr/bin/chmod",
        "usr/bin/chown",
        "usr/bin/rm",
    ] or selected["forbidden_entry_prefixes"] != ["bin/"] or selected["size_limits"] != {
        "min_regular_bytes": 1,
        "max_entry_bytes": 67108864,
        "max_total_regular_bytes": 268435456,
        "symlink_size": "target-bytes",
    } or selected["required_module_names"] != ["spi-bcm2835", "spidev"]:
        raise ValueError("selected initramfs inventory is not exact")

    if document["uart_invariants"] != {
        "required_config": ["dtoverlay=disable-bt", "enable_uart=0"],
        "forbidden_config": ["enable_uart=1"],
        "forbidden_cmdline_prefixes": ["console=serial0", "console=ttyAMA0", "console=ttyS0"],
        "masks": ["serial-getty@serial0.service", "serial-getty@ttyAMA0.service", "serial-getty@ttyS0.service"],
        "disabled_services": ["bluetooth.service", "hciuart.service"],
    }:
        raise ValueError("Raspberry UART invariants are not exact")

    proofs = document["proofs"]
    if [proof["name"] for proof in proofs] != ["initramfs-watchdog", "systemd-graph", "sanitized-image", "boot-layout-fixture", "kernel-image"]:
        raise ValueError("boot-layer proof set is not exact")
    for proof in proofs:
        if set(proof) != {"name", "path"} or not (root / proof["path"]).is_file():
            raise ValueError("boot-layer proof path is invalid")
    if document["expected_changes"] != {"packages": [], "accounts": [], "kernel": [], "firmware": []}:
        raise ValueError("boot-layer expected changes are not empty")


class BootLayerContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.document = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))

    def test_contract_is_constructor_only_and_source_bound(self) -> None:
        validate(self.document, ROOT)

    def test_stale_source_digest_is_rejected(self) -> None:
        altered = copy.deepcopy(self.document)
        altered["source_inputs"][0]["sha256"] = "0" * 64
        with self.assertRaises(ValueError):
            validate(altered, ROOT)

    def test_missing_required_initramfs_entry_is_rejected(self) -> None:
        altered = copy.deepcopy(self.document)
        altered["selected_initramfs"]["required_regular_executables"].pop()
        with self.assertRaises(ValueError):
            validate(altered, ROOT)

    def test_extra_output_and_wrong_classification_are_rejected(self) -> None:
        altered = copy.deepcopy(self.document)
        altered["managed_outputs"].append(copy.deepcopy(altered["managed_outputs"][0]))
        with self.assertRaises(ValueError):
            validate(altered, ROOT)
        altered = copy.deepcopy(self.document)
        altered["classification"] = "trusted-parent-finalization"
        with self.assertRaises(ValueError):
            validate(altered, ROOT)

    def test_selected_initramfs_semantic_fields_are_fail_closed(self) -> None:
        for field, mutate in (
            ("required_symlinks", lambda value: value.pop()),
            ("required_regular_executables", lambda value: value.pop()),
            ("forbidden_entry_prefixes", lambda value: value.__setitem__(0, "../bin/")),
            ("byte_bindings", lambda value: value[0].pop("rootfs_type")),
            ("runtime_resolution", lambda value: value[1]["rootfs_resolution"].update(current_target_pattern="releases/latest")),
            ("size_limits", lambda value: value.update(min_regular_bytes=0)),
        ):
            altered = copy.deepcopy(self.document)
            mutate(altered["selected_initramfs"]["byte_bindings"] if field == "runtime_resolution" else altered["selected_initramfs"][field])
            with self.assertRaises(ValueError):
                validate(altered, ROOT)


if __name__ == "__main__":
    unittest.main()
