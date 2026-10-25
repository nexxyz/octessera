#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from rpi_kernel_contract import Contract, ContractError, load_contract, sha256_file

class BuildError(ValueError):
    pass

def _run(command: list[str], *, cwd: Path | None = None, env: dict[str, str] | None = None, capture: bool = False) -> str:
    try:
        result = subprocess.run(command, cwd=cwd, env=env, check=True, capture_output=capture, text=True)
    except FileNotFoundError as error:
        raise BuildError(f"required command is unavailable: {command[0]}") from error
    except subprocess.CalledProcessError as error:
        detail = ((error.stdout or "") + (error.stderr or "")).strip()
        raise BuildError(f"command failed ({error.returncode}): {' '.join(command)}\n{detail}") from error
    return result.stdout if capture else ""

def _prepare_source(contract: Contract, source_arg: Path | None, run_dir: Path) -> Path:
    source = run_dir / "source"
    if source_arg:
        source_arg = source_arg.resolve()
        if not source_arg.is_dir():
            raise BuildError(f"source directory does not exist: {source_arg}")
        _run(["git", "clone", "--no-local", str(source_arg), str(source)])
    else:
        _run(["git", "clone", "--no-checkout", "--filter=blob:none", "--depth=1", contract.source_repository, str(source)])
    if source_arg:
        try:
            _run(["git", "-C", str(source), "cat-file", "-e", f"{contract.source_commit}^{{commit}}"])
        except BuildError:
            _run(["git", "-C", str(source), "fetch", "--depth=1", "origin", contract.source_commit])
    else:
        _run(["git", "-C", str(source), "fetch", "--depth=1", "origin", contract.source_commit])
    _run(["git", "-C", str(source), "checkout", "--detach", contract.source_commit])
    return source

def _apply_patches(contract: Contract, source: Path) -> list[str]:
    applied = []
    for patch in contract.patch_paths:
        _run(["git", "-C", str(source), "apply", "--check", "--whitespace=error", str(patch)])
        _run(["git", "-C", str(source), "apply", "--whitespace=error", str(patch)])
        rejects = list(source.rglob("*.rej")) + list(source.rglob("*.orig"))
        if rejects:
            raise BuildError(f"patch application left rejects or originals: {rejects[0]}")
        applied.append(patch.relative_to(contract.root).as_posix())
    return applied

def _make_environment(
    cross_compile: str,
    package_version: str | None = None,
    *,
    preserve_localversion: bool = False,
) -> dict[str, str]:
    environment = os.environ.copy()
    if not preserve_localversion:
        environment["LOCALVERSION"] = ""
    for key in list(environment):
        if key.startswith("DEB_HOST_") or key.startswith("DEB_BUILD_"):
            environment.pop(key)
    environment.update({"ARCH": "arm64", "CROSS_COMPILE": cross_compile, "KBUILD_DEBARCH": "arm64"})
    if package_version:
        environment["KDEB_PKGVERSION"] = package_version
    return environment

def _configure(contract: Contract, source: Path, build: Path, make: str, cross_compile: str) -> None:
    build.mkdir(parents=True, exist_ok=True)
    environment = _make_environment(cross_compile)
    make_base = [make, "-C", str(source), f"O={build}", "ARCH=arm64", f"CROSS_COMPILE={cross_compile}"]
    _run([make, "-s", "-C", str(source), f"O={build}", "ARCH=arm64", f"CROSS_COMPILE={cross_compile}", "bcm2711_defconfig"], env=environment)
    config_script = source / "scripts/config"
    if not config_script.is_file():
        raise BuildError(f"missing kernel config helper: {config_script}")
    _run([
        "bash",
        str(config_script),
        "--file",
        str(build / ".config"),
        "--set-str",
        "CONFIG_LOCALVERSION",
        contract.localversion,
        "--disable",
        "CONFIG_LOCALVERSION_AUTO",
    ], env=environment)
    _run([*make_base, "olddefconfig"], env=environment)
    _run([*make_base, "syncconfig"], env=environment)
    release = _run([make, "-s", "-C", str(source), f"O={build}", "ARCH=arm64", f"CROSS_COMPILE={cross_compile}", "kernelrelease"], env=environment, capture=True).strip()
    if release != contract.kernel_release:
        raise BuildError(f"kernelrelease mismatch: {release} != {contract.kernel_release}")

def select_exact_linux_image_package(candidates: list[Path], contract: Contract) -> Path:
    packages = sorted(path for path in candidates if path.suffix == ".deb")
    if len(packages) != 1:
        raise BuildError(f"expected exactly one Debian package, found {len(packages)}")
    package = packages[0]
    if package.name != contract.package_filename:
        raise BuildError(f"unexpected linux-image package filename: {package.name}")
    return package

def _write_checksum(package: Path, path: Path) -> None:
    digest = sha256_file(package)
    path.write_text(f"{digest}  {package.name}\n", encoding="utf-8")

def _host_architecture() -> str:
    host_architecture = _run(["dpkg", "--print-architecture"], capture=True).strip()
    if not host_architecture:
        raise BuildError("dpkg did not report the host architecture")
    return host_architecture


def _debian_metadata_command(
    args: argparse.Namespace,
    source: Path,
    build: Path,
    kernelrelease: str,
    package_version: str,
    generator: Path,
) -> list[str]:
    return [
        args.make,
        "-s",
        "-C",
        str(source),
        f"O={build}",
        "ARCH=arm64",
        f"CROSS_COMPILE={args.cross_compile}",
        f"KERNELRELEASE={kernelrelease}",
        f"KDEB_PKGVERSION={package_version}",
        f"KBUILD_RUN_COMMAND={generator}",
        "run-command",
    ]


def _generate_debian_metadata(args: argparse.Namespace, contract: Contract, source: Path, build: Path) -> None:
    generator = source / "scripts/package/mkdebian"
    if not generator.is_file():
        raise BuildError(f"missing Debian metadata generator: {generator}")
    _run(
        _debian_metadata_command(args, source, build, contract.kernel_release, contract.package_version, generator),
        env=_make_environment(args.cross_compile),
    )


def _image_package_command(args: argparse.Namespace, build: Path, kernelrelease: str) -> list[str]:
    return [
        "fakeroot",
        "--",
        args.make,
        "-C",
        str(build),
        "-f",
        "debian/rules",
        "ARCH=arm64",
        f"CROSS_COMPILE={args.cross_compile}",
        f"KERNELRELEASE={kernelrelease}",
        "binary-image",
    ]


def _package_environment(cross_compile: str, host_architecture: str) -> dict[str, str]:
    environment = _make_environment(cross_compile)
    environment.update({"DEB_HOST_ARCH": "arm64", "DEB_BUILD_ARCH": host_architecture})
    return environment


def _run_image_package(args: argparse.Namespace, build: Path, kernelrelease: str, environment: dict[str, str]) -> None:
    _run(_image_package_command(args, build, kernelrelease), cwd=build, env=environment)


def _build(args: argparse.Namespace, contract: Contract, run_dir: Path) -> None:
    source = _prepare_source(contract, args.source_dir, run_dir)
    applied_patches = _apply_patches(contract, source)
    _configure(contract, source, run_dir / "build", args.make, args.cross_compile)
    if args.dry_run:
        print(f"Raspberry kernel dry-run gates passed for {contract.kernel_release}")
        print(f"source_commit={contract.source_commit}")
        print(f"patches={','.join(applied_patches)}")
        return

    build = run_dir / "build"
    _generate_debian_metadata(args, contract, source, build)
    output = args.output_dir.resolve()
    if output.exists() and any(output.iterdir()):
        raise BuildError(f"output directory must be empty: {output}")
    if list(run_dir.rglob("*.deb")):
        raise BuildError("fresh Raspberry build output contains stale Debian packages")
    _run_image_package(
        args,
        build,
        contract.kernel_release,
        _package_environment(args.cross_compile, _host_architecture()),
    )
    package = select_exact_linux_image_package(list(run_dir.rglob("*.deb")), contract)
    staging = run_dir / "artifact-stage"
    staging.mkdir()
    staged_package = staging / contract.package_filename
    shutil.copy2(package, staged_package)
    checksum = staging / "SHA256SUMS"
    _write_checksum(staged_package, checksum)

    validator = Path(__file__).with_name("validate-rpi-kernel-package.py")
    _run([
        sys.executable,
        str(validator),
        str(staged_package),
        "--manifest",
        str(contract.manifest_path),
    ])
    if output.exists() and any(output.iterdir()):
        raise BuildError(f"output directory is no longer empty: {output}")
    output.mkdir(parents=True, exist_ok=True)
    for artifact in (staged_package, checksum):
        shutil.copy2(artifact, output / artifact.name)
    final_package = output / contract.package_filename
    print(f"Raspberry kernel package written: {final_package}")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Build the pinned Octessera Raspberry arm64 kernel package.")
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--source-dir", type=Path)
    parser.add_argument("--output-dir", type=Path, default=Path("build/rpi-kernel"))
    parser.add_argument("--work-dir", type=Path)
    parser.add_argument("--cross-compile", default="aarch64-linux-gnu-")
    parser.add_argument("--make", default="make")
    parser.add_argument("--dry-run", action="store_true", help="run source, patch, config, and kernelrelease gates without compiling")
    parser.add_argument("--keep-work", action="store_true")
    args = parser.parse_args(argv)
    root = Path(__file__).resolve().parents[2]
    temporary: tempfile.TemporaryDirectory[str] | None = None
    try:
        contract = load_contract(root, args.manifest)
        if args.work_dir:
            active_run_dir = args.work_dir.resolve()
            active_run_dir.mkdir(parents=True, exist_ok=True)
            if any(active_run_dir.iterdir()):
                raise BuildError(f"work directory must be empty: {active_run_dir}")
        elif args.keep_work:
            active_run_dir = Path(tempfile.mkdtemp(prefix="octessera-rpi-kernel-"))
        else:
            temporary = tempfile.TemporaryDirectory(prefix="octessera-rpi-kernel-")
            active_run_dir = Path(temporary.name)
        _build(args, contract, active_run_dir)
    except (ContractError, BuildError) as error:
        print(f"Raspberry kernel build failed: {error}", file=sys.stderr)
        return 1
    finally:
        if temporary:
            temporary.cleanup()
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
