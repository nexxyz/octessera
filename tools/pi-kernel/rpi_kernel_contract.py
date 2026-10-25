from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path


RASPBERRY_PATCH_ROOT = "tools/pi-kernel/patches"


class ContractError(ValueError):
    pass


@dataclass(frozen=True)
class Contract:
    root: Path
    manifest_path: Path
    patch_paths: tuple[Path, ...]
    source_repository: str
    source_commit: str
    kernel_release: str
    localversion: str
    package_name: str
    package_version: str
    package_architecture: str
    required_modules: tuple[str, ...]

    @property
    def package_filename(self) -> str:
        return f"{self.package_name}_{self.package_version}_{self.package_architecture}.deb"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def load_contract(root: Path, manifest_path: Path | None = None) -> Contract:
    root = root.resolve()
    path = (manifest_path or root / "tools/kernel-patches/orange-midi-interface-manifest.json").resolve()
    try:
        manifest = json.loads(path.read_text(encoding="utf-8"))
        raspberry = manifest["kernels"]["raspberry"]
        patch_root = root / manifest["patch_root"]
        patch_paths = tuple(patch_root / name for name in manifest["patch_order"])
        patch_paths += tuple(sorted((root / RASPBERRY_PATCH_ROOT).glob("*.patch")))
        return Contract(
            root=root,
            manifest_path=path,
            patch_paths=patch_paths,
            source_repository=raspberry["repository"],
            source_commit=raspberry["commit"],
            kernel_release=raspberry["kernel_release"],
            localversion=raspberry["localversion"],
            package_name=raspberry["package"]["name"],
            package_version=raspberry["package"]["version"],
            package_architecture=raspberry["package"]["architecture"],
            required_modules=tuple(raspberry["required_modules"]),
        )
    except (OSError, KeyError, TypeError, json.JSONDecodeError) as error:
        raise ContractError(f"cannot load kernel manifest {path}: {error}") from error
