from __future__ import annotations

import copy
import json
import tempfile
import unittest
from pathlib import Path
import sys
from typing import Any

sys.path.insert(0, str(Path(__file__).parent))

from runtime_contract import MutationError, load_contract


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


if __name__ == "__main__":
    unittest.main()
