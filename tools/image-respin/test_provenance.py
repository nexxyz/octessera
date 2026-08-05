from __future__ import annotations

import sys
import shutil
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from provenance import TOOL_CODE_EXTERNAL_FILES, TOOL_CODE_FILES, build_provenance, canonical_json, digest_object, provenance_bytes, tool_code_digest


class ProvenanceTests(unittest.TestCase):
    def test_provenance_is_deterministic_and_binds_all_digests(self) -> None:
        arguments = {
            "board_profile": "orange-pi-zero-2w",
            "version": "1.2.3",
            "source_identity": {"commit": "abc", "tree": "def"},
            "parent_identity": {"prior_version": "1.0.0", "prior_binary_sha256": "a" * 64},
            "payload_digest": "b" * 64,
            "mutation_contract_digest": "c" * 64,
            "pre_inventory_digest": "d" * 64,
            "post_inventory_digest": "e" * 64,
            "changed_paths": ["opt/octessera/current", "usr/local/bin/octessera-pi"],
        }
        first = build_provenance(**arguments)
        second = build_provenance(**arguments)
        self.assertEqual(provenance_bytes(first), provenance_bytes(second))
        self.assertEqual(first["parent"]["digest"], digest_object(arguments["parent_identity"]))
        self.assertEqual(first["payload"]["digest"], "b" * 64)
        self.assertEqual(first["mutation_contract"]["digest"], "c" * 64)
        self.assertEqual(first["inventories"], {"pre": "d" * 64, "post": "e" * 64})
        self.assertEqual(first["finalizer"]["tool_code_digest"], tool_code_digest())
        self.assertNotEqual(first["finalizer"]["tool_code_digest"], arguments["source_identity"]["commit"])
        self.assertEqual(canonical_json({"b": 1, "a": 2}), '{"a":2,"b":1}')

    def test_tool_code_digest_changes_when_a_checked_module_changes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repository = Path(temporary)
            directory = repository / "tools/image-respin"
            directory.mkdir(parents=True)
            for name in TOOL_CODE_FILES:
                shutil.copy2(Path(__file__).with_name(name), directory / name)
            for name in TOOL_CODE_EXTERNAL_FILES:
                target = repository / name
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(Path(__file__).resolve().parents[2] / name, target)
            before = tool_code_digest(directory)
            (directory / "runtime_mutation.py").write_bytes((directory / "runtime_mutation.py").read_bytes() + b"\n")
            self.assertNotEqual(before, tool_code_digest(directory))


if __name__ == "__main__":
    unittest.main()
