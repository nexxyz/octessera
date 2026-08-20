from __future__ import annotations

import argparse
import json
from pathlib import Path

try:
    from .post_proof_record import build_record as build_post_record, validate_record as validate_post_record
    from .requested_build_record import build_record as build_requested_record, validate_record as validate_requested_record
    from .setup_workflow_record import build_record as build_setup_post_record, validate_record as validate_setup_post_record
    from .record_documents import load_json, write_new
    from .record_validation import RecordError, require
except ImportError:
    from post_proof_record import build_record as build_post_record, validate_record as validate_post_record
    from requested_build_record import build_record as build_requested_record, validate_record as validate_requested_record
    from setup_workflow_record import build_record as build_setup_post_record, validate_record as validate_setup_post_record
    from record_documents import load_json, write_new
    from record_validation import RecordError, require


def _assignment(value: str, label: str) -> tuple[str, str]:
    key, separator, item = value.partition("=")
    require(bool(key and separator and item), f"{label} assignment is malformed")
    return key, item


def _map(values: list[str], label: str) -> dict[str, str]:
    result: dict[str, str] = {}
    for value in values:
        key, item = _assignment(value, label)
        require(key not in result, f"duplicate {label}: {key}")
        result[key] = item
    return result


def _text(path: Path, label: str) -> str:
    try:
        value = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        raise RecordError(f"cannot read {label}: {path}") from error
    require(len(value.strip()) > 0, f"{label} is empty: {path}")
    return value.rstrip("\n")


def _requested_parser(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--board", required=True)
    parser.add_argument("--feature-command", required=True)
    parser.add_argument("--input-file", action="append", required=True, type=Path)
    parser.add_argument("--trust-manifest", required=True, type=Path)
    parser.add_argument("--rustc-version-file", required=True, type=Path)
    parser.add_argument("--cargo-version-file", required=True, type=Path)
    parser.add_argument("--cross-version-file", required=True, type=Path)
    parser.add_argument("--container-rustc-version-file", required=True, type=Path)
    parser.add_argument("--container-cargo-version-file", required=True, type=Path)
    parser.add_argument("--cross-image-id", required=True)
    parser.add_argument("--cross-image-repo-digests", required=True)
    parser.add_argument("--base-image-id", required=True)
    parser.add_argument("--base-image-repo-digests", required=True)
    parser.add_argument("--proof-package", action="append", required=True)
    parser.add_argument("--setup-layer", choices=("setup-portal",), default=None)
    parser.add_argument("--setup-contract", type=Path)
    parser.add_argument("--setup-input-file", action="append", type=Path, default=[])
    parser.add_argument("--strict-setup-source-tracking", "--require-setup-sources-tracked", dest="require_setup_sources_tracked", action="store_true")
    parser.add_argument("--output", required=True, type=Path)


def _post_parser(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--requested-build", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--board", required=True)
    parser.add_argument("--runtime-bundle", required=True, type=Path)
    parser.add_argument("--artifact", required=True, type=Path)
    parser.add_argument("--respin-provenance", required=True, type=Path)
    parser.add_argument("--proof-output", action="append", required=True)
    parser.add_argument("--proof-template", action="append", required=True)
    parser.add_argument("--companion", action="append", required=True, type=Path)
    parser.add_argument("--workflow", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)


def _setup_post_parser(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--requested-build", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--board", required=True)
    parser.add_argument("--runtime-bundle", required=True, type=Path)
    parser.add_argument("--artifact", required=True, type=Path)
    parser.add_argument("--respin-provenance", required=True, type=Path)
    parser.add_argument("--setup-proof", required=True, type=Path)
    parser.add_argument("--production-proof", action="append", required=True)
    parser.add_argument("--companion", action="append", required=True, type=Path)
    parser.add_argument("--workflow", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate or verify Octessera respin workflow records.")
    modes = parser.add_subparsers(dest="mode", required=True)
    requested = modes.add_parser("requested")
    _requested_parser(requested)
    post = modes.add_parser("post-proof")
    _post_parser(post)
    setup_post = modes.add_parser("setup-post-proof")
    _setup_post_parser(setup_post)
    verify = modes.add_parser("verify-post")
    verify.add_argument("--root", type=Path, default=Path("."))
    verify.add_argument("--record", required=True, type=Path)
    verify_setup = modes.add_parser("verify-post-setup")
    verify_setup.add_argument("--root", type=Path, default=Path("."))
    verify_setup.add_argument("--record", required=True, type=Path)
    arguments = parser.parse_args()
    try:
        if arguments.mode == "requested":
            packages = _map(arguments.proof_package, "proof package")
            record = build_requested_record(
                root=arguments.root,
                source_sha=arguments.source_sha,
                version=arguments.version,
                board=arguments.board,
                feature_command=arguments.feature_command,
                input_files=arguments.input_file,
                trust_manifest=arguments.trust_manifest,
                rustc_vv=_text(arguments.rustc_version_file, "rustc version"),
                cargo_version=_text(arguments.cargo_version_file, "cargo version"),
                cross_version=_text(arguments.cross_version_file, "cross version"),
                container_rustc_vv=_text(arguments.container_rustc_version_file, "container rustc version"),
                container_cargo_version=_text(arguments.container_cargo_version_file, "container cargo version"),
                cross_image_id=arguments.cross_image_id,
                cross_repo_digests=json.loads(arguments.cross_image_repo_digests),
                base_image_id=arguments.base_image_id,
                base_repo_digests=json.loads(arguments.base_image_repo_digests),
                proof_packages=packages,
                setup_layer=arguments.setup_layer,
                setup_contract=arguments.setup_contract,
                setup_input_files=arguments.setup_input_file,
                require_setup_sources_tracked=arguments.require_setup_sources_tracked,
            )
            write_new(arguments.output, record)
        elif arguments.mode == "post-proof":
            record = build_post_record(
                root=arguments.root,
                requested_build=arguments.requested_build,
                manifest=arguments.manifest,
                board=arguments.board,
                runtime_bundle=arguments.runtime_bundle,
                artifact=arguments.artifact,
                respin_provenance=arguments.respin_provenance,
                proof_outputs={key: Path(value) for key, value in (_assignment(item, "proof output") for item in arguments.proof_output)},
                template_ids=_map(arguments.proof_template, "proof template"),
                companions=arguments.companion,
                workflow=arguments.workflow,
            )
            write_new(arguments.output, record)
            validate_post_record(record, arguments.root)
        elif arguments.mode == "setup-post-proof":
            record = build_setup_post_record(root=arguments.root, requested_build=arguments.requested_build, manifest=arguments.manifest, board=arguments.board, runtime_bundle=arguments.runtime_bundle, artifact=arguments.artifact, respin_provenance=arguments.respin_provenance, setup_proof=arguments.setup_proof, production_proofs={key: Path(value) for key, value in (_assignment(item, "production proof") for item in arguments.production_proof)}, companions=arguments.companion, workflow=arguments.workflow)
            write_new(arguments.output, record)
            validate_setup_post_record(record, arguments.root)
        elif arguments.mode == "verify-post":
            validate_post_record(load_json(arguments.record), arguments.root)
        else:
            validate_setup_post_record(load_json(arguments.record), arguments.root)
        return 0
    except (OSError, RecordError, ValueError) as error:
        print(f"workflow record rejected: {error}")
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
