#!/usr/bin/env python3
import argparse
import hashlib
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import NoReturn
PATCH_TARGETS = (
    "drivers/usb/gadget/function/f_midi.c",
    "drivers/usb/gadget/function/u_midi.h",
    "Documentation/ABI/testing/configfs-usb-gadget-midi",
    "Documentation/usb/gadget-testing.rst",
)
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
SERIES_PATH = "patch/kernel/archive/sunxi-6.18/series.conf"
SERIES_ROOT = "patch/kernel/archive/sunxi-6.18"
EXPECTED_ARMBIAN_COMMIT = "fa7a7b2294d9e760a77630950afd460b7a0b2a26"
EXPECTED_ORANGE_KERNEL_COMMIT = "e46dc0adfe39724bcf52cea47b8f9c9aed86a394"
EXPECTED_ORANGE_KERNEL_RELEASE = "6.18.38-current-sunxi64"
EXPECTED_ORANGE_PACKAGE_REVISION = "26.8.0-trunk.417"
EXPECTED_RASPBERRY_KERNEL_COMMIT = "d8ab4e908235da7727f22dd36ad5af224671677d"
EXPECTED_RASPBERRY_RELEASE = "6.12.93"
EXPECTED_RASPBERRY_KERNEL_RELEASE = "6.12.93-octessera-rpi-v8-0.7.5"
EXPECTED_RASPBERRY_PACKAGE_REVISION = "6.12.93-octessera0.7.5-1"
EXPECTED_ORANGE_PACKAGES = (
    "linux-image-current-sunxi64_26.8.0-trunk.417_arm64.deb",
    "linux-dtb-current-sunxi64_26.8.0-trunk.417_arm64.deb",
)
class GateFailure(Exception):
    pass
def fail(message: str) -> NoReturn:
    raise GateFailure(message)
def run(command, cwd=None):
    result = subprocess.run(
        command,
        cwd=cwd,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode:
        output = (result.stdout + result.stderr).strip()
        fail(f"command failed ({result.returncode}): {' '.join(command)}\n{output}")
    return result.stdout + result.stderr
def require_commit(value, name):
    if not isinstance(value, str) or not COMMIT_RE.fullmatch(value):
        fail(f"{name} is not a full lowercase commit SHA")
def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
def load_manifest(root):
    path = root / "tools/kernel-patches/orange-midi-interface-manifest.json"
    try:
        manifest = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot load manifest: {error}")
    expected_order = [
        "zzzz-0001-usb-gadget-f-midi-configfs-interface-string.patch",
        "zzzz-0002-usb-gadget-f-midi-instance-local-string.patch",
    ]
    if manifest.get("patch_order") != expected_order:
        fail("manifest patch order is not the required two-patch order")
    patch_root = root / manifest.get("patch_root", "")
    if not patch_root.is_dir():
        fail(f"missing patch root: {patch_root}")
    if any(path.name == "series.conf" for path in patch_root.rglob("series.conf")):
        fail("user patch root contains series.conf")
    actual_patches = sorted(
        path.relative_to(patch_root).as_posix()
        for path in patch_root.rglob("*.patch")
        if path.is_file()
    )
    if actual_patches != sorted(expected_order):
        fail(f"user patch root .patch set differs from manifest: {actual_patches}")
    patches = manifest.get("patches", {})
    accepted = patches.get("accepted_upstream", {})
    follow_up = patches.get("octessera_follow_up", {})
    if accepted.get("sha256") != sha256(patch_root / expected_order[0]):
        fail("accepted upstream patch SHA-256 does not match the manifest")
    if follow_up.get("sha256") != sha256(patch_root / expected_order[1]):
        fail("Octessera follow-up patch SHA-256 does not match the manifest")
    require_commit(accepted.get("commit"), "accepted upstream commit")
    if not accepted.get("url", "").endswith(accepted["commit"] + ".patch"):
        fail("accepted upstream patch URL is not pinned to its commit")
    frameworks = manifest.get("build_frameworks", {})
    armbian = frameworks.get("armbian", {})
    pi_gen = frameworks.get("pi_gen", {})
    require_commit(armbian.get("commit"), "Armbian commit")
    require_commit(pi_gen.get("commit"), "pi-gen commit")
    if armbian.get("commit") != EXPECTED_ARMBIAN_COMMIT:
        fail("manifest Armbian commit differs from the pinned source gate commit")
    if pi_gen.get("commit") != "d7a31c6aa09f4b867902c51da2b45807c0a1709e":
        fail("manifest pi-gen commit differs from the pinned source gate commit")
    kernels = manifest.get("kernels", {})
    for name in ("orange", "raspberry"):
        kernel = kernels.get(name, {})
        require_commit(kernel.get("commit"), f"{name} kernel commit")
        if not kernel.get("release"):
            fail(f"{name} kernel release is missing")
        if name == "raspberry":
            config = kernel.get("config_base", {})
            if not config.get("path") or not re.fullmatch(r"[0-9a-f]{64}", config.get("sha256", "")):
                fail(f"{name} config base is incomplete")
        if not kernel.get("package_revision") and name == "raspberry":
            fail("Raspberry package revision is missing")
    if not armbian.get("package_revision") or not armbian.get("packages"):
        fail("Armbian package revision or package pair is missing")
    if armbian.get("kernel_commit") != kernels.get("orange", {}).get("commit"):
        fail("Armbian and Orange kernel commits differ in the manifest")
    if armbian.get("kernel_release") != kernels.get("orange", {}).get("release"):
        fail("Armbian and Orange kernel releases differ in the manifest")
    orange = kernels.get("orange", {})
    if orange.get("package_revision") != armbian.get("package_revision"):
        fail("Armbian and Orange package revisions differ in the manifest")
    if tuple(orange.get("packages", ())) != tuple(armbian.get("packages", ())):
        fail("Armbian and Orange package names differ in the manifest")
    if armbian.get("kernel_commit") != EXPECTED_ORANGE_KERNEL_COMMIT:
        fail("manifest Orange kernel commit differs from the pinned source gate commit")
    if armbian.get("kernel_release") != EXPECTED_ORANGE_KERNEL_RELEASE:
        fail("manifest Orange kernel release differs from the pinned source gate release")
    if armbian.get("package_revision") != EXPECTED_ORANGE_PACKAGE_REVISION:
        fail("manifest Orange package revision differs from the pinned source gate revision")
    if tuple(armbian.get("packages", ())) != EXPECTED_ORANGE_PACKAGES:
        fail("manifest Orange package names differ from the pinned package pair")
    core_series = armbian.get("core_series", {})
    if core_series.get("path") != SERIES_PATH or core_series.get("active_patch_count") != 515:
        fail("manifest Armbian core series identity is incomplete")
    if not re.fullmatch(r"[0-9a-f]{64}", core_series.get("sha256", "")):
        fail("manifest Armbian core series SHA-256 is invalid")
    patching_order = armbian.get("patching_order", {})
    if patching_order.get("source_path") != "lib/tools/patching.py":
        fail("manifest Armbian patching-order source is invalid")
    if patching_order.get("source_sha256") != "b53967b15f216872551b0261f9f917b865ad9fd132c1c3803a84fd07d0522a84":
        fail("manifest Armbian patching-order source is not the pinned revision")
    if patching_order.get("series_before_regular") is not True or patching_order.get("regular_sort_key") != "file_name":
        fail("manifest Armbian regular patch ordering is incomplete")
    raspberry = kernels.get("raspberry", {})
    if raspberry.get("commit") != EXPECTED_RASPBERRY_KERNEL_COMMIT:
        fail("manifest Raspberry kernel commit differs from the pinned source gate commit")
    if raspberry.get("release") != EXPECTED_RASPBERRY_RELEASE:
        fail("manifest Raspberry kernel release differs from the pinned source gate release")
    if raspberry.get("kernel_release") != EXPECTED_RASPBERRY_KERNEL_RELEASE:
        fail("manifest Raspberry packaged kernel release differs from the pinned release")
    if raspberry.get("package_revision") != EXPECTED_RASPBERRY_PACKAGE_REVISION:
        fail("manifest Raspberry package revision differs from the pinned source gate revision")
    return manifest, patch_root, expected_order
def prepare_repo(parent, name, repository, commit, source):
    if source:
        repo = Path(source).resolve()
        if not repo.is_dir():
            fail(f"{name} source directory does not exist: {repo}")
    else:
        repo = parent / name
        run(["git", "init", str(repo)])
        run(["git", "-C", str(repo), "remote", "add", "origin", repository])
        run(["git", "-C", str(repo), "fetch", "--filter=blob:none", "--depth=1", "origin", commit])
    actual = run(["git", "-C", str(repo), "rev-parse", commit if source else "FETCH_HEAD"]).strip()
    if actual != commit:
        fail(f"{name} source resolved to {actual}, expected {commit}")
    return repo
def source_blob(repo, commit, path):
    return subprocess.check_output(
        ["git", "-C", str(repo), "show", f"{commit}:{path}"],
    )
def stage_source(repo, commit, destination):
    for path in PATCH_TARGETS:
        target = destination / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(source_blob(repo, commit, path))
def active_series_entries(series_text):
    entries = []
    for line in series_text.splitlines():
        value = line.strip()
        if value and not value.startswith(("#", "-")):
            entries.append(value)
    return entries
def patch_paths(patch_text):
    paths = set()
    for line in patch_text.splitlines():
        match = re.match(r"^diff --git a/(.+) b/(.+)$", line)
        if match:
            paths.update(match.groups())
            continue
        if line.startswith(("--- ", "+++ ")):
            value = line[4:].split("\t", 1)[0].split(" ", 1)[0]
            if value.startswith(("a/", "b/")):
                paths.add(value[2:])
    return paths
def direct_patch_names(repo, commit, root):
    prefix = root.rstrip("/") + "/"
    paths = run(["git", "-C", str(repo), "ls-tree", "-r", "--name-only", commit, "--", root]).splitlines()
    names = []
    for path in paths:
        if not path.startswith(prefix) or not path.endswith(".patch"):
            continue
        relative = path[len(prefix) :].split("/")
        if len(relative) == 1 or (len(relative) == 2 and relative[0].startswith(("board_", "target_"))):
            names.append(relative[-1])
    return sorted(names)
def inspect_armbian_order(manifest, armbian_repo):
    armbian = manifest["build_frameworks"]["armbian"]
    commit = armbian["commit"]
    series = armbian["core_series"]
    series_data = source_blob(armbian_repo, commit, series["path"])
    if hashlib.sha256(series_data).hexdigest() != series["sha256"]:
        fail("pinned Armbian series.conf SHA-256 changed")
    entries = active_series_entries(series_data.decode("utf-8"))
    if len(entries) != series["active_patch_count"]:
        fail(f"pinned Armbian series.conf active inventory changed: {len(entries)}")
    targets = set(PATCH_TARGETS)
    for entry in entries:
        if entry.startswith("/") or ".." in Path(entry).parts:
            fail(f"invalid path in pinned Armbian series.conf: {entry}")
        patch_path = f"{SERIES_ROOT}/{entry}"
        patch_data = source_blob(armbian_repo, commit, patch_path).decode("utf-8", errors="replace")
        changed = patch_paths(patch_data)
        if not changed:
            fail(f"cannot inspect paths touched by core series patch: {entry}")
        overlap = sorted(changed & targets)
        if overlap:
            fail(f"core series patch touches MIDI gate target {overlap}: {entry}")
    ordering = armbian["patching_order"]
    patching_data = source_blob(armbian_repo, commit, ordering["source_path"])
    if hashlib.sha256(patching_data).hexdigest() != ordering["source_sha256"]:
        fail("pinned Armbian patching-order source changed")
    patching_text = patching_data.decode("utf-8")
    markers = (
        "SERIES_PATCH_FILES: list[patching_utils.PatchFileInDir] = []",
        "NORMAL_PATCH_FILES = list(dict(sorted(ALL_DIR_PATCH_FILES_BY_NAME.items())).values())",
        "ALL_PATCH_FILES_SORTED = PATCH_FILES_FIRST + SERIES_PATCH_FILES + NORMAL_PATCH_FILES",
    )
    if any(marker not in patching_text for marker in markers):
        fail("pinned Armbian patching order no longer proves series-before-sorted-regular")
    core_regular = direct_patch_names(armbian_repo, commit, SERIES_ROOT)
    return entries, core_regular
def validate_regular_patch_order(patch_root, patch_order, core_regular):
    user_regular = sorted(path.name for path in patch_root.glob("*.patch") if path.is_file())
    if user_regular != sorted(patch_order):
        fail(f"user regular patch inventory differs from manifest: {user_regular}")
    regular_names = core_regular + user_regular
    if len(regular_names) != len(set(regular_names)):
        fail("Armbian regular patch ordering has duplicate basenames")
    if sorted(regular_names)[-len(patch_order):] != patch_order:
        fail("Octessera patches are not the final regular patches in Armbian ordering")
def apply_patch(source, patch, label, target):
    try:
        output = run(["git", "-C", str(source), "apply", "--check", "--verbose", "--whitespace=error", str(patch)])
        output += run(["git", "-C", str(source), "apply", "--verbose", "--whitespace=error", str(patch)])
    except GateFailure as error:
        fail(f"{target}: {label} conflict; no workaround was attempted\n{error}")
    if re.search(r"fuzz|offset|reject", output, re.IGNORECASE):
        fail(f"{target}: {label} applied with fuzz, offset, or reject output\n{output}")
def validate_follow_up_patch(patch):
    paths = []
    for line in patch.read_text().splitlines():
        if line.startswith("diff --git "):
            fields = line.split()
            if len(fields) != 4:
                fail("follow-up patch has malformed diff header")
            paths.append((fields[2][2:], fields[3][2:]))
    if paths != [("drivers/usb/gadget/function/f_midi.c", "drivers/usb/gadget/function/f_midi.c")]:
        fail("follow-up patch modifies more than f_midi.c")
def function_body(source, signature, target):
    start = source.find(signature)
    if start < 0:
        fail(f"{target}: missing function signature: {signature}")
    opening = source.find("{", start)
    if opening < 0:
        fail(f"{target}: missing function body: {signature}")
    depth = 0
    quote = None
    escaped = False
    line_comment = False
    block_comment = False
    index = opening
    while index < len(source):
        char = source[index]
        next_char = source[index + 1] if index + 1 < len(source) else ""
        if line_comment:
            if char == "\n":
                line_comment = False
            index += 1
            continue
        if block_comment:
            if char == "*" and next_char == "/":
                block_comment = False
                index += 2
            else:
                index += 1
            continue
        if quote:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == quote:
                quote = None
            index += 1
            continue
        if char == "/" and next_char == "/":
            line_comment = True
            index += 2
            continue
        if char == "/" and next_char == "*":
            block_comment = True
            index += 2
            continue
        if char in ('"', "'"):
            quote = char
        elif char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return source[start : index + 1]
        index += 1
    fail(f"{target}: unterminated function body: {signature}")
def validate_final_text(midi, header, target):
    if not re.search(r"struct\s+f_midi\s*\{.*?char\s+\*interface_string\s*;", midi, re.DOTALL):
        fail(f"{target}: f_midi does not own interface_string")
    if "interface_string" not in header:
        fail(f"{target}: u_midi.h does not expose interface_string")
    if re.search(r"static\s+struct\s+usb_string\s+midi_string_defs", midi):
        fail(f"{target}: static midi_string_defs remains")
    if "midi_string_defs[STRING_FUNC_IDX].s = opts->interface_string" in midi:
        fail(f"{target}: instance pointer is assigned to static midi_string_defs")
    bind = function_body(midi, "static int f_midi_bind", target)
    allocation = function_body(midi, "static struct usb_function *f_midi_alloc", target)
    final_free = function_body(midi, "static void f_midi_free(struct usb_function *f)", target)
    local_table = re.search(
        r"struct\s+usb_string\s+midi_string_defs\[\]\s*=\s*\{\s*"
        r"\[STRING_FUNC_IDX\]\.s\s*=\s*midi->interface_string\s*,",
        bind,
        re.DOTALL,
    )
    if local_table is None:
        fail(f"{target}: bind table does not reference midi->interface_string")
    for declaration in (
        "struct usb_gadget_strings midi_stringtab",
        "struct usb_gadget_strings *midi_strings[]",
        "usb_gstrings_attach(c->cdev, midi_strings",
    ):
        if declaration not in bind:
            fail(f"{target}: missing instance-local bind structure: {declaration}")
    duplicate = allocation.find("midi->interface_string = kstrdup")
    if duplicate < 0:
        fail(f"{target}: interface_string duplication is missing")
    lock = allocation.find("mutex_lock(&opts->lock);")
    unlock = allocation.find("mutex_unlock(&opts->lock);", duplicate)
    if lock < 0 or not lock < duplicate < unlock:
        fail(f"{target}: options lock does not cover interface_string duplication")
    if not re.search(
        r"midi->interface_string\s*=\s*kstrdup\([^;]+\);\s*"
        r"if\s*\(\s*!midi->interface_string\s*\)\s*\{\s*"
        r"status\s*=\s*-ENOMEM;\s*goto\s+midi_free;\s*\}",
        allocation,
        re.DOTALL,
    ):
        fail(f"{target}: interface_string allocation lacks its immediate null check")
    midi_free = allocation.find("midi_free:")
    setup_fail = allocation.find("setup_fail:")
    if midi_free < 0 or setup_fail < 0:
        fail(f"{target}: allocation cleanup labels are incomplete")
    if "kfree(midi->interface_string);" not in allocation[midi_free:setup_fail]:
        fail(f"{target}: midi_free does not free interface_string")
    if "kfree(midi->interface_string);" not in final_free:
        fail(f"{target}: final f_midi_free path does not free interface_string")
def replace_function(source, signature, mutation, target):
    original = function_body(source, signature, target)
    start = source.find(signature)
    replacement = mutation(original)
    return source[:start] + replacement + source[start + len(original) :]
def validate_negative_mutation_fixtures(midi, header, target):
    mutations = (
        (
            "lock ordering",
            lambda source: replace_function(
                source,
                "static struct usb_function *f_midi_alloc",
                lambda body: body.replace("mutex_lock(&opts->lock);", "", 1),
                target,
            ),
        ),
        (
            "local table pointer",
            lambda source: replace_function(
                source,
                "static int f_midi_bind",
                lambda body: body.replace("midi->interface_string", "\"broken\"", 1),
                target,
            ),
        ),
        (
            "immediate allocation null check",
            lambda source: replace_function(
                source,
                "static struct usb_function *f_midi_alloc",
                lambda body: body.replace("if (!midi->interface_string)", "if (false)", 1),
                target,
            ),
        ),
        (
            "midi_free cleanup",
            lambda source: replace_function(
                source,
                "static struct usb_function *f_midi_alloc",
                lambda body: body.replace("kfree(midi->interface_string);", "", 1),
                target,
            ),
        ),
        (
            "final free cleanup",
            lambda source: replace_function(
                source,
                "static void f_midi_free(struct usb_function *f)",
                lambda body: body.replace("kfree(midi->interface_string);", "", 1),
                target,
            ),
        ),
    )
    for label, mutate in mutations:
        mutated = mutate(midi)
        try:
            validate_final_text(mutated, header, f"{target} negative fixture: {label}")
        except GateFailure:
            continue
        fail(f"{target}: negative mutation fixture was accepted: {label}")
def validate_final_source(source, target):
    midi = (source / "drivers/usb/gadget/function/f_midi.c").read_text()
    header = (source / "drivers/usb/gadget/function/u_midi.h").read_text()
    validate_final_text(midi, header, target)
    validate_negative_mutation_fixtures(midi, header, target)
def validate_config_bases(manifest, armbian_repo, raspberry_repo):
    armbian = manifest["build_frameworks"]["armbian"]
    orange_config = armbian["config_base"]
    orange_data = source_blob(armbian_repo, armbian["commit"], orange_config["path"])
    if hashlib.sha256(orange_data).hexdigest() != orange_config["sha256"]:
        fail("Armbian config base SHA-256 does not match the manifest")
    raspberry = manifest["kernels"]["raspberry"]
    raspberry_config = raspberry["config_base"]
    raspberry_data = source_blob(raspberry_repo, raspberry["commit"], raspberry_config["path"])
    if hashlib.sha256(raspberry_data).hexdigest() != raspberry_config["sha256"]:
        fail("Raspberry config base SHA-256 does not match the manifest")
def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--orange-source")
    parser.add_argument("--raspberry-source")
    parser.add_argument("--armbian-source")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    try:
        manifest, patch_root, order = load_manifest(root)
        validate_follow_up_patch(patch_root / order[1])
        with tempfile.TemporaryDirectory(prefix="octessera-midi-source-gate-") as temp:
            temp_root = Path(temp)
            kernels = manifest["kernels"]
            orange_repo = prepare_repo(
                temp_root,
                "orange-kernel",
                kernels["orange"]["repository"],
                kernels["orange"]["commit"],
                args.orange_source,
            )
            raspberry_repo = prepare_repo(
                temp_root,
                "raspberry-kernel",
                kernels["raspberry"]["repository"],
                kernels["raspberry"]["commit"],
                args.raspberry_source,
            )
            armbian = manifest["build_frameworks"]["armbian"]
            armbian_repo = prepare_repo(
                temp_root,
                "armbian-build",
                armbian["repository"],
                armbian["commit"],
                args.armbian_source,
            )
            _, core_regular = inspect_armbian_order(manifest, armbian_repo)
            validate_regular_patch_order(patch_root, order, core_regular)
            validate_config_bases(manifest, armbian_repo, raspberry_repo)
            for target, repo in (("orange", orange_repo), ("raspberry", raspberry_repo)):
                source = temp_root / f"{target}-staged"
                stage_source(repo, kernels[target]["commit"], source)
                for label, patch_name in zip(("accepted upstream patch", "Octessera safety patch"), order):
                    apply_patch(source, patch_root / patch_name, label, target)
                validate_final_source(source, target)
    except GateFailure as error:
        print(f"Orange MIDI kernel source gate failed: {error}", file=sys.stderr)
        return 1
    print("Orange MIDI kernel source gate passed for Orange and Raspberry pinned sources")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
