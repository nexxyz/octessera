from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))

from setup_contract import contract_for_board, load_contract, setup_source_paths, validate_sources, validate_tracked_sources


ROOT = Path(__file__).resolve().parents[2]
BOARDS = ("raspberry-pi-zero-2w", "orange-pi-zero-2w")
ORANGE_UI_ROOT = "userpatches/overlay/usr/local/share/octessera-setup-ui/"
ORANGE_UI_FILES = ("app.js", "index.html", "styles.css", "README.md", "octessera-mark.svg", "octessera-wordmark.svg")
ORANGE_SETUP_SERVICE_TARGET = "/etc/systemd/system/octessera-setup.service"
TRUSTED_FIXTURE_IDENTITIES = {
    "tools/pi-image/fixtures/trusted-parent-v0.7.5/boot/config.txt": (1847, "1018cf257f0b22c1dde87770d0433d0e3e2f442461db33f847307d427642fd9e"),
    "tools/pi-image/fixtures/trusted-parent-v0.7.5/boot/cmdline.txt": (154, "284c0fe29f0f60cff7e0b9c370756f083148a6274e8cb445dcc5294e0a88bcd4"),
    "tools/pi-image/fixtures/trusted-parent-v0.7.5/root/boot/config.txt": (91, "c39b0866eec314a741f6cba65f10937b914408d6660d5a81f6b3a9ce81471010"),
}


class SetupContractTests(unittest.TestCase):
    def test_both_setup_contracts_are_composable_and_source_bound(self) -> None:
        for board in BOARDS:
            with self.subTest(board=board):
                contract, digest = load_contract(contract_for_board(board))
                self.assertEqual(contract["contract_kind"], "setup-layer")
                self.assertEqual(len(digest), 64)
                self.assertEqual(len(validate_sources(contract, ROOT)), len(contract["source_inputs"]))
                self.assertEqual([item["target"] for item in contract["directories"]], ["usr/local/share/octessera-setup-ui"])
                self.assertEqual(contract["directories"][0]["postimage"], "required")
                self.assertEqual(contract["directories"][0]["preimage"]["kind"], "absent" if board == "raspberry-pi-zero-2w" else "exact")
                if board == "orange-pi-zero-2w":
                    self.assertEqual(set(contract["directories"][0]["preimage"]) - {"kind"}, {"type", "mode", "uid", "gid", "symlink", "xattrs", "capability"})
                    enabled = next(item for item in contract["symlinks"] if item["classification"] == "first-boot-setup-enabled")
                    self.assertEqual((enabled["link_target"], enabled["preimage"]["link_target"], enabled["postimage"]), (ORANGE_SETUP_SERVICE_TARGET, ORANGE_SETUP_SERVICE_TARGET, "preserve"))
                self.assertFalse(any(contract["recipe"][key] for key in ("account_mutation", "package_mutation", "network_mutation", "boot_mutation", "firmware_mutation")))
                classifications = {item["classification"] for item in contract["entries"]}
                self.assertTrue({"setup-profile", "wifi-wrapper", "sidecar", "static-ui", "setup-unit", "request-path-unit", "request-unit"} <= classifications)
                self.assertEqual({item["classification"] for item in contract["symlinks"]}, {"enabled-request-path", "setup-service-disabled"} if board == "raspberry-pi-zero-2w" else {"enabled-request-path", "first-boot-setup-enabled"})

    def test_contract_rejects_source_digest_and_preimage_changes(self) -> None:
        contract, _ = load_contract(contract_for_board("raspberry-pi-zero-2w"))
        altered = json.loads(json.dumps(contract))
        altered["source_inputs"][0]["sha256"] = "0" * 64
        path = Path(tempfile.mkdtemp()) / "contract.json"
        path.write_text(json.dumps(altered), encoding="utf-8")
        loaded, _ = load_contract(path)
        with self.assertRaises(ValueError):
            validate_sources(loaded, ROOT)
        pinned = subprocess.check_output(["git", "-c", f"safe.directory={ROOT.as_posix()}", "show", "4eec2b7edf6619fa22c709d4a589237a5748de78:userpatches/overlay/usr/local/sbin/octessera-setup-sidecar"], cwd=ROOT)
        self.assertEqual(len(pinned), 9323)

    def _fixture_repository(self) -> tuple[Path, dict]:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        repository = Path(temporary.name)
        subprocess.run(["git", "init", "-q"], cwd=repository, check=True)
        (repository / "source").mkdir()
        contract = {"source_root": "source", "source_inputs": [{"path": "source/payload", "sha256": "0" * 64, "size": 0}], "entries": [{"source": "payload"}]}
        return repository, contract

    def test_tracked_source_passes_local_and_strict_modes(self) -> None:
        repository, contract = self._fixture_repository()
        (repository / "source/payload").write_bytes(b"")
        subprocess.run(["git", "add", "source/payload"], cwd=repository, check=True)
        validate_tracked_sources(contract, repository, strict=False)
        validate_tracked_sources(contract, repository, strict=True)

    def test_present_nonignored_untracked_source_is_local_only(self) -> None:
        repository, contract = self._fixture_repository()
        (repository / "source/payload").write_bytes(b"")
        validate_tracked_sources(contract, repository, strict=False)
        with self.assertRaises(ValueError):
            validate_tracked_sources(contract, repository, strict=True)
        with patch.dict(os.environ, {"CI": "true"}):
            with self.assertRaises(ValueError):
                validate_tracked_sources(contract, repository)

    def test_ignored_source_is_rejected_in_both_modes(self) -> None:
        repository, contract = self._fixture_repository()
        (repository / "source/payload").write_bytes(b"")
        (repository / ".gitignore").write_text("source/payload\n", encoding="utf-8")
        for strict in (False, True):
            with self.subTest(strict=strict), self.assertRaises(ValueError):
                validate_tracked_sources(contract, repository, strict=strict)

    def test_missing_source_is_rejected_in_both_modes(self) -> None:
        repository, contract = self._fixture_repository()
        for strict in (False, True):
            with self.subTest(strict=strict), self.assertRaises(ValueError):
                validate_tracked_sources(contract, repository, strict=strict)

    def test_source_path_inventory_is_exact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repository = Path(temporary)
            subprocess.run(["git", "init", "-q"], cwd=repository, check=True)
            contract = {"source_root": "source", "source_inputs": [{"path": "tracked", "sha256": "0" * 64, "size": 0}], "entries": [{"source": "payload"}]}
            (repository / "tracked").write_bytes(b"")
            (repository / "source").mkdir()
            (repository / "source/payload").write_bytes(b"")
            subprocess.run(["git", "add", "tracked"], cwd=repository, check=True)
            self.assertEqual(set(setup_source_paths(contract)), {"tracked", "source/payload"})
            with self.assertRaises(ValueError):
                validate_tracked_sources(contract, repository, strict=True)
            subprocess.run(["git", "add", "source/payload"], cwd=repository, check=True)
            validate_tracked_sources(contract, repository, strict=True)

    def test_shipped_setup_contract_sources_are_visible_locally(self) -> None:
        for board in BOARDS:
            with self.subTest(board=board):
                contract, _ = load_contract(contract_for_board(board))
                validate_tracked_sources(contract, ROOT, strict=False)

    def test_setup_source_rows_match_raw_bytes_and_clean_git_blobs(self) -> None:
        for board in BOARDS:
            with self.subTest(board=board):
                contract, _ = load_contract(contract_for_board(board))
                validate_sources(contract, ROOT)
                for item in contract["source_inputs"]:
                    path = ROOT / item["path"]
                    raw = path.read_bytes()
                    self.assertEqual(len(raw), item["size"], item["path"])
                    self.assertEqual(hashlib.sha256(raw).hexdigest(), item["sha256"], item["path"])
                    raw_blob = subprocess.check_output(["git", "hash-object", "--no-filters", item["path"]], cwd=ROOT, text=True).strip()
                    clean_blob = subprocess.check_output(["git", "hash-object", f"--path={item['path']}", item["path"]], cwd=ROOT, text=True).strip()
                    self.assertEqual(raw_blob, clean_blob, item["path"])
                for entry in contract["entries"]:
                    source = ROOT / contract["source_root"] / entry["source"]
                    self.assertEqual(hashlib.sha256(source.read_bytes()).hexdigest(), entry["sha256"], entry["source"])

    def test_orange_setup_ui_has_the_exact_effective_lf_rule(self) -> None:
        attributes = (ROOT / ".gitattributes").read_text(encoding="utf-8").splitlines()
        contract, _ = load_contract(contract_for_board("orange-pi-zero-2w"))
        paths = sorted(item["path"] for item in contract["source_inputs"] if item["path"].startswith(ORANGE_UI_ROOT))
        expected_paths = sorted(f"{ORANGE_UI_ROOT}{name}" for name in ORANGE_UI_FILES)
        expected_rules = {f"/{path} text eol=lf" for path in expected_paths}
        self.assertEqual(paths, expected_paths)
        self.assertTrue(expected_rules <= set(attributes))
        self.assertNotIn(f"/{ORANGE_UI_ROOT}** text eol=lf", attributes)
        output = subprocess.check_output(["git", "check-attr", "eol", "--", *paths], cwd=ROOT, text=True)
        self.assertEqual(output.splitlines(), [f"{path}: eol: lf" for path in paths])

    def test_trusted_parent_fixtures_have_exact_lf_attributes_and_hashes(self) -> None:
        attributes = (ROOT / ".gitattributes").read_text(encoding="utf-8").splitlines()
        paths = sorted(TRUSTED_FIXTURE_IDENTITIES)
        expected_rules = {f"/{path} text eol=lf" for path in paths}
        self.assertTrue(expected_rules <= set(attributes))
        output = subprocess.check_output(["git", "check-attr", "eol", "--", *paths], cwd=ROOT, text=True)
        self.assertEqual(output.splitlines(), [f"{path}: eol: lf" for path in paths])
        for path, (size, expected_hash) in TRUSTED_FIXTURE_IDENTITIES.items():
            raw = (ROOT / path).read_bytes()
            self.assertEqual(len(raw), size, path)
            self.assertEqual(hashlib.sha256(raw).hexdigest(), expected_hash, path)


if __name__ == "__main__":
    unittest.main()
