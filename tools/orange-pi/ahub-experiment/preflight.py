#!/usr/bin/env python3
import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys

EXPECTED_COMMIT = "166b786fc978d88f4ff9ee3e33c353afb39763e8"
EXPECTED_REPOSITORY = "../../../.slim/clonedeps/repos/armbian__build"
EXPECTED_KERNEL_PATCH_DIR = "patch/kernel/archive/sunxi-6.12"
EXPECTED_STAGED_PATCH_DIR = "archive/sunxi-6.12"
EXPECTED_SERIES = {
    "path": "patch/kernel/archive/sunxi-6.12/series.conf",
    "sha256": "256c5db677c7495281bf31258c3f2e10ee1464db31abbb27287f5e56036a843a",
    "patch_count": 458,
    "manifest_sha256": "52916c95aa5935d50bccbdb4d28c0a77782092d324af4211989769e6e1db5804",
}
EXPECTED_REQUIRED_SERIES = {
    "patches.armbian/arm64-dts-h616-add-hdmi-support-for-zero2-and-zero3.patch",
    "patches.armbian/Sound-for-H616-H618-Allwinner-SOCs.patch",
    "patches.armbian/sun50i-h616-Add-the-missing-digital-audio-nodes.patch",
}
EXPECTED_SOURCE = [
    ("config/kernel/linux-sunxi64-current.config", "kernel-config", "07a1a6f808df491bfffb9dac16bb6a41d47567be991f5f6decd0d40480320a2f"),
]
EXPECTED_ASSETS = [
    "README.md", "check-patch-stack.sh", "Kconfig.fragment", "h618-fixture-base.dts",
    "octessera-ahub0-pi123-overlay.dts", "build-hooks/normalize-kernel-package-input.patch",
    "preflight.py", "validate-fixture.sh",
    "deploy-rollback.sh", "build-ahub-experiment.sh", "test-validate.sh", "kernel-build-plan.json",
    "runtime-fixture/running-kernel.config", "runtime-fixture/asoc-registration.txt",
    "runtime-fixture/deferred-probes.txt",
]
REQUIRED_CONFIG = {
    "CONFIG_ARCH_SUNXI": "y", "CONFIG_SOUND": "y", "CONFIG_SND": "y",
    "CONFIG_SND_SOC": "y", "CONFIG_REGMAP_MMIO": "y", "CONFIG_SUNXI_SYS_INFO": "n",
    "CONFIG_SND_SOC_GENERIC_DMAENGINE_PCM": "y",
    "CONFIG_SND_SOC_SUNXI_AHUB": "y", "CONFIG_SND_SOC_SUNXI_AHUB_DAM": "y",
    "CONFIG_SND_SOC_SUNXI_MACH": "y", "CONFIG_SND_SOC_PCM5102A": "y",
}
EXPECTED_BASE_CONFIG = {
    "CONFIG_ARCH_SUNXI": "y", "CONFIG_SOUND": "y", "CONFIG_SND": "y",
    "CONFIG_SND_SOC": "y",
}


def fail(message):
    raise SystemExit(f"AHUB preflight: {message}")


def require_keys(value, expected, name):
    if not isinstance(value, dict):
        fail(f"{name} is not an object")
    actual = set(value)
    if actual != set(expected):
        fail(f"{name} fields differ; unknown={sorted(actual - set(expected))}, missing={sorted(set(expected) - actual)}")


def pairs(values):
    result = {}
    for key, value in values:
        if key in result:
            fail(f"duplicate lock field: {key}")
        result[key] = value
    return result


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git_blobs(source_dir, paths):
    requests = b"".join(f"{EXPECTED_COMMIT}:{path}\n".encode() for path in paths)
    output = subprocess.check_output(["git", "-C", str(source_dir), "cat-file", "--batch"], input=requests)
    result = {}
    offset = 0
    for path in paths:
        end = output.index(b"\n", offset)
        header = output[offset:end].split()
        offset = end + 1
        if len(header) != 3 or header[1] != b"blob":
            fail(f"source path is not a tracked blob: {path}")
        size = int(header[2])
        content = output[offset:offset + size]
        result[path] = content
        offset += size + 1
    return result


def safe_path(value, name):
    if not isinstance(value, str) or not value or pathlib.PurePosixPath(value).is_absolute() or "\\" in value:
        fail(f"unsafe {name}")
    if any(part in ("", ".", "..") for part in value.split("/")):
        fail(f"unsafe {name}")
    return value


def parse_series(content):
    paths = []
    for line in content.decode("utf-8").splitlines():
        value = line.strip()
        if not value or value.startswith("#") or value.startswith("-"):
            continue
        paths.append(safe_path(value, "series patch path"))
    return paths


def source_config(content):
    values = {}
    for line in content.decode("utf-8").splitlines():
        match = re.fullmatch(r"(CONFIG_[A-Za-z0-9_]+)=(\S.*)", line)
        if match:
            values[match.group(1)] = match.group(2)
    return values


def exact_config(path):
    values = {}
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        match = re.fullmatch(r"(CONFIG_[A-Za-z0-9_]+)=(y|m|n)", line)
        if match is None:
            fail(f"invalid or duplicate build config entry at line {number}: {path}")
            continue
        if match.group(1) in values:
            fail(f"invalid or duplicate build config entry at line {number}: {path}")
        values[match.group(1)] = match.group(2)
    return values


def main():
    fixture_dir = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else pathlib.Path(__file__).parent).resolve()
    lock = {}
    try:
        lock = json.loads((fixture_dir / "stack-lock.json").read_text(encoding="utf-8"), object_pairs_hook=pairs)
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot parse stack-lock.json: {error}")
    require_keys(lock, ["schema", "source", "assets", "fixture", "deploy"], "lock")
    require_keys(lock["source"], ["repository", "commit", "kernel_patch_dir", "staged_patch_dir", "files", "series"], "source")
    require_keys(lock["fixture"], ["base", "overlay", "kconfig", "preserve_nodes", "expected_root_children"], "fixture")
    require_keys(lock["deploy"], ["board", "dtb_path", "overlay_path", "overlay_name", "env_path", "compatible"], "deploy")
    if lock["schema"] != 1 or lock["source"]["repository"] != EXPECTED_REPOSITORY or lock["source"]["commit"] != EXPECTED_COMMIT:
        fail("source repository or commit is not pinned")
    if lock["source"]["kernel_patch_dir"] != EXPECTED_KERNEL_PATCH_DIR or lock["source"]["staged_patch_dir"] != EXPECTED_STAGED_PATCH_DIR:
        fail("source kernel patch directory is not pinned to archive/sunxi-6.12")
    if lock["source"]["series"] != EXPECTED_SERIES:
        fail("full source series lock changed")

    source_override = os.environ.get("ARMBIAN_SOURCE_DIR")
    source_dir = pathlib.Path(source_override).resolve() if source_override else (fixture_dir / EXPECTED_REPOSITORY).resolve()
    root = pathlib.Path()
    head = ""
    try:
        root = pathlib.Path(subprocess.check_output(["git", "-C", str(source_dir), "rev-parse", "--show-toplevel"], text=True, stderr=subprocess.STDOUT).strip()).resolve()
        head = subprocess.check_output(["git", "-C", str(source_dir), "rev-parse", "HEAD"], text=True, stderr=subprocess.STDOUT).strip()
    except (OSError, subprocess.CalledProcessError) as error:
        fail(f"cannot inspect Armbian source: {error}")
    if root != source_dir or head != EXPECTED_COMMIT:
        fail("Armbian source is not at the pinned commit")

    source_files = lock["source"]["files"]
    if source_files != [{"path": EXPECTED_SOURCE[0][0], "role": EXPECTED_SOURCE[0][1], "sha256": EXPECTED_SOURCE[0][2]}]:
        fail("source file lock is incomplete")
    series_path = source_dir / EXPECTED_SERIES["path"]
    config_path = source_dir / EXPECTED_SOURCE[0][0]
    if not series_path.is_file() or not config_path.is_file():
        fail("pinned source files are missing")
    source_blobs = git_blobs(source_dir, [EXPECTED_SERIES["path"], EXPECTED_SOURCE[0][0]])
    series_content = source_blobs[EXPECTED_SERIES["path"]]
    config_content = source_blobs[EXPECTED_SOURCE[0][0]]
    if hashlib.sha256(config_content).hexdigest() != EXPECTED_SOURCE[0][2]:
        fail("source SHA-256 mismatch: kernel config")
    if hashlib.sha256(series_content).hexdigest() != EXPECTED_SERIES["sha256"]:
        fail("source SHA-256 mismatch: series.conf")
    series_paths = parse_series(series_content)
    if len(series_paths) != EXPECTED_SERIES["patch_count"] or not EXPECTED_REQUIRED_SERIES.issubset(series_paths):
        fail("full series order or required AHUB patches changed")
    patch_paths = [f"{EXPECTED_KERNEL_PATCH_DIR}/{path}" for path in series_paths]
    blobs = git_blobs(source_dir, patch_paths)
    manifest = b"".join((path + "\t" + hashlib.sha256(blobs[path]).hexdigest() + "\n").encode() for path in patch_paths)
    if hashlib.sha256(manifest).hexdigest() != EXPECTED_SERIES["manifest_sha256"]:
        fail("full series patch order or source hashes changed")

    assets = lock["assets"]
    if not isinstance(assets, list) or [item.get("path") for item in assets if isinstance(item, dict)] != EXPECTED_ASSETS:
        fail("asset lock is not the exact fixture asset list")
    for item in assets:
        require_keys(item, ["path", "sha256"], "asset")
        path = fixture_dir / safe_path(item["path"], "asset path")
        if not path.is_file() or digest(path) != item["sha256"]:
            fail(f"asset SHA-256 mismatch: {item['path']}")

    expected_fixture = {
        "base": "h618-fixture-base.dts", "overlay": "octessera-ahub0-pi123-overlay.dts", "kconfig": "Kconfig.fragment",
        "preserve_nodes": ["/serial@5000000", "/serial@5002000", "/spi@5010000", "/i2c@5003000", "/hdmi@6000000", "/codec@5096000", "/main-encoder", "/ahub1_plat", "/ahub1_mach"],
        "expected_root_children": ["ahub1_mach", "ahub1_plat", "ahub_dam_mach", "ahub_dam_plat", "aliases", "chosen", "codec@5096000", "dma-controller@3002000", "hdmi@6000000", "i2c@5003000", "main-encoder", "octessera-dac", "octessera_plat", "pcm5102a", "pinctrl@300b000", "serial@5000000", "serial@5002000", "spi@5010000"],
    }
    expected_deploy = {
        "board": "orangepizero2w", "dtb_path": "/boot/dtb/allwinner/sun50i-h618-orangepi-zero2w.dtb",
        "overlay_path": "/boot/dtb/allwinner/overlay/octessera-ahub0-pcm5102.dtbo", "overlay_name": "octessera-ahub0-pcm5102",
        "env_path": "/boot/armbianEnv.txt", "compatible": ["xunlong,orangepi-zero2w", "allwinner,sun50i-h618"],
    }
    if lock["fixture"] != expected_fixture or lock["deploy"] != expected_deploy:
        fail("fixture or deploy identity facts changed")

    build_config = exact_config(fixture_dir / "Kconfig.fragment")
    if build_config != REQUIRED_CONFIG or (fixture_dir / "Kconfig.fragment").read_text(encoding="utf-8").splitlines() != [f"{key}={value}" for key, value in REQUIRED_CONFIG.items()]:
        fail("kernel build config does not force the exact built-in AHUB/PCM5102 closure")
    if build_config.get("CONFIG_SUNXI_SYS_INFO") != "n" or "CONFIG_NVMEM_SUNXI_SID" in build_config:
        fail("experimental config carries the unrelated sysinfo/SID dependency")
    config = source_config(config_content)
    for key, value in EXPECTED_BASE_CONFIG.items():
        if config.get(key) != value:
            fail(f"Kconfig closure is missing {key}={value}")
    if config.get("CONFIG_SND_SOC_PCM5102A") == "y":
        fail("PCM5102A must be supplied by the experimental built-in config")

    plan = json.loads((fixture_dir / "kernel-build-plan.json").read_text(encoding="utf-8"), object_pairs_hook=pairs)
    require_keys(plan, ["schema", "board", "branch", "source_commit", "kernel_patch_dir", "source_series", "source_series_patch_count", "kernel_config", "dtb", "overlay", "package_input_hook", "package_input_hook_target", "kernel_package_glob", "runtime_output_validator", "required_config"], "kernel build plan")
    expected_plan = {
        "schema": 1, "board": "orangepizero2w", "branch": "current", "source_commit": EXPECTED_COMMIT,
        "kernel_patch_dir": EXPECTED_STAGED_PATCH_DIR, "kernel_config": "Kconfig.fragment",
        "source_series": "patch/kernel/archive/sunxi-6.12/series.conf", "source_series_patch_count": 458,
        "dtb": "sun50i-h618-orangepi-zero2w.dtb", "overlay": "octessera-ahub0-pi123-overlay.dts",
        "package_input_hook": "build-hooks/normalize-kernel-package-input.patch",
        "package_input_hook_target": "lib/functions/compilation/kernel-debs.sh",
        "kernel_package_glob": "linux-image-*.deb", "runtime_output_validator": "one-nonempty-linux-image-package",
        "required_config": REQUIRED_CONFIG,
    }
    if plan != expected_plan:
        fail("kernel build plan is not pinned to the full built-in AHUB experiment")
    if exact_config(fixture_dir / "runtime-fixture/running-kernel.config") != REQUIRED_CONFIG:
        fail("running-kernel.config does not prove built-in PCM5102/AHUB support")
    package_hook = (fixture_dir / "build-hooks/normalize-kernel-package-input.patch").read_text(encoding="utf-8")
    for fact in ["function ahub_normalize_kernel_package_inputs()", "ahub_normalize_kernel_package_inputs", "for stem in vmlinuz config System.map", "${expected_path}-dirty"]:
        if fact not in package_hook:
            fail(f"package input hook is missing required fact: {fact}")
    asoc_log = (fixture_dir / "runtime-fixture/asoc-registration.txt").read_text(encoding="utf-8")
    for fact in ["snd_soc_register_card: octessera-dac", "sunxi-snd-mach: card=octessera-dac cpu=octessera_plat codec=pcm5102a", "pcm5102a-codec: ti,pcm5102a registered", "sunxi-snd-mach: card=HDMI cpu=ahub1_plat codec=hdmi registered"]:
        if fact not in asoc_log:
            fail(f"required ASoC runtime fact is missing: {fact}")
    if (fixture_dir / "runtime-fixture/deferred-probes.txt").read_text(encoding="utf-8").strip() != "deferred_probe_count=0":
        fail("deferred-probe fixture is not empty")

    overlay = (fixture_dir / expected_fixture["overlay"]).read_text(encoding="utf-8")
    if "/dts-v1/;" not in overlay or "/plugin/;" not in overlay:
        fail("overlay is not a DTS plugin")
    if re.search(r"(?i)\bpi0\b|pin\s*29", overlay) or re.search(r"(?i)mclk", overlay):
        fail("overlay claims PI0, physical pin 29, or MCLK")
    if re.findall(r"pins\s*=\s*([^;]+);", overlay) != ['"PI1", "PI2", "PI3"'] or re.findall(r'"PI[0-9]+"', overlay) != ['"PI1"', '"PI2"', '"PI3"']:
        fail("overlay is not PI1/PI2/PI3-only")
    if re.findall(r"fragment@(\d+)", overlay) != ["0", "1"] or re.findall(r"target\s*=\s*<\s*&([A-Za-z0-9_]+)\s*>;", overlay) != ["pio"] or overlay.count('target-path = "/";') != 1:
        fail("overlay target set changed")
    if re.search(r"&(?:spi|i2c|uart|hdmi|codec)[A-Za-z0-9_]*", overlay) or "delete-node" in overlay or "delete-property" in overlay:
        fail("overlay modifies a preserved node or contains a deletion")
    if overlay.count('compatible = "ti,pcm5102a";') != 1 or overlay.count('soundcard-mach,name = "octessera-dac";') != 1 or overlay.count('compatible = "allwinner,sunxi-snd-mach";') != 1:
        fail("overlay card identity is not stable")
    if overlay.count("sound-dai = <&octessera_plat>;") != 1 or overlay.count("sound-dai = <&pcm5102a>;") != 1 or overlay.count("apb_num = <0>;") != 1 or overlay.count("dmas = <&dma 3>, <&dma 3>;") != 1 or overlay.count("tdm_num = <0>;") != 1 or overlay.count("tx_pin = <0>;") != 1 or "&ahub1_plat" in overlay or "&ahub1_mach" in overlay:
        fail("overlay does not keep AHUB0 separate from HDMI AHUB1")
    base = (fixture_dir / expected_fixture["base"]).read_text(encoding="utf-8")
    for required in ["ahub_dam_plat:", "ahub_dam_mach:", "ahub1_plat:", "ahub1_mach:", "hdmi:", "codec:", "main_encoder:", "uart0:", "uart2:", "spi0:", "i2c0:", "pio:", "dma:"]:
        if required not in base:
            fail(f"fixture is missing binding or preserved node: {required}")
    print("AHUB full source series, lock, overlay, and preflight checks passed")


if __name__ == "__main__":
    main()
