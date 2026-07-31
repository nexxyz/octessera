#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

from rpi_kernel_contract import (
    EXPECTED_NATIVE_PACKAGES,
    Contract,
    ContractError,
    assert_final_config,
    load_contract,
    sha256_bytes,
    sha256_file,
)

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

def _git_blob(source: Path, commit: str, path: str) -> bytes:
    try:
        result = subprocess.run(
            ["git", "-C", str(source), "show", f"{commit}:{path}"],
            check=True,
            capture_output=True,
        )
    except (FileNotFoundError, subprocess.CalledProcessError) as error:
        raise BuildError(f"cannot read pinned source config {path}") from error
    return result.stdout

def _prepare_source(contract: Contract, source_arg: Path | None, run_dir: Path) -> tuple[Path, str]:
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
    actual = _run(["git", "-C", str(source), "rev-parse", "HEAD"], capture=True).strip()
    if actual != contract.source_commit:
        raise BuildError(f"Raspberry source commit mismatch: {actual} != {contract.source_commit}")
    config_hash = sha256_bytes(_git_blob(source, contract.source_commit, contract.config_path))
    if config_hash != contract.config_sha256:
        raise BuildError(f"Raspberry config base hash mismatch: {config_hash} != {contract.config_sha256}")
    return source, actual

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

def _configure(contract: Contract, source: Path, build: Path, make: str, cross_compile: str) -> dict[str, str]:
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
        contract.config_overrides["CONFIG_LOCALVERSION"],
        "--disable",
        "CONFIG_LOCALVERSION_AUTO",
    ], env=environment)
    _run([*make_base, "olddefconfig"], env=environment)
    _run([*make_base, "syncconfig"], env=environment)
    config = assert_final_config(build / ".config", contract)
    release = _run([make, "-s", "-C", str(source), f"O={build}", "ARCH=arm64", f"CROSS_COMPILE={cross_compile}", "kernelrelease"], env=environment, capture=True).strip()
    if release != contract.kernel_release:
        raise BuildError(f"kernelrelease mismatch: {release} != {contract.kernel_release}")
    config["path"] = str((build / ".config").relative_to(build).as_posix())
    return config

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

def _write_json(path: Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")

def _checkout_sha(root: Path) -> str:
    value = _run(["git", "-C", str(root), "rev-parse", "HEAD"], capture=True).strip()
    if re.fullmatch(r"[0-9a-f]{40}", value) is None:
        raise BuildError("Octessera checkout SHA is not a full SHA-1")
    return value


def _tool_version(command: str, *, version_argument: str = "--version", recorded_command: str | None = None) -> dict[str, str]:
    output = _run([command, version_argument], capture=True)
    version = next((line.strip() for line in output.splitlines() if line.strip()), "")
    if not version:
        raise BuildError(f"tool did not report a version: {command}")
    return {
        "command": recorded_command or command,
        "version": version,
        "version_sha256": sha256_bytes(version.encode("utf-8")),
    }

def _tool_commands(args: argparse.Namespace) -> dict[str, str]:
    return {
        "compiler": f"{args.cross_compile}gcc",
        "linker": f"{args.cross_compile}ld",
        "host_compiler": "gcc",
        "host_linker": "ld",
        "make": args.make,
        "bc": "bc",
        "bison": "bison",
        "flex": "flex",
        "openssl": "openssl",
        "fakeroot": "fakeroot",
        "dpkg": "dpkg",
        "dpkg_deb": "dpkg-deb",
        "dpkg_query": "dpkg-query",
        "dpkg_parsechangelog": "dpkg-parsechangelog",
        "readelf": "readelf",
        "strings": "strings",
        "git": "git",
        "python": sys.executable,
        "bash": "bash",
    }


def _debhelper_version(host_architecture: str) -> dict[str, str]:
    command = ["dpkg-query", "-W", "-f=${Version}", f"debhelper:{host_architecture}"]
    output = _run(command, capture=True).strip()
    if not output:
        raise BuildError("native tool preflight did not report the debhelper version")
    return {
        "command": " ".join(command),
        "version": output,
        "version_sha256": sha256_bytes(output.encode("utf-8")),
    }


def _tool_versions(args: argparse.Namespace, host_architecture: str) -> dict[str, dict[str, str]]:
    versions = {}
    for name, command in _tool_commands(args).items():
        if name == "openssl":
            versions[name] = _tool_version(command, version_argument="version", recorded_command="openssl version")
        elif name == "dpkg_parsechangelog":
            versions[name] = _tool_version(command, recorded_command="dpkg-parsechangelog --version")
        else:
            versions[name] = _tool_version(command)
    versions["debhelper"] = _debhelper_version(host_architecture)
    return versions

def _host_architecture() -> str:
    host_architecture = _run(["dpkg", "--print-architecture"], capture=True).strip()
    if not host_architecture:
        raise BuildError("native package preflight did not report the host architecture")
    return host_architecture


def _preflight_packages(host_architecture: str) -> dict[str, dict[str, Any]]:
    packages: dict[str, dict[str, Any]] = {}
    for package, headers in EXPECTED_NATIVE_PACKAGES.items():
        package_ref = f"{package}:{host_architecture}"
        metadata = _run(
            ["dpkg-query", "-W", "-f=${db:Status-Status} ${Architecture} ${Version}", package_ref],
            capture=True,
        ).strip().split(maxsplit=2)
        if len(metadata) != 3 or metadata[0] != "installed" or metadata[1] != host_architecture:
            raise BuildError(f"native package preflight requires installed {package_ref}")
        files = set(_run(["dpkg-query", "-L", package_ref], capture=True).splitlines())
        missing_headers = [header for header in headers if header not in files or not Path(header).is_file()]
        if missing_headers:
            raise BuildError(f"native package preflight is missing headers for {package_ref}: {', '.join(missing_headers)}")
        packages[package] = {
            "package": package_ref,
            "architecture": metadata[1],
            "status": metadata[0],
            "version": metadata[2],
            "headers": list(headers),
        }
    return packages


def preflight_build_environment(args: argparse.Namespace) -> dict[str, Any]:
    try:
        host_architecture = _host_architecture()
    except BuildError as error:
        raise BuildError(f"native package preflight failed: {error}") from error
    try:
        tools = _tool_versions(args, host_architecture)
    except BuildError as error:
        raise BuildError(f"native tool preflight failed: {error}") from error
    try:
        packages = _preflight_packages(host_architecture)
    except BuildError as error:
        raise BuildError(f"native package preflight failed: {error}") from error
    return {"host_architecture": host_architecture, "tools": tools, "packages": packages}


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


def _generate_debian_metadata(args: argparse.Namespace, contract: Contract, source: Path, build: Path, kernelrelease: str) -> dict[str, str]:
    generator = source / contract.package_builder["generator"]
    if not generator.is_file():
        raise BuildError(f"missing pinned Debian metadata generator: {generator}")
    environment = _make_environment(args.cross_compile)
    _run(
        _debian_metadata_command(args, source, build, kernelrelease, contract.package_version, generator),
        env=environment,
    )
    return _validate_debian_metadata(contract, source, build, kernelrelease)


def _validate_debian_metadata(contract: Contract, source: Path, build: Path, kernelrelease: str) -> dict[str, str]:
    builder = contract.package_builder
    generator = source / builder["generator"]
    rules_source = source / builder["rules_source"]
    debian = build / "debian"
    if not rules_source.is_file():
        raise BuildError(f"missing pinned Debian rules source: {rules_source}")
    arch = debian / "arch"
    rules_vars = debian / "rules.vars"
    changelog = debian / "changelog"
    rules = debian / "rules"
    for path in (arch, rules_vars, changelog, rules):
        if not path.is_file():
            raise BuildError(f"missing generated Debian metadata: {path}")
    if arch.read_text(encoding="utf-8") != "arm64\n":
        raise BuildError("generated Debian architecture is not exactly arm64")
    expected_rules_vars = f"ARCH := arm64\nKERNELRELEASE := {kernelrelease}\n"
    if rules_vars.read_text(encoding="utf-8") != expected_rules_vars:
        raise BuildError("generated Debian rules.vars does not match ARCH/kernelrelease")
    changelog_version = _run(
        ["dpkg-parsechangelog", "-l", "debian/changelog", "-S", "Version"],
        cwd=build,
        capture=True,
    ).strip()
    if changelog_version != contract.package_version:
        raise BuildError(f"generated Debian changelog version mismatch: {changelog_version} != {contract.package_version}")
    generated_rules = rules.read_bytes()
    source_rules = rules_source.read_bytes()
    if generated_rules != source_rules:
        raise BuildError("generated Debian rules differ from the pinned source rules")
    rules_sha256 = sha256_bytes(generated_rules)
    if rules_sha256 != builder["rules_sha256"]:
        raise BuildError(f"generated Debian rules hash mismatch: {rules_sha256} != {builder['rules_sha256']}")
    return {
        "arch": "arm64",
        "kernelrelease": kernelrelease,
        "changelog_version": changelog_version,
        "rules_sha256": rules_sha256,
    }


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


def _provenance_scope(contract: Contract) -> dict[str, Any]:
    scope = list(contract.package_builder["binary_package_scope"])
    return {
        "binary_package_scope": scope,
        "uapi_headers_prepared_by_build_arch": True,
        "header_package": False,
        "libc_dev_package": False,
        "dev_package": False,
        "debug_package": False,
        "description": "build-arch prepares UAPI headers for the image build but does not package headers, libc-dev, dev, or debug artifacts.",
    }


def _build(args: argparse.Namespace, contract: Contract, run_dir: Path) -> None:
    source, actual_commit = _prepare_source(contract, args.source_dir, run_dir)
    applied_patches = _apply_patches(contract, source)
    config = _configure(contract, source, run_dir / "build", args.make, args.cross_compile)
    if args.dry_run:
        print(f"Raspberry kernel dry-run gates passed for {contract.kernel_release}")
        print(f"source_commit={actual_commit}")
        print(f"patches={','.join(applied_patches)}")
        return

    preflight = preflight_build_environment(args)
    build = run_dir / "build"
    metadata = _generate_debian_metadata(args, contract, source, build, contract.kernel_release)
    output = args.output_dir.resolve()
    if output.exists() and any(output.iterdir()):
        raise BuildError(f"output directory must be empty: {output}")
    if list(run_dir.rglob("*.deb")):
        raise BuildError("fresh Raspberry build output contains stale Debian packages")
    _run_image_package(
        args,
        build,
        contract.kernel_release,
        _package_environment(args.cross_compile, preflight["host_architecture"]),
    )
    package = select_exact_linux_image_package(list(run_dir.rglob("*.deb")), contract)
    staging = run_dir / "artifact-stage"
    staging.mkdir()
    staged_package = staging / contract.package_filename
    shutil.copy2(package, staged_package)
    checksum = staging / "SHA256SUMS"
    _write_checksum(staged_package, checksum)

    validator = Path(__file__).with_name("validate-rpi-kernel-package.py")
    inventory_path = staging / "inventory.json"
    provenance_path = staging / "provenance.json"
    _run([
        sys.executable,
        str(validator),
        str(staged_package),
        "--manifest",
        str(contract.manifest_path),
        "--checksum-file",
        str(checksum),
        "--inventory-out",
        str(inventory_path),
        "--provenance-out",
        str(provenance_path),
    ])
    inventory = json.loads(inventory_path.read_text(encoding="utf-8"))
    if inventory["config"]["sha256"] != config["sha256"]:
        raise BuildError("packaged kernel config hash does not match the configured config hash")
    provenance = json.loads(provenance_path.read_text(encoding="utf-8"))
    provenance["build"] = {
        "source_commit": actual_commit,
        "patch_order": applied_patches,
        "config_gate": config,
        "arch": "arm64",
        "cross_compile": args.cross_compile,
        "builder": dict(contract.package_builder),
        "octessera_checkout_sha": _checkout_sha(contract.root),
        "rules_sha256": metadata["rules_sha256"],
        "target": contract.package_builder["target"],
        "fakeroot": contract.package_builder["fakeroot"],
        "scope": _provenance_scope(contract),
        "debian_metadata": metadata,
        "preflight": preflight,
        "tool_versions": preflight["tools"],
    }
    _write_json(provenance_path, provenance)
    _run([
        sys.executable,
        str(validator),
        str(staged_package),
        "--manifest",
        str(contract.manifest_path),
        "--checksum-file",
        str(checksum),
        "--provenance-in",
        str(provenance_path),
    ])
    if output.exists() and any(output.iterdir()):
        raise BuildError(f"output directory is no longer empty: {output}")
    output.mkdir(parents=True, exist_ok=True)
    for artifact in (staged_package, checksum, inventory_path, provenance_path):
        shutil.copy2(artifact, output / artifact.name)
    final_package = output / contract.package_filename
    print(f"Raspberry kernel package written: {final_package}")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="Build the pinned Octessera Raspberry arm64 kernel package.")
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--source-dir", type=Path)
    parser.add_argument("--output-dir", type=Path, default=Path("release-artifacts/rpi-kernel"))
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
