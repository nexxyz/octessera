from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

try:
    from .boot_neutral import BootNeutralError, build_integrity, capture_state, load_policy, parent_binding
    from .disk_layout import DiskLayoutError, assert_no_drift
    from .disk_mount import DiskMountError, mounted_runtime, require_linux_root
    from .disk_packaging import DiskPackagingError, compression_identity, package_derived, provenance_sidecar, verify_parent_asset, write_derived_sidecar, prepare_parent_image
    from .disk_provenance import build_derived_provenance
    from .provenance import TOOL_IDENTITY
    from .runtime_mutation import MutationResult, mutate_runtime
except ImportError:
    from boot_neutral import BootNeutralError, build_integrity, capture_state, load_policy, parent_binding
    from disk_layout import DiskLayoutError, assert_no_drift
    from disk_mount import DiskMountError, mounted_runtime, require_linux_root
    from disk_packaging import DiskPackagingError, compression_identity, package_derived, provenance_sidecar, verify_parent_asset, write_derived_sidecar, prepare_parent_image
    from disk_provenance import build_derived_provenance
    from provenance import TOOL_IDENTITY
    from runtime_mutation import MutationResult, mutate_runtime


class DiskRespinError(RuntimeError):
    pass


@dataclass(frozen=True)
class DiskRespinResult:
    board_profile: str
    output: Path
    provenance_output: Path
    provenance: dict[str, Any]
    runtime_result: MutationResult


def _require_derived_output(output: Path, board: str, version: str) -> None:
    suffix = ".img.xz" if board == "orange-pi-zero-2w" else ".zip"
    expected = f"octessera-{version}-{board}-derived-runtime-respin{suffix}"
    if output.name != expected:
        raise DiskRespinError(f"output must be the exact derived board-qualified name: {expected}")


def respin_image(
    *,
    board_profile: str,
    assets_directory: Path,
    manifest_path: Path,
    runtime_bundle: Path,
    version: str,
    source_identity: object,
    output: Path,
    boot_neutral_contract: Path | None = None,
    mutation_hook: Callable[[str], None] | None = None,
) -> DiskRespinResult:
    require_linux_root()
    _require_derived_output(Path(output), board_profile, version)
    output = Path(output).absolute()
    policy = None
    if board_profile == "orange-pi-zero-2w":
        policy = load_policy(Path(__file__).resolve().parents[2], boot_neutral_contract)
    elif boot_neutral_contract is not None:
        raise DiskRespinError("boot-neutral policy is Orange-only")
    default_provenance_path = provenance_sidecar(output)
    provenance_path = default_provenance_path
    if output == provenance_path or output.exists() or output.is_symlink() or provenance_path.exists() or provenance_path.is_symlink():
        raise DiskRespinError("derived output or provenance already exists")
    try:
        source, parent_context, manifest_digest, imager_manifest = verify_parent_asset(assets_directory, manifest_path, board_profile)
        prepared = prepare_parent_image(source, parent_context, manifest_digest, board_profile, imager_manifest)
    except (DiskPackagingError, OSError) as exc:
        raise DiskRespinError(str(exc)) from exc
    runtime_result: MutationResult | None = None
    package: Path | None = None
    retain_workspace = False
    retained_path: Path | None = None
    boot_before: dict[str, Any] | None = None
    boot_after: dict[str, Any] | None = None
    try:
        with mounted_runtime(prepared.image, board_profile) as mounted:
            if mounted.pre_layout is None or mounted.root_mount is None:
                raise DiskRespinError("mounted runtime did not expose a root layout")
            if policy is not None:
                boot_before = capture_state(policy, mounted.root_mount, mounted.pre_layout)
            runtime_result = mutate_runtime(mounted.root_mount, runtime_bundle, board_profile, version, source_identity, parent_context, mutation_hook=mutation_hook)
            if policy is not None:
                boot_after = capture_state(policy, mounted.root_mount, mounted.pre_layout)
        if mounted.post_layout is None:
            raise DiskRespinError("mounted runtime did not capture a post-layout invariant")
        assert_no_drift(mounted.pre_layout, mounted.post_layout)
        boot_integrity = None
        boot_policy = None
        boot_parent = None
        if policy is not None and boot_before is not None and boot_after is not None:
            boot_integrity = build_integrity(policy, boot_before, boot_after, mounted.post_layout)
            boot_policy = {"proof_mode": policy.proof_mode, "policy": policy.policy}
            boot_parent = parent_binding(policy, manifest_path, manifest_digest, parent_context)
        prepared.verify_source_unchanged()
        package = package_derived(prepared.image, output, board_profile, version)
        provenance = build_derived_provenance(board_profile=board_profile, version=version, source_identity=source_identity, parent_context=parent_context, trust_manifest_digest=manifest_digest, runtime_provenance=runtime_result.provenance, pre_layout=mounted.pre_layout, post_layout=mounted.post_layout, image=prepared.image, packaged=package, compression_identity=compression_identity(board_profile), tool_identity=TOOL_IDENTITY, boot_integrity=boot_integrity, boot_policy=boot_policy, parent_binding=boot_parent, derivation_kind="runtime-only" if policy is not None else None)
        write_derived_sidecar(output, provenance)
        return DiskRespinResult(board_profile, package, provenance_path, provenance, runtime_result)
    except Exception as exc:
        if isinstance(exc, DiskMountError):
            retain_workspace = exc.retain_workspace
            retained_path = exc.backing_path
        if package is not None:
            package.unlink(missing_ok=True)
            provenance_path.unlink(missing_ok=True)
            package = None
        if retain_workspace:
            retained_path = retained_path or prepared.work
            message = f"{exc}; private workspace retained at {retained_path}"
        else:
            message = str(exc)
        raise DiskRespinError(message) from exc
    finally:
        if package is None:
            output.unlink(missing_ok=True)
            default_provenance_path.unlink(missing_ok=True)
            provenance_path.unlink(missing_ok=True)
        if not retain_workspace:
            try:
                prepared.close()
            except OSError as exc:
                output.unlink(missing_ok=True)
                provenance_path.unlink(missing_ok=True)
                raise DiskRespinError(f"cannot clean private respin workspace: {exc}") from exc


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Respin a trusted Octessera disk image runtime")
    parser.add_argument("--board", choices=("raspberry-pi-zero-2w", "orange-pi-zero-2w"), required=True)
    parser.add_argument("--assets-directory", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--runtime-bundle", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--source-identity", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--boot-neutral-contract", type=Path)
    return parser.parse_args()


def main() -> int:
    args = _arguments()
    result = respin_image(board_profile=args.board, assets_directory=args.assets_directory, manifest_path=args.manifest, runtime_bundle=args.runtime_bundle, version=args.version, source_identity=args.source_identity, output=args.output, boot_neutral_contract=args.boot_neutral_contract)
    print(json.dumps(result.provenance, sort_keys=True, indent=2))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (DiskRespinError, DiskMountError, DiskLayoutError, DiskPackagingError, BootNeutralError) as exc:
        print(f"disk runtime respin rejected: {exc}")
        raise SystemExit(2) from exc
