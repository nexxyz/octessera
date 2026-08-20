from __future__ import annotations

import shutil
import tempfile
from pathlib import Path

import test_orange_image_proof_boot
import test_orange_image_proof_image
import test_orange_image_proof_runtime
import test_orange_image_proof_security
import test_orange_image_proof_source
from test_orange_image_proof_support import make_fixture


def main() -> None:
    test_orange_image_proof_source.run_source_proof()
    missing_tools = [tool for tool in ("dpkg-deb",) if shutil.which(tool) is None]
    if missing_tools:
        print(f"Orange image proof fixture skipped: missing {', '.join(missing_tools)}")
        return
    with tempfile.TemporaryDirectory(prefix="octessera-orange-proof-fixture-") as temporary:
        work = Path(temporary)
        root, image, dtb, evidence, provenance = make_fixture(work)
        test_orange_image_proof_image.run_image_proof(work, (root, image, dtb, evidence, provenance))
        test_orange_image_proof_boot.run_boot_proof(work, root, image, dtb, evidence, provenance)
        test_orange_image_proof_runtime.run_runtime_proof(work, image, dtb, evidence, provenance)
        test_orange_image_proof_security.run_security_proof(work, root, image, dtb, evidence, provenance)
    print("Orange final image proof synthetic fixtures passed")


if __name__ == "__main__":
    main()
