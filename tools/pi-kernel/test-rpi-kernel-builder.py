from __future__ import annotations

from pathlib import Path
import tempfile
from types import SimpleNamespace
from typing import Any, Callable


def _write(path: Path, value: bytes | str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(value if isinstance(value, bytes) else value.encode())


def _expect_failure(builder: Any, label: str, operation: Callable[[], Any]) -> None:
    try:
        operation()
    except builder.BuildError:
        return
    raise AssertionError(f"builder fixture was accepted: {label}")


def _metadata_fixture(root: Path, contract: Any, builder: Any) -> tuple[Any, Path, Path]:
    source = root / "source"
    build = root / "build"
    rules = b"#!/usr/bin/make -f\nfixture-rules\n"
    rules_source = source / contract.package_builder["rules_source"]
    _write(source / contract.package_builder["generator"], "#!/bin/sh\n")
    _write(rules_source, rules)
    package_builder = dict(contract.package_builder)
    package_builder["rules_sha256"] = builder.sha256_bytes(rules)
    fake_contract = SimpleNamespace(package_builder=package_builder, package_version="fixture-version")
    _write(build / "debian/arch", "arm64\n")
    _write(build / "debian/rules.vars", "ARCH := arm64\nKERNELRELEASE := fixture-release\n")
    _write(build / "debian/changelog", "linux-upstream (fixture-version) unstable; urgency=low\n")
    _write(build / "debian/rules", rules)
    return fake_contract, source, build


def run(root: Path, contract: Any, builder: Any) -> None:
    with tempfile.TemporaryDirectory(prefix="octessera-rpi-builder-test-") as temporary:
        fake_contract, source, build = _metadata_fixture(Path(temporary), contract, builder)
        args = SimpleNamespace(make="make", cross_compile="aarch64-linux-gnu-")
        calls: list[tuple[list[str], dict[str, Any]]] = []
        reported_version = {"value": fake_contract.package_version}
        original_run = builder._run

        def capture(command: list[str], **kwargs: Any) -> str:
            calls.append((command, kwargs))
            if command[0] == "dpkg-parsechangelog":
                return reported_version["value"] + "\n"
            return ""

        def generate_metadata() -> dict[str, str]:
            builder._run = capture
            try:
                return builder._generate_debian_metadata(args, fake_contract, source, build, "fixture-release")
            finally:
                builder._run = original_run

        def validate_metadata() -> dict[str, str]:
            builder._run = capture
            try:
                return builder._validate_debian_metadata(fake_contract, source, build, "fixture-release")
            finally:
                builder._run = original_run

        metadata = generate_metadata()
        assert metadata["rules_sha256"] == fake_contract.package_builder["rules_sha256"]
        assert calls[0][0] == [
            "make",
            "-s",
            "-C",
            str(source),
            f"O={build}",
            "ARCH=arm64",
            "CROSS_COMPILE=aarch64-linux-gnu-",
            "KERNELRELEASE=fixture-release",
            "KDEB_PKGVERSION=fixture-version",
            f"KBUILD_RUN_COMMAND={source / fake_contract.package_builder['generator']}",
            "run-command",
        ]
        assert calls[1][0] == ["dpkg-parsechangelog", "-l", "debian/changelog", "-S", "Version"]
        assert calls[1][1]["cwd"] == build

        for relative, value in (
            ("debian/arch", "amd64\n"),
            ("debian/rules.vars", "ARCH := amd64\nKERNELRELEASE := fixture-release\n"),
            ("debian/rules", "mutated\n"),
        ):
            path = build / relative
            original = path.read_bytes()
            _write(path, value)
            _expect_failure(builder, relative, validate_metadata)
            _write(path, original)
        reported_version["value"] = "wrong-version"
        _expect_failure(builder, "changelog version", generate_metadata)
        reported_version["value"] = fake_contract.package_version
        fake_contract.package_builder["rules_sha256"] = "0" * 64
        _expect_failure(builder, "rules hash", validate_metadata)

        package_calls: list[tuple[list[str], dict[str, Any]]] = []

        def capture_package(command: list[str], **kwargs: Any) -> str:
            package_calls.append((command, kwargs))
            return ""

        builder._run = capture_package
        try:
            environment = {"LOCALVERSION": "fixture", "ARCH": "arm64"}
            builder._run_image_package(args, build, "fixture-release", environment)
        finally:
            builder._run = original_run
        assert package_calls[0][0] == builder._image_package_command(args, build, "fixture-release")
        assert package_calls[0][1]["cwd"] == build
        assert package_calls[0][1]["env"] is environment

        def fail_package(command: list[str], **kwargs: Any) -> str:
            raise builder.BuildError("package command failed")

        builder._run = fail_package
        try:
            _expect_failure(builder, "package command failure", lambda: builder._run_image_package(args, build, "fixture-release", environment))
        finally:
            builder._run = original_run
