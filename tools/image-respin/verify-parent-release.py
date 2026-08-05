import argparse
import sys
from pathlib import Path

from trust_manifest import (
    ManifestError,
    load_json_file,
    load_manifest,
    parse_json_text,
    validate_downloaded_directory,
    validate_release_document,
)


DEFAULT_MANIFEST = (
    Path(__file__).resolve().parents[2]
    / "resources"
    / "image-parents"
    / "v0.7.5-trust-manifest.json"
)


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate the exact v0.7.5 image-parent trust lane.")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--validate-manifest", action="store_true")
    mode.add_argument("--directory", type=Path)
    mode.add_argument("--release-json", metavar="PATH")
    mode.add_argument("--print-board-assets", action="store_true")
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument(
        "--board",
        action="append",
        choices=("orange-pi-zero-2w", "raspberry-pi-zero-2w"),
        help="select one exact board asset set",
    )
    return parser.parse_args()


def _release_json_document(path: str):
    if path == "-":
        return parse_json_text(sys.stdin.read(), "stdin")
    return load_json_file(Path(path))


def main() -> int:
    arguments = _arguments()
    try:
        manifest = load_manifest(arguments.manifest)
        if arguments.validate_manifest:
            print(f"manifest valid: {arguments.manifest}")
        elif arguments.print_board_assets:
            if arguments.board is None or len(arguments.board) != 1:
                raise ManifestError("--print-board-assets requires exactly one --board")
            parent = next(
                parent
                for parent in manifest["image_parents"]
                if parent["board"] == arguments.board[0]
            )
            print(parent["asset"])
            print(*parent["proof_companion_assets"], sep="\n")
        elif arguments.directory is not None:
            boards = tuple(arguments.board) if arguments.board else None
            validate_downloaded_directory(arguments.directory, manifest, boards)
            print(f"downloaded directory valid: {arguments.directory}")
        else:
            validate_release_document(_release_json_document(arguments.release_json), manifest)
            print("release JSON valid: v0.7.5")
        return 0
    except (ManifestError, OSError) as error:
        print(f"verification failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
