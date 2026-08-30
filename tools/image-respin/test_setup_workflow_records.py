from __future__ import annotations

import copy
import sys
import unittest
from pathlib import Path


sys.path.insert(0, str(Path(__file__).parent))

from post_proof_record import ORANGE_TOOLS
from record_paths import identity
from record_validation import RecordError
from setup_workflow_record import SETUP_PROOF_TOOLS, _production_proof_identities, _validate_production_proofs, _validate_setup_proof_tools


ROOT = Path(__file__).resolve().parents[2]


class SetupWorkflowRecordTests(unittest.TestCase):
    def test_setup_proof_tools_are_current_parent_tools_only(self) -> None:
        self.assertEqual(SETUP_PROOF_TOOLS, {"orange-pi-zero-2w": ORANGE_TOOLS})
        tools = [identity(ROOT / path, ROOT) for path in ORANGE_TOOLS]
        _validate_setup_proof_tools(tools, ROOT, "orange-pi-zero-2w")
        with self.assertRaises(RecordError):
            _validate_setup_proof_tools(tools[:-1], ROOT, "orange-pi-zero-2w")
        altered = copy.deepcopy(tools)
        altered[0]["sha256"] = "0" * 64
        with self.assertRaises(RecordError):
            _validate_setup_proof_tools(altered, ROOT, "orange-pi-zero-2w")

    def test_raspberry_production_proofs_fail_closed(self) -> None:
        with self.assertRaises(RecordError):
            _production_proof_identities(ROOT, "raspberry-pi-zero-2w", {})
        with self.assertRaises(RecordError):
            _validate_production_proofs({}, ROOT, "raspberry-pi-zero-2w")

    def test_production_proof_output_set_is_exact(self) -> None:
        with self.assertRaises(RecordError):
            _validate_production_proofs({}, ROOT, "orange-pi-zero-2w")


if __name__ == "__main__":
    unittest.main()
