from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
from pathlib import Path

from test_orange_image_proof_support import (
    CANONICAL_DTB,
    CANONICAL_IMAGE,
    make_missing_builtin_fixture,
    replace_option,
    run_proof,
    run_proof_failure,
    verifier_args,
)


def run_image_proof(work: Path, fixture: tuple[Path, Path, Path, Path, Path]) -> None:
    root, image, dtb, evidence, provenance = fixture
    args = verifier_args(root, image, dtb, evidence, provenance)
    run_proof(args, True)
    negative_root, negative_image, negative_evidence, negative_provenance = make_missing_builtin_fixture(
        work, root, image, evidence, provenance
    )
    negative_args = args
    for option, value in (
        ("--root", negative_root),
        ("--linux-image", negative_image),
        ("--evidence", negative_evidence),
        ("--provenance", negative_provenance),
    ):
        negative_args = replace_option(negative_args, option, value)
    negative_result = subprocess.run(negative_args, capture_output=True, text=True)
    if negative_result.returncode == 0 or "CONFIG_SPI_SPIDEV=y" not in negative_result.stderr:
        raise AssertionError(negative_result.stdout + negative_result.stderr)

    artifact = work / "image-provenance.txt"
    run_proof([*args, "--output", str(artifact)], True)
    proof_document = json.loads(artifact.read_text())
    assert proof_document["schema"] == "octessera.image-proof/v2"
    assert proof_document["schema_version"] == 2
    assert proof_document["proof_mode"] == "phase5-constructor"
    tampered_artifact = work / "tampered-image-provenance.txt"
    tampered = json.loads(artifact.read_text())
    tampered["artifact"]["sha256"] = "b" * 64
    tampered_artifact.write_text(json.dumps(tampered) + "\n")
    run_proof([*args, "--image-provenance", str(tampered_artifact)], False)
    canonical_image = work / CANONICAL_IMAGE
    canonical_dtb = work / CANONICAL_DTB
    shutil.copy2(image, canonical_image)
    shutil.copy2(dtb, canonical_dtb)
    run_proof(verifier_args(root, canonical_image, canonical_dtb, evidence, provenance), True)

    wrong_suffix = "self-consistent-wrong-suffix"
    wrong_image = work / f"{CANONICAL_IMAGE.removesuffix('.deb')}__{wrong_suffix}.deb"
    wrong_dtb = work / f"{CANONICAL_DTB.removesuffix('.deb')}__{wrong_suffix}.deb"
    shutil.copy2(image, wrong_image)
    shutil.copy2(dtb, wrong_dtb)
    wrong_evidence = work / "wrong-suffix-evidence.env"
    evidence_values = {}
    for line in evidence.read_text().splitlines():
        key, _, value = line.partition("=")
        evidence_values[key] = {"image_package_native_basename": wrong_image.name, "dtb_package_native_basename": wrong_dtb.name, "artifact_suffix": wrong_suffix}.get(key, value)
    wrong_evidence.write_text("\n".join(f"{key}={value}" for key, value in evidence_values.items()) + "\n")
    wrong_provenance = work / "wrong-suffix-provenance.txt"
    provenance_values = {}
    for line in provenance.read_text().splitlines():
        key, _, value = line.partition("=")
        provenance_values[key] = {"image_package_native": wrong_image.name, "dtb_package_native": wrong_dtb.name, "artifact_suffix": wrong_suffix, "evidence_sha256": hashlib.sha256(wrong_evidence.read_bytes()).hexdigest()}.get(key, value)
    wrong_provenance.write_text("\n".join(f"{key}={value}" for key, value in provenance_values.items()) + "\n")
    run_proof_failure(verifier_args(root, wrong_image, wrong_dtb, wrong_evidence, wrong_provenance), "native package suffix evidence is not manifest-approved")
