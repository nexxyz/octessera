from __future__ import annotations

import argparse
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "tools" / "legal"))

from package_notice_zip import notice_payload  # type: ignore[import-not-found]
from stage_notices import NoticeStageError  # type: ignore[import-not-found]
from tools.samples.sample_library import SampleLibraryError, verify_media_tree


class DesktopArtifactError(ValueError):
    pass


def _verify_legal_tree(repository_root: Path, legal_root: Path) -> None:
    expected = {name.removeprefix("legal/"): data for name, data in notice_payload(repository_root).items()}
    if not legal_root.is_dir():
        raise DesktopArtifactError(f"legal resource tree is missing: {legal_root}")
    actual = {path.relative_to(legal_root).as_posix() for path in legal_root.rglob("*") if path.is_file()}
    if actual != set(expected):
        raise DesktopArtifactError(f"legal resource tree has missing or extra files: {sorted(actual ^ set(expected))}")
    for relative, data in expected.items():
        if (legal_root / relative).read_bytes() != data:
            raise DesktopArtifactError(f"legal resource file is stale: {relative}")


def verify_resource_layout(repository_root: Path, resource_root: Path, samples_relative: str = "samples", legal_relative: str = "legal") -> None:
    """Checks an installed desktop bundle's resources: exactly the shipped samples and legal files."""
    repository_root = repository_root.resolve()
    try:
        verify_media_tree(resource_root / samples_relative, repository_root / "samples")
    except SampleLibraryError as error:
        raise DesktopArtifactError(str(error)) from error
    _verify_legal_tree(repository_root, resource_root / legal_relative)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Verify Octessera desktop sample and legal resources.")
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--resource-root", type=Path, required=True)
    parser.add_argument("--samples-relative", default="samples")
    parser.add_argument("--legal-relative", default="legal")
    args = parser.parse_args(argv)
    try:
        verify_resource_layout(args.repository_root, args.resource_root, args.samples_relative, args.legal_relative)
    except (DesktopArtifactError, NoticeStageError, OSError) as error:
        print(f"desktop artifact verification failed: {error}", file=sys.stderr)
        return 1
    print("Desktop artifact resources verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
