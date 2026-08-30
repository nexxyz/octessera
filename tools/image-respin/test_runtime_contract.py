from __future__ import annotations

import copy
import json
import tempfile
import unittest
from pathlib import Path
import sys
from typing import Any

sys.path.insert(0, str(Path(__file__).parent))

from runtime_contract import MutationError, load_contract, validate_parent_context
from current_parent import parent_context


ROOT = Path(__file__).resolve().parents[2]
CONTRACTS = ROOT / "resources" / "image-mutations"


class RuntimeContractTests(unittest.TestCase):
    def _document(self, board: str) -> dict[str, Any]:
        return json.loads((CONTRACTS / f"{board}.json").read_text(encoding="utf-8"))

    def _assert_invalid(self, document: dict[str, Any]) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "contract.json"
            path.write_text(json.dumps(document), encoding="utf-8")
            with self.assertRaises(MutationError):
                load_contract(path)

    def test_shipped_contracts_are_valid(self) -> None:
        for board in ("raspberry-pi-zero-2w", "orange-pi-zero-2w"):
            with self.subTest(board=board):
                contract, digest = load_contract(CONTRACTS / f"{board}.json")
                self.assertEqual(contract["board_profile"], board)
                self.assertEqual(len(digest), 64)

    def test_rejects_unknown_top_level_fields_and_missing_spec_fields(self) -> None:
        unknown = self._document("raspberry-pi-zero-2w")
        unknown["unexpected"] = True
        self._assert_invalid(unknown)

        missing = self._document("raspberry-pi-zero-2w")
        del missing["real_parents"][0]["xattrs"]
        self._assert_invalid(missing)

    def test_rejects_unsafe_paths_and_inconsistent_capabilities(self) -> None:
        unsafe = self._document("orange-pi-zero-2w")
        unsafe["managed"]["state"] = "../update-state.json"
        self._assert_invalid(unsafe)

        capability = self._document("orange-pi-zero-2w")
        metadata = capability["build_metadata_contract"]
        metadata["capability"] = "00"
        self._assert_invalid(capability)

    def test_requires_exact_orange_metadata_key_contract(self) -> None:
        document = self._document("orange-pi-zero-2w")
        metadata = document["build_metadata_contract"]
        metadata["required_keys"] = copy.deepcopy(metadata["required_keys"][:-1])
        self._assert_invalid(document)

    def test_requires_exact_orange_metadata_preimage_and_output_modes(self) -> None:
        for field, value in (("preimage_mode", 436), ("mode", 436)):
            with self.subTest(field=field):
                document = self._document("orange-pi-zero-2w")
                document["build_metadata_contract"][field] = value
                self._assert_invalid(document)

    def test_only_current_parent_context_is_accepted(self) -> None:
        current = parent_context(ROOT)
        self.assertEqual(validate_parent_context(current, "orange-pi-zero-2w"), current)
        legacy = {
            "schema": "octessera.image-parent-trust/v1",
            "repository": "nexxyz/octessera",
            "tag": "v9.9.9",
            "source_commit": "a" * 40,
            "asset": {
                "name": "octessera-9.9.9-raspberry-pi-zero-2w.img.zip",
                "node_id": "RA_test-parent",
                "size": 123,
                "sha256": "b" * 64,
            },
        }
        with self.assertRaises(MutationError):
            validate_parent_context(legacy, "raspberry-pi-zero-2w")


if __name__ == "__main__":
    unittest.main()
