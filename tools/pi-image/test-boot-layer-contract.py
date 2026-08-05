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
        {"path": "tools/pi/deploy-pi.sh", "sha256": "5ba7792299b16e74f42362a346b116bcc1f10f320cb0faae4dd4e5e3be291b80", "size": 15497},
        {"path": "tools/pi/provision/provision.sh", "sha256": "f7619799f5ad2ab8f8b82243bff344199b47c1252bc568fef8b10ad8bb095e06", "size": 10993},
    ]:
        raise ValueError("Raspberry live parity input identities are not exact")
    for source in live_inputs:
        actual = root / source["path"]
        if not actual.is_file() or hashlib.sha256(actual.read_bytes()).hexdigest() != source["sha256"] or actual.stat().st_size != source["size"]:
            raise ValueError(f"live parity input digest is stale: {source['path']}")
    deploy = (root / "tools/pi/deploy-pi.sh").read_text(encoding="utf-8")
    provision = (root / "tools/pi/provision/provision.sh").read_text(encoding="utf-8")
    for text in (deploy, provision):
        if re.search(r"(?:cat|tee)[^\n]*octessera-welcome\.sh[^\n]*<<", text):
            raise ValueError("Raspberry live parity contains a welcome heredoc")
        if re.search(r'ensure_boot_config_line\s+"(?:dtoverlay=disable-bt|enable_uart=0)"', text):
            raise ValueError("Raspberry live parity owns UART config outside the utility")
        if "rpi_uart_release.py --live" not in text:
            raise ValueError("Raspberry live parity does not invoke the UART utility")
    if "tools/pi-image/stage4-octessera/files/root/etc/profile.d/octessera-welcome.sh" not in deploy or "IMAGE_ROOT/etc/profile.d/octessera-welcome.sh" not in provision:
        raise ValueError("Raspberry live parity does not use the canonical welcome source")
    if "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/rpi_uart_release.py" not in deploy or "IMAGE_ROOT/usr/local/lib/octessera/rpi_uart_release.py" not in provision:
        raise ValueError("Raspberry live parity does not use the canonical UART utility")
    for text in (deploy, provision):
        if re.search(r"bluetooth|hciuart|disable_service_if_present", text, re.IGNORECASE):
            raise ValueError("Raspberry live parity owns Bluetooth services outside the UART utility")
        if re.search(r"systemctl\s+(?:stop|disable|mask).*serial-getty|ensure_boot_config_line[^\n]*(?:console=(?:serial0|ttyAMA0|ttyS0)|enable_uart=)", text):
            raise ValueError("Raspberry live parity owns UART state outside the utility")

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
    if set(selected) != {"path", "required_entries", "required_module_names"} or selected["path"] != "octessera/initrd.img-6.12.93-octessera-rpi-v8-0.7.5":
        raise ValueError("selected initramfs contract is not exact")
    if selected["required_entries"] != [
        "scripts/init-premount/octessera-boot-splash",
        "usr/local/bin/octessera-pi",
        "usr/bin/setsid",
        "bin/sh",
        "bin/sleep",
        "bin/cat",
        "bin/mv",
        "bin/chmod",
        "bin/chown",
        "bin/rm",
    ] or selected["required_module_names"] != ["spi-bcm2835", "spidev"]:
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
    if [proof["name"] for proof in proofs] != ["initramfs-watchdog", "systemd-graph", "sanitized-image", "boot-layout-fixture", "kernel-image", "welcome-uart", "uart-release"]:
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
        altered["selected_initramfs"]["required_entries"].pop()
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


if __name__ == "__main__":
    unittest.main()
