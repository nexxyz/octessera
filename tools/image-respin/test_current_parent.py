from __future__ import annotations

import copy
import hashlib
import json
import tempfile
import unittest
import zipfile
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from current_parent import (
    CurrentParentError,
    load_record,
    parent_context,
    validate_archive,
    validate_artifact_metadata,
    validate_downloaded_directory,
    validate_run_metadata,
)


ROOT = Path(__file__).resolve().parents[2]
RECORD_PATH = ROOT / "resources/image-parents/orange-pi-zero-2w-current.json"


class CurrentParentTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.record, cls.record_digest = load_record(ROOT, RECORD_PATH)

    def test_record_and_context_are_exact(self) -> None:
        context = parent_context(ROOT, RECORD_PATH)
        self.assertEqual(context["record"], {"path": "resources/image-parents/orange-pi-zero-2w-current.json", "sha256": self.record_digest, "size": RECORD_PATH.stat().st_size})
        self.assertEqual(context["image"], self.record["image"])
        self.assertEqual(context["artifact"], self.record["artifact"])

    def test_run_and_artifact_metadata_bind_the_current_parent(self) -> None:
        run = {"id": 33301343618, "head_sha": "f7db4257171ebaa80ad59a68e8f8d8ce311f81cc", "head_branch": "main", "status": "completed", "conclusion": "success"}
        validate_run_metadata(run, self.record)
        artifact = {"id": 9730022123, "name": "octessera-orange-image-release-assets", "size_in_bytes": 512083160, "digest": "sha256:2532da85dc315328061d9795b0fa3f104cd7feee3d5d04b67495b9309a2ab35a", "expired": False, "expires_at": "2026-11-28T08:20:32Z", "workflow_run": {"id": 33301343618, "head_sha": "f7db4257171ebaa80ad59a68e8f8d8ce311f81cc"}}
        validate_artifact_metadata(artifact, self.record)
        altered = copy.deepcopy(artifact)
        altered["digest"] = "sha256:" + "0" * 64
        with self.assertRaises(CurrentParentError):
            validate_artifact_metadata(altered, self.record)

    def test_archive_rejects_non_exact_or_unsafe_entries_without_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive_path = root / "parent.zip"
            with zipfile.ZipFile(archive_path, "w") as archive:
                archive.writestr("../escape", b"bad")
            output = root / "output"
            with self.assertRaises(CurrentParentError):
                validate_archive(archive_path, self.record, output)
            self.assertFalse(output.exists())

    def test_record_rejects_modified_identity(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / RECORD_PATH.name
            altered = copy.deepcopy(self.record)
            altered["image"]["sha256"] = "0" * 64
            path.write_text(json.dumps(altered), encoding="utf-8")
            with self.assertRaises(CurrentParentError):
                load_record(ROOT, path)

    def test_record_rejects_duplicate_keys(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "resources/image-parents/orange-pi-zero-2w-current.json"
            path.parent.mkdir(parents=True)
            path.write_text('{"schema":"one","schema":"two"}', encoding="utf-8")
            with self.assertRaises(CurrentParentError):
                load_record(root)

    def test_record_values_are_read_from_the_record(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "resources/image-parents/orange-pi-zero-2w-current.json"
            record = copy.deepcopy(self.record)
            image_name = "octessera-9.9.9-orange-pi-zero-2w.img.xz"
            record["version"] = "9.9.9"
            record["constructor"] = {"run_id": 42, "source_sha": "a" * 40}
            record["artifact"].update(
                {
                    "id": 43,
                    "name": "reviewed-parent-assets",
                    "size": 44,
                    "digest": "sha256:" + "b" * 64,
                    "expires_at": "2099-01-01T00:00:00Z",
                    "entries": [
                        "linux-dtb-current-sunxi64_9.9_arm64.deb",
                        "linux-image-current-sunxi64_9.9_arm64.deb",
                        image_name,
                        image_name + ".sha256",
                        "octessera-orange-image-proof.json",
                        "octessera-orange-kernel-evidence.env",
                        "octessera-orange-kernel-provenance.txt",
                        "SHA256SUMS-orange-pi-zero-2w.txt",
                    ],
                }
            )
            record["image"] = {"name": image_name, "size": 45, "sha256": "c" * 64}
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps(record), encoding="utf-8")
            loaded, _ = load_record(root)
            self.assertEqual(loaded, record)

    def test_historical_expiry_loads_but_live_metadata_rejects_it(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / "resources/image-parents/orange-pi-zero-2w-current.json"
            record = copy.deepcopy(self.record)
            record["artifact"]["expires_at"] = "2000-01-01T00:00:00Z"
            path.parent.mkdir(parents=True)
            path.write_text(json.dumps(record), encoding="utf-8")
            loaded, _ = load_record(root)
            self.assertEqual(loaded["artifact"]["expires_at"], "2000-01-01T00:00:00Z")
            metadata = {
                "id": record["artifact"]["id"],
                "name": record["artifact"]["name"],
                "size_in_bytes": record["artifact"]["size"],
                "digest": record["artifact"]["digest"],
                "expired": False,
                "expires_at": record["artifact"]["expires_at"],
                "workflow_run": {
                    "id": record["constructor"]["run_id"],
                    "head_sha": record["constructor"]["source_sha"],
                },
            }
            with self.assertRaises(CurrentParentError):
                validate_artifact_metadata(metadata, loaded)

    def test_checksum_summary_covers_companions_but_not_the_image(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            record = copy.deepcopy(self.record)
            image_name = record["image"]["name"]
            image_bytes = b"synthetic current parent image"
            image_digest = hashlib.sha256(image_bytes).hexdigest()
            record["image"] = {"name": image_name, "size": len(image_bytes), "sha256": image_digest}
            summary_name = "SHA256SUMS-orange-pi-zero-2w.txt"
            image_checksum_name = image_name + ".sha256"
            for name in record["artifact"]["entries"]:
                if name == summary_name:
                    continue
                if name == image_name:
                    content = image_bytes
                elif name == image_checksum_name:
                    content = f"{image_digest}  {image_name}\n".encode()
                else:
                    content = name.encode()
                (directory / name).write_bytes(content)
            covered = [name for name in record["artifact"]["entries"] if name not in {image_name, summary_name}]
            summary = "\n".join(
                f"{hashlib.sha256((directory / name).read_bytes()).hexdigest()}  {name}" for name in covered
            ) + "\n"
            (directory / summary_name).write_text(summary, encoding="utf-8")
            validate_downloaded_directory(directory, record)
            (directory / summary_name).write_text(summary + f"{image_digest}  {image_name}\n", encoding="utf-8")
            with self.assertRaises(CurrentParentError):
                validate_downloaded_directory(directory, record)


if __name__ == "__main__":
    unittest.main()
