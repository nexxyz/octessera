import copy
import hashlib
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from trust_manifest import (
    ManifestError,
    load_manifest,
    parse_json_text,
    validate_downloaded_directory,
    validate_manifest_document,
    validate_release_document,
)


ROOT = Path(__file__).resolve().parents[2]
MANIFEST_PATH = ROOT / "resources" / "image-parents" / "v0.7.5-trust-manifest.json"
VERIFIER_PATH = ROOT / "tools" / "image-respin" / "verify-parent-release.py"


def checked_manifest() -> dict:
    return load_manifest(MANIFEST_PATH)


def gh_release_fixture(manifest: dict) -> dict:
    return {
        "tagName": "v0.7.5",
        "url": "https://github.com/nexxyz/octessera/releases/tag/v0.7.5",
        "publishedAt": "2026-08-02T14:27:16Z",
        "repository": "nexxyz/octessera",
        "sourceCommit": "4eec2b7edf6619fa22c709d4a589237a5748de78",
        "isDraft": False,
        "isPrerelease": False,
        "assets": [
            {
                "id": asset["node_id"],
                "name": asset["name"],
                "size": asset["size"],
                "digest": f"sha256:{asset['sha256']}",
                "contentType": asset["content_type"],
            }
            for asset in manifest["assets"]
        ],
    }


def api_release_fixture(manifest: dict) -> dict:
    fixture = gh_release_fixture(manifest)
    return {
        "tag_name": fixture["tagName"],
        "html_url": fixture["url"],
        "published_at": fixture["publishedAt"],
        "repository": {"full_name": fixture["repository"]},
        "source_commit": fixture["sourceCommit"],
        "draft": fixture["isDraft"],
        "prerelease": fixture["isPrerelease"],
        "assets": [
            {
                "node_id": asset["id"],
                "name": asset["name"],
                "size": asset["size"],
                "digest": asset["digest"],
                "content_type": asset["contentType"],
            }
            for asset in fixture["assets"]
        ],
    }


def directory_fixture(manifest: dict, board: str) -> dict:
    fixture = copy.deepcopy(manifest)
    names = next(parent for parent in fixture["image_parents"] if parent["board"] == board)
    selected = {names["asset"], *names["proof_companion_assets"]}
    for asset in fixture["assets"]:
        if asset["name"] in selected:
            payload = asset["name"].encode("utf-8")
            asset["size"] = len(payload)
            asset["sha256"] = hashlib.sha256(payload).hexdigest()
    return fixture


class TrustManifestTests(unittest.TestCase):
    def assert_manifest_rejected(self, mutation) -> None:
        manifest = checked_manifest()
        mutation(manifest)
        with self.assertRaises(ManifestError):
            validate_manifest_document(manifest)

    def test_checked_manifest_is_valid(self) -> None:
        manifest = checked_manifest()
        self.assertEqual(manifest["release"]["asset_count"], 27)
        self.assertEqual(len(manifest["assets"]), 27)

    def test_manifest_rejects_schema_and_top_level_shape(self) -> None:
        self.assert_manifest_rejected(lambda value: value.update(schema="v0"))
        self.assert_manifest_rejected(lambda value: value.update(unexpected=True))
        self.assert_manifest_rejected(lambda value: value.pop("assets"))

    def test_manifest_rejects_release_identity_mutations(self) -> None:
        for field, value in (
            ("repository", "someone/else"),
            ("tag", "latest"),
            ("url", "https://example.invalid/release"),
            ("published_at", "2026-08-02T14:27:17Z"),
            ("source_commit", "0" * 40),
            ("asset_count", 26),
            ("is_draft", True),
            ("is_prerelease", True),
        ):
            with self.subTest(field=field):
                self.assert_manifest_rejected(
                    lambda manifest, field=field, value=value: manifest["release"].update(
                        {field: value}
                    )
                )

    def test_manifest_rejects_asset_count_and_asset_set_mutations(self) -> None:
        self.assert_manifest_rejected(lambda value: value["assets"].pop())
        self.assert_manifest_rejected(
            lambda value: value["assets"].append(copy.deepcopy(value["assets"][0]))
        )
        self.assert_manifest_rejected(
            lambda value: value["assets"][0].update(name="octessera-v0.7.5-prefix.img.xz")
        )
        self.assert_manifest_rejected(
            lambda value: value["assets"][1].update(name=value["assets"][0]["name"])
        )
        self.assert_manifest_rejected(
            lambda value: value["assets"][1].update(node_id=value["assets"][0]["node_id"])
        )

    def test_manifest_rejects_malformed_asset_fields(self) -> None:
        mutations = (
            lambda value: value["assets"][0].update(node_id="not-a-node-id"),
            lambda value: value["assets"][0].update(size=-1),
            lambda value: value["assets"][0].update(size=True),
            lambda value: value["assets"][0].update(sha256="not-a-sha256"),
            lambda value: value["assets"][0].update(content_type="not a type"),
            lambda value: value["assets"][0].update(artifact_class="unsupported"),
            lambda value: value["assets"][0].update(extra=True),
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                self.assert_manifest_rejected(mutation)

    def test_manifest_rejects_wrong_board_classification(self) -> None:
        self.assert_manifest_rejected(
            lambda value: value["image_parents"][0].update(artifact_class="derived-image")
        )
        self.assert_manifest_rejected(
            lambda value: value["image_parents"][0].update(asset="octessera-latest.img.xz")
        )
        self.assert_manifest_rejected(
            lambda value: value["image_parents"][0]["proof_companion_assets"].append(
                "cached-proof.txt"
            )
        )

    def test_manifest_rejects_duplicate_json_keys(self) -> None:
        with self.assertRaises(ManifestError):
            parse_json_text('{"schema":"one","schema":"two"}', "fixture")

    def test_release_fixtures_match_all_anchors(self) -> None:
        manifest = checked_manifest()
        validate_release_document(gh_release_fixture(manifest), manifest)
        validate_release_document(api_release_fixture(manifest), manifest)

    def test_release_fixture_rejects_release_anchor_mutations(self) -> None:
        mutations = (
            lambda value: value.update(tagName="latest"),
            lambda value: value.update(url="https://example.invalid/release"),
            lambda value: value.update(publishedAt="2026-08-02T14:27:17Z"),
            lambda value: value.update(repository="someone/else"),
            lambda value: value.update(sourceCommit="0" * 40),
            lambda value: value.update(isDraft=True),
            lambda value: value.update(isPrerelease=True),
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                manifest = checked_manifest()
                release = gh_release_fixture(manifest)
                mutation(release)
                with self.assertRaises(ManifestError):
                    validate_release_document(release, manifest)

    def test_release_fixture_requires_draft_and_prerelease_state(self) -> None:
        manifest = checked_manifest()
        for field in ("isDraft", "isPrerelease"):
            release = gh_release_fixture(manifest)
            release.pop(field)
            with self.subTest(field=field), self.assertRaises(ManifestError):
                validate_release_document(release, manifest)

    def test_release_fixture_rejects_every_asset_anchor_mutation(self) -> None:
        for field, value in (
            ("id", "RA_invalid"),
            ("size", 1),
            ("digest", "sha256:" + "0" * 64),
            ("contentType", "application/octet-stream"),
        ):
            with self.subTest(field=field):
                manifest = checked_manifest()
                release = gh_release_fixture(manifest)
                release["assets"][0][field] = value
                with self.assertRaises(ManifestError):
                    validate_release_document(release, manifest)

    def test_release_fixture_rejects_missing_extra_duplicate_and_bad_shape(self) -> None:
        manifest = checked_manifest()
        for mutation in (
            lambda value: value["assets"].pop(),
            lambda value: value["assets"].append({"name": "extra"}),
            lambda value: value["assets"].__setitem__(1, value["assets"][0]),
            lambda value: value.update(assets="not-an-array"),
        ):
            with self.subTest(mutation=mutation):
                release = gh_release_fixture(manifest)
                mutation(release)
                with self.assertRaises(ManifestError):
                    validate_release_document(release, manifest)

    def test_downloaded_directory_requires_exact_board_set_and_hashes(self) -> None:
        manifest = directory_fixture(checked_manifest(), "orange-pi-zero-2w")
        names = {
            manifest_asset["name"]: manifest_asset
            for manifest_asset in manifest["assets"]
            if manifest_asset["name"]
            in {
                "octessera-0.7.5-orange-pi-zero-2w.img.xz",
                "octessera-0.7.5-orange-pi-zero-2w.img.xz.sha256",
                "linux-image-current-sunxi64_26.8.0-trunk.417_arm64.deb",
                "linux-dtb-current-sunxi64_26.8.0-trunk.417_arm64.deb",
                "octessera-orange-kernel-evidence.env",
                "octessera-orange-kernel-provenance.txt",
                "octessera-orange-image-provenance.txt",
                "SHA256SUMS-orange-pi-zero-2w.txt",
            }
        }
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            for asset in names.values():
                (directory / asset["name"]).write_bytes(asset["name"].encode("utf-8"))
            validate_downloaded_directory(directory, manifest, ("orange-pi-zero-2w",))
            target = next(iter(names))
            expected_size = names[target]["size"]
            (directory / target).write_bytes(b"x" * expected_size)
            with self.assertRaises(ManifestError):
                validate_downloaded_directory(directory, manifest, ("orange-pi-zero-2w",))
            (directory / target).write_bytes(b"changed")
            with self.assertRaises(ManifestError):
                validate_downloaded_directory(directory, manifest, ("orange-pi-zero-2w",))

    def test_downloaded_directory_rejects_missing_extra_and_bad_board(self) -> None:
        manifest = directory_fixture(checked_manifest(), "orange-pi-zero-2w")
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            with self.assertRaises(ManifestError):
                validate_downloaded_directory(directory, manifest, ("orange-pi-zero-2w",))
            (directory / "extra").write_bytes(b"extra")
            with self.assertRaises(ManifestError):
                validate_downloaded_directory(directory, manifest, ("orange-pi-zero-2w",))
        with self.assertRaises(ManifestError):
            validate_downloaded_directory(Path(temporary), manifest, ("unknown",))

    def test_verifier_cli_validates_manifest_release_stdin_and_directory(self) -> None:
        commands = (
            (["--validate-manifest"], None),
            (["--release-json", "-"], json.dumps(gh_release_fixture(checked_manifest()))),
        )
        for arguments, input_text in commands:
            result = subprocess.run(
                [sys.executable, str(VERIFIER_PATH), *arguments],
                cwd=ROOT,
                input=input_text,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
        for board in ("orange-pi-zero-2w", "raspberry-pi-zero-2w"):
            result = subprocess.run(
                [
                    sys.executable,
                    str(VERIFIER_PATH),
                    "--print-board-assets",
                    "--board",
                    board,
                ],
                cwd=ROOT,
                text=True,
                capture_output=True,
                check=False,
            )
            expected_parent = next(
                parent for parent in checked_manifest()["image_parents"] if parent["board"] == board
            )
            self.assertEqual(
                result.stdout.splitlines(),
                [expected_parent["asset"], *expected_parent["proof_companion_assets"]],
            )
            self.assertEqual(result.returncode, 0, result.stderr)
        manifest = directory_fixture(checked_manifest(), "orange-pi-zero-2w")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory = root / "downloaded"
            directory.mkdir()
            for parent in manifest["image_parents"]:
                if parent["board"] != "orange-pi-zero-2w":
                    continue
                for name in (parent["asset"], *parent["proof_companion_assets"]):
                    (directory / name).write_bytes(name.encode("utf-8"))
            manifest_file = root / "fixture-manifest.json"
            manifest_file.write_text(json.dumps(manifest), encoding="utf-8")
            result = subprocess.run(
                [
                    sys.executable,
                    str(VERIFIER_PATH),
                    "--manifest",
                    str(manifest_file),
                    "--directory",
                    str(directory),
                    "--board",
                    "orange-pi-zero-2w",
                ],
                cwd=ROOT,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            release_file = root / "fixture-release.json"
            release_file.write_text(json.dumps(gh_release_fixture(checked_manifest())), encoding="utf-8")
            result = subprocess.run(
                [
                    sys.executable,
                    str(VERIFIER_PATH),
                    "--release-json",
                    str(release_file),
                ],
                cwd=ROOT,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
