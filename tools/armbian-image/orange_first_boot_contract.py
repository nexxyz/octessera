from __future__ import annotations

import re
import stat
from collections.abc import Callable
from pathlib import Path
from typing import Any


Require = Callable[[bool, str], None]


def _verify_terminal_identity(root: Path, invariants: dict[str, Any], require: Require) -> None:
    records = [line.split(":") for line in (root / "etc/passwd").read_text(encoding="utf-8").splitlines() if line]
    interactive = [record for record in records if record and record[0] == "octessera"]
    require(len(interactive) == 1 and len(interactive[0]) == 7, "Orange interactive account is missing, duplicated, or malformed")
    account = interactive[0]
    require(account[2].isdigit() and account[3].isdigit() and account[5] == "/home/octessera" and account[6] == "/bin/bash", "Orange interactive account home or shell is not exact")
    groups = [line.split(":") for line in (root / "etc/group").read_text(encoding="utf-8").splitlines() if line]
    primary_groups = [group for group in groups if group and group[0] == "octessera"]
    require(len(primary_groups) == 1 and len(primary_groups[0]) == 4 and primary_groups[0][2].isdigit() and int(primary_groups[0][2]) == int(account[3]), "Orange interactive group is missing, duplicated, or has the wrong GID")
    home = root / account[5].lstrip("/")
    require(home.is_dir() and not home.is_symlink(), "Orange interactive account home is missing or symlinked")
    hush = root / invariants["hushlogin_path"]
    require(hush.is_file() and not hush.is_symlink(), "Orange hushlogin is missing or symlinked")
    metadata = hush.lstat()
    require(metadata.st_uid == int(account[2]) and metadata.st_gid == int(primary_groups[0][2]) and metadata.st_mode & 0o777 == invariants["hushlogin_mode"] and metadata.st_size == 0, "Orange hushlogin ownership, mode, or content is not exact")
    for directory in (root / "etc/pam.d", root / "etc/update-motd.d"):
        if directory.is_dir() and not directory.is_symlink():
            for path in directory.rglob("*"):
                if any("octessera" in part.lower() for part in path.relative_to(directory).parts):
                    require(False, f"Orange repository PAM or update-motd override remains: {path}")


def _verify_ssh_masks(root: Path, invariants: dict[str, Any], require: Require) -> None:
    for unit in invariants["ssh_masked_units"]:
        mask = root / "etc/systemd/system" / unit
        require(mask.is_symlink() and mask.readlink().as_posix() == "/dev/null", f"Orange SSH unit is not masked: {unit}")
        target_directory = "sockets.target.wants" if unit.endswith(".socket") else "multi-user.target.wants"
        enabled = root / "etc/systemd/system" / target_directory / unit
        require(not enabled.exists() and not enabled.is_symlink(), f"Orange SSH unit remains enabled: {unit}")


def _verify_first_boot(root: Path, invariants: dict[str, Any], require: Require) -> None:
    onboarding_marker = root / invariants["armbian_onboarding_marker"]
    require(not onboarding_marker.exists() and not onboarding_marker.is_symlink(), "Orange Armbian onboarding marker remains")
    firstrun = root / invariants["armbian_firstrun_service"]
    require(firstrun.is_file() and not firstrun.is_symlink(), "Orange Armbian firstrun service is missing or symlinked")
    require("ExecStart=/usr/lib/armbian/armbian-firstrun start" in firstrun.read_text(encoding="utf-8"), "Orange Armbian firstrun service is not canonical")
    firstrun_executable = root / invariants["armbian_firstrun_executable"]
    try:
        executable_metadata = firstrun_executable.lstat()
    except FileNotFoundError:
        executable_metadata = None
    require(executable_metadata is not None and stat.S_ISREG(executable_metadata.st_mode) and executable_metadata.st_uid == 0 and executable_metadata.st_gid == 0 and stat.S_IMODE(executable_metadata.st_mode) == 0o755, "Orange Armbian firstrun executable is missing, unsafe, or not executable")
    firstrun_script = firstrun_executable.read_text(encoding="utf-8").splitlines()
    require(firstrun_script and firstrun_script[0] == "#!/bin/bash", "Orange Armbian firstrun executable is missing, unsafe, or not executable")
    regeneration_condition = 'if [[ "${OPENSSHD_REGENERATE_HOST_KEYS}" = true ]]; then'
    condition_index = next((index for index, line in enumerate(firstrun_script) if line.strip() == regeneration_condition), None)
    if condition_index is None:
        require(False, "Orange Armbian firstrun host-key regeneration behavior is missing")
        return
    regeneration_branch = []
    for line in firstrun_script[condition_index + 1 :]:
        stripped = line.strip()
        if stripped == "else":
            break
        regeneration_branch.append(stripped)
    required_regeneration_steps = ("rm -f /etc/ssh/ssh_host*", "dpkg-reconfigure openssh-server >/dev/null 2>&1", "service ssh restart")
    step_index = 0
    for line in regeneration_branch:
        if line == required_regeneration_steps[step_index]:
            step_index += 1
            if step_index == len(required_regeneration_steps):
                break
    require(step_index == len(required_regeneration_steps), "Orange Armbian firstrun host-key regeneration behavior is missing")
    firstrun_enabled = root / invariants["armbian_firstrun_enablement"]
    require(firstrun_enabled.is_symlink() and firstrun_enabled.readlink().as_posix() in {"/lib/systemd/system/armbian-firstrun.service", "/usr/lib/systemd/system/armbian-firstrun.service"}, "Orange Armbian firstrun service is not enabled")
    firstrun_defaults = root / invariants["armbian_firstrun_defaults"]
    require(firstrun_defaults.is_file() and not firstrun_defaults.is_symlink(), "Orange Armbian host-key regeneration defaults are missing or symlinked")
    assignments = []
    for line in firstrun_defaults.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if "OPENSSHD_REGENERATE_HOST_KEYS" not in stripped:
            continue
        match = re.fullmatch(r"(?:export[ \t]+)?OPENSSHD_REGENERATE_HOST_KEYS[ \t]*=[ \t]*(true|false)(?:[ \t]+#.*)?[ \t]*", stripped)
        if match is None:
            require(False, "Orange Armbian host-key regeneration assignment is malformed")
            continue
        assignments.append(match.group(1))
    require(len(assignments) == 1, "Orange Armbian host-key regeneration assignment is missing or duplicated")
    require(assignments == ["true"], "Orange Armbian host-key regeneration is not enabled")
    ssh_directory = root / "etc/ssh"
    require(ssh_directory.is_dir() and not ssh_directory.is_symlink(), "Orange SSH directory is missing or symlinked")
    require(not list(ssh_directory.glob("ssh_host_*")), "Orange image contains baked SSH host keys")


def verify_initial_access(root: Path, invariants: dict[str, Any], require: Require) -> None:
    _verify_terminal_identity(root, invariants, require)
    _verify_ssh_masks(root, invariants, require)


def verify_production_first_boot(root: Path, invariants: dict[str, Any], require: Require) -> None:
    _verify_first_boot(root, invariants, require)
