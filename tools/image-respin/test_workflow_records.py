from __future__ import annotations

import copy
import json
import sys
import unittest
from pathlib import Path


sys.path.insert(0, str(Path(__file__).parent))

from requested_build_record import PROOF_PACKAGES, REQUIRED_FILES, build_record, validate_record
from record_validation import RecordError


ROOT = Path(__file__).resolve().parents[2]
PARENT_RECORD = ROOT / "resources/image-parents/orange-pi-zero-2w-current.json"
FEATURE_COMMAND = "cross build --release --locked --target aarch64-unknown-linux-gnu -p octessera-pi --features hardware-orange-pi-zero-2w"


def requested() -> dict:
    return build_record(
        root=ROOT,
        source_sha="a" * 40,
        version="0.8.2",
        board="orange-pi-zero-2w",
        feature_command=FEATURE_COMMAND,
        input_files=[ROOT / path for path in REQUIRED_FILES],
        parent_record=PARENT_RECORD,
        rustc_vv="rustc 1.90.0\nhost: x86_64-unknown-linux-gnu\n",
        cargo_version="cargo 1.90.0",
        cross_version="cross 0.2.5",
        container_rustc_vv="rustc 1.90.0 container",
        container_cargo_version="cargo 1.90.0 container",
        cross_image_id="sha256:" + "a" * 64,
        cross_repo_digests=[],
        base_image_id="sha256:" + "c" * 64,
        base_repo_digests=["rust@sha256:" + "b" * 64],
        proof_packages={name: "1.0.0" for name in PROOF_PACKAGES},
    )


def write_json(path: Path, value: dict) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


class WorkflowRecordTests(unittest.TestCase):
    def test_requested_record_is_deterministic_and_current_parent_bound(self) -> None:
        first = requested()
        second = requested()
        self.assertEqual(first, second)
        validate_record(first, ROOT)
        self.assertEqual(first["parent_record"]["path"], "resources/image-parents/orange-pi-zero-2w-current.json")
        self.assertEqual(first["source"]["feature_command"], FEATURE_COMMAND)
        self.assertEqual([item["path"] for item in first["inputs"]], sorted(REQUIRED_FILES))

    def test_requested_record_rejects_raspberry_without_a_current_parent(self) -> None:
        with self.assertRaises(RecordError):
            build_record(
                root=ROOT,
                source_sha="a" * 40,
                version="0.8.2",
                board="raspberry-pi-zero-2w",
                feature_command="cross build --release --features hardware-raspberry-pi-zero-2w",
                input_files=[ROOT / path for path in REQUIRED_FILES],
                parent_record=PARENT_RECORD,
                rustc_vv="rustc",
                cargo_version="cargo",
                cross_version="cross",
                container_rustc_vv="rustc",
                container_cargo_version="cargo",
                cross_image_id="sha256:" + "a" * 64,
                cross_repo_digests=[],
                base_image_id="sha256:" + "c" * 64,
                base_repo_digests=["rust@sha256:" + "b" * 64],
                proof_packages={name: "1.0.0" for name in PROOF_PACKAGES},
            )

    def test_requested_record_rejects_tampered_parent_identity(self) -> None:
        record = requested()
        record["parent_record"]["sha256"] = "0" * 64
        with self.assertRaises(RecordError):
            validate_record(record, ROOT)

    def test_requested_record_rejects_tampered_toolchain(self) -> None:
        record = copy.deepcopy(requested())
        record["toolchain"]["host_orchestration"]["cross_version"] = ""
        with self.assertRaises(RecordError):
            validate_record(record, ROOT)


if __name__ == "__main__":
    unittest.main()
