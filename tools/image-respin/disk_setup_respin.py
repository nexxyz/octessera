from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any, Callable

try:
    from .boot_neutral import BootNeutralError, build_integrity, capture_state, load_policy, parent_binding
    from .disk_layout import DiskLayoutError, assert_no_drift
    from .disk_mount import DiskMountError, mounted_runtime, require_linux_root
    from .disk_packaging import DiskPackagingError, compression_identity, package_derived, prepare_parent_image, provenance_sidecar, verify_current_parent_asset, write_derived_sidecar
    from .disk_provenance import build_derived_provenance
    from .provenance import TOOL_IDENTITY, digest_object
    from .runtime_mutation import MutationResult, mutate_runtime
    from .setup_mutation import SetupMutationResult, mutate_setup
    from .setup_proof import prove_setup_root
except ImportError:
    from boot_neutral import BootNeutralError, build_integrity, capture_state, load_policy, parent_binding
    from disk_layout import DiskLayoutError, assert_no_drift
    from disk_mount import DiskMountError, mounted_runtime, require_linux_root
    from disk_packaging import DiskPackagingError, compression_identity, package_derived, prepare_parent_image, provenance_sidecar, verify_current_parent_asset, write_derived_sidecar
    from disk_provenance import build_derived_provenance
    from provenance import TOOL_IDENTITY, digest_object
    from runtime_mutation import MutationResult, mutate_runtime
    from setup_mutation import SetupMutationResult, mutate_setup
    from setup_proof import prove_setup_root


class DiskSetupRespinError(RuntimeError):
    pass


def _output_name(board: str, version: str, suffix: str) -> str:
    return f"octessera-{version}-{board}-derived-setup-respin{suffix}"


def _setup_provenance(runtime: MutationResult, setup: SetupMutationResult, proof: dict[str, Any], image: Path, packaged: Path, pre_layout: Any, post_layout: Any, parent_context: dict[str, Any], parent_record_digest: str, source_identity: object, version: str, boot_integrity: dict[str, Any] | None = None, boot_policy: dict[str, Any] | None = None, boot_parent: dict[str, Any] | None = None) -> dict[str, Any]:
    if boot_integrity is None or boot_policy is None or boot_parent is None:
        raise DiskSetupRespinError("Orange boot-neutral provenance context is incomplete")
    return build_derived_provenance(board_profile=setup.board_profile, version=version, source_identity=source_identity, parent_context=parent_context, parent_record_digest=parent_record_digest, runtime_provenance=runtime.provenance, pre_layout=pre_layout, post_layout=post_layout, image=image, packaged=packaged, compression_identity=compression_identity(setup.board_profile), tool_identity=TOOL_IDENTITY, boot_integrity=boot_integrity, boot_policy=boot_policy, parent_binding=boot_parent, derivation_kind="setup-portal", setup_mutation={"digest": digest_object(setup.provenance), "provenance": setup.provenance}, setup_proof=proof)


def respin_setup_image(*, board_profile: str, assets_directory: Path, parent_record_path: Path, runtime_bundle: Path, version: str, source_identity: object, output: Path, proof_output: Path, boot_neutral_contract: Path | None = None, mutation_hook: Callable[[str], None] | None = None) -> dict[str, Any]:
    require_linux_root()
    if board_profile != "orange-pi-zero-2w":
        raise DiskSetupRespinError(f"no current parent record exists for {board_profile}")
    suffix = ".img.xz"
    output = Path(output).absolute()
    proof_output = Path(proof_output).absolute()
    policy = load_policy(Path(__file__).resolve().parents[2], boot_neutral_contract)
    if output.name != _output_name(board_profile, version, suffix) or output.exists() or output.is_symlink() or proof_output.exists() or proof_output.is_symlink():
        raise DiskSetupRespinError("setup derived output or proof already exists")
    try:
        source, parent_context, parent_record_digest, imager_manifest = verify_current_parent_asset(assets_directory, parent_record_path, board_profile)
        prepared = prepare_parent_image(source, parent_context, parent_record_digest, board_profile, imager_manifest)
    except (DiskPackagingError, OSError) as exc:
        raise DiskSetupRespinError(str(exc)) from exc
    package: Path | None = None
    boot_before: dict[str, Any] | None = None
    boot_after: dict[str, Any] | None = None
    try:
        with mounted_runtime(prepared.image, board_profile) as mounted:
            if mounted.pre_layout is None or mounted.root_mount is None:
                raise DiskSetupRespinError("mounted setup respin did not expose a root layout")
            boot_before = capture_state(policy, mounted.root_mount, mounted.pre_layout)
            runtime = mutate_runtime(mounted.root_mount, runtime_bundle, board_profile, version, source_identity, parent_context, mutation_hook=mutation_hook)
            setup = mutate_setup(mounted.root_mount, board_profile, source_identity, mutation_hook=mutation_hook)
            proof = prove_setup_root(mounted.root_mount, board_profile)
            boot_after = capture_state(policy, mounted.root_mount, mounted.pre_layout)
        if mounted.post_layout is None:
            raise DiskSetupRespinError("mounted setup respin did not capture a post-layout invariant")
        assert_no_drift(mounted.pre_layout, mounted.post_layout)
        boot_integrity = None
        boot_policy = None
        boot_parent = None
        if boot_before is not None and boot_after is not None:
            boot_integrity = build_integrity(policy, boot_before, boot_after, mounted.post_layout)
            boot_policy = {"proof_mode": policy.proof_mode, "policy": policy.policy}
            boot_parent = parent_binding(policy, parent_record_path, parent_record_digest, parent_context)
        prepared.verify_source_unchanged()
        proof_output.parent.mkdir(parents=True, exist_ok=True)
        proof_output.write_text(json.dumps(proof, sort_keys=True, indent=2) + "\n", encoding="utf-8")
        package = package_derived(prepared.image, output, board_profile, version, "setup")
        provenance = _setup_provenance(runtime, setup, proof, prepared.image, package, mounted.pre_layout, mounted.post_layout, parent_context, parent_record_digest, source_identity, version, boot_integrity, boot_policy, boot_parent)
        write_derived_sidecar(output, provenance)
        return provenance
    except Exception as exc:
        output.unlink(missing_ok=True)
        provenance_sidecar(output).unlink(missing_ok=True)
        proof_output.unlink(missing_ok=True)
        raise DiskSetupRespinError(str(exc)) from exc
    finally:
        try:
            prepared.close()
        except OSError as exc:
            raise DiskSetupRespinError(f"cannot clean private setup respin workspace: {exc}") from exc


def main() -> int:
    parser = argparse.ArgumentParser(description="Respin a trusted board image with the separate setup portal layer")
    parser.add_argument("--board", required=True)
    parser.add_argument("--assets-directory", type=Path, required=True)
    parser.add_argument("--parent-record", type=Path, required=True)
    parser.add_argument("--runtime-bundle", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--source-identity", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--proof-output", type=Path, required=True)
    parser.add_argument("--boot-neutral-contract", type=Path)
    args = parser.parse_args()
    result = respin_setup_image(board_profile=args.board, assets_directory=args.assets_directory, parent_record_path=args.parent_record, runtime_bundle=args.runtime_bundle, version=args.version, source_identity=args.source_identity, output=args.output, proof_output=args.proof_output, boot_neutral_contract=args.boot_neutral_contract)
    print(json.dumps(result, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (DiskSetupRespinError, DiskMountError, DiskLayoutError, DiskPackagingError, BootNeutralError) as exc:
        print(f"disk setup respin rejected: {exc}")
        raise SystemExit(2) from exc
