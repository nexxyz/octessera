#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
config="$root/userpatches/overlay/etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf"
customize="$root/userpatches/customize-image.sh"
runtime_assets="$root/userpatches/overlay/usr/local/lib/octessera/orange-runtime-assets-install.sh"

python3 - "$root" "$config" "$customize" "$runtime_assets" <<'PY'
from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path


root, config_path, customize_path, runtime_assets_path = map(Path, sys.argv[1:])
spam = "sun8i-dw-hdmi 6000000.hdmi: EVENT=plugin"
expected_config = f'if ($msg == "{spam}") then stop\n'
assert config_path.is_file() and not config_path.is_symlink()
assert config_path.read_text(encoding="utf-8") == expected_config
assert config_path.name.startswith("00-") and config_path.name < "50-default.conf"
assert sorted(path.name for path in config_path.parent.glob("*.conf")) == [config_path.name]
construction = json.loads((root / "resources/image-construction/boot-layers/orange-pi-zero-2w.json").read_text(encoding="utf-8"))
assert next(item for item in construction["managed_outputs"] if item["path"] == "etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf") == {"path": "etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf", "mode": 420, "uid": 0, "gid": 0}

customize = customize_path.read_text(encoding="utf-8")
runtime_assets = runtime_assets_path.read_text(encoding="utf-8")
assert 'install_overlay_file etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf /etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf 0644' in runtime_assets
assert "octessera_validate_orange_rsyslog_configuration()" in runtime_assets
assert 'validation_config="$(mktemp /tmp/octessera-rsyslog-validation.XXXXXX)"' in runtime_assets
assert 'global(net.enableDNS="off")' in runtime_assets
assert 'include(file="/etc/rsyslog.conf")' in runtime_assets
assert 'if printf \'%s\\n\' \'global(net.enableDNS="off")\' \'include(file="/etc/rsyslog.conf")\' > "$validation_config"; then' in runtime_assets
assert 'rsyslogd -N1 -f "$validation_config"' in runtime_assets
assert 'validation_status=$?' in runtime_assets
assert 'if rm -f -- "$validation_config"; then' in runtime_assets
assert 'return "$validation_status"' in runtime_assets
assert "rsyslogd -N1 -f /etc/rsyslog.conf" not in runtime_assets
for forbidden in ("rsyslogd -x", "/etc/hosts", "/etc/hostname", "hostnamectl"):
    assert forbidden not in runtime_assets
assert not (root / "userpatches/overlay/etc/rsyslog.conf").exists()
assert not (root / "userpatches/overlay/etc/systemd/journald.conf").exists()
for forbidden in ("SystemMaxUse", "RuntimeMaxUse", "RateLimit", "rateLimit", "Storage="):
    assert forbidden not in customize
    assert forbidden not in runtime_assets
    assert forbidden not in config_path.read_text(encoding="utf-8")

neighbors = [
    "sun8i-dw-hdmi 6000000.hdmi: EVENT=plugout",
    "sun8i-dw-hdmi 6000000.hdmi: ERROR=edid-read",
    "sun8i-dw-hdmi 6000000.hdmi: EVENT=plugin extra",
    "sun8i-dw-hdmi 6000000.hdmi: EVENT=plugin ",
]
with tempfile.TemporaryDirectory(prefix="octessera-orange-hdmi-rsyslog-") as temporary:
    work = Path(temporary)
    fixture = work / "input.log"
    output = work / "output.log"
    with fixture.open("w", encoding="utf-8") as stream:
        for _ in range(180_000):
            stream.write(spam + "\n")
        for message in neighbors:
            stream.write(message.rstrip("\n") + "\n")

    with fixture.open(encoding="utf-8") as stream:
        kept = [line.rstrip("\n") for line in stream if line.rstrip("\n") != spam]
    output.write_text("".join(f"{line}\n" for line in kept), encoding="utf-8")
    assert kept == [message.rstrip("\n") for message in neighbors]
    assert all(line != spam for line in output.read_text(encoding="utf-8").splitlines())
    assert output.stat().st_size < 50 * 1024 * 1024

    rsyslogd = shutil.which("rsyslogd")
    if rsyslogd:
        actual_output = work / "rsyslog-output.log"
        state = work / "state"
        state.mkdir()
        rsyslog_config = work / "rsyslog.conf"
        rsyslog_config.write_text(
            "\n".join(
                [
                    f'global(workDirectory="{state}")',
                    'module(load="imfile" PollingInterval="1")',
                    'template(name="fixture" type="string" string="%msg%\\n")',
                    f'input(type="imfile" File="{fixture}" Tag="fixture:" Facility="kern" Severity="info" freshStartTail="off")',
                    config_path.read_text(encoding="utf-8").rstrip("\n"),
                    f'*.* action(type="omfile" file="{actual_output}" template="fixture")',
                    "",
                ]
            ),
            encoding="utf-8",
        )
        syntax = subprocess.run([rsyslogd, "-N1", "-f", str(rsyslog_config)], capture_output=True, text=True)
        if syntax.returncode != 0:
            raise AssertionError(f"rsyslogd fixture syntax check failed: {syntax.stdout}{syntax.stderr}")

        malformed_include = work / "malformed.conf"
        malformed_include.write_text('global(net.enableDNS="off"\n', encoding="utf-8")
        malformed_wrapper = work / "malformed-wrapper.conf"
        malformed_wrapper.write_text(
            "\n".join(
                [
                    'global(net.enableDNS="off")',
                    f'include(file="{malformed_include}")',
                    "",
                ]
            ),
            encoding="utf-8",
        )
        malformed = subprocess.run([rsyslogd, "-N1", "-f", str(malformed_wrapper)], capture_output=True, text=True)
        assert malformed.returncode != 0, f"rsyslogd accepted malformed included config: {malformed.stdout}{malformed.stderr}"

        process = subprocess.Popen([rsyslogd, "-n", "-f", str(rsyslog_config), "-i", str(work / "rsyslog.pid")], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        deadline = time.monotonic() + 20
        expected_output = [message.rstrip("\n") for message in neighbors]
        try:
            while time.monotonic() < deadline:
                if actual_output.is_file() and actual_output.read_text(encoding="utf-8").splitlines() == expected_output:
                    break
                if process.poll() is not None:
                    stdout, stderr = process.communicate()
                    raise AssertionError(f"rsyslogd fixture exited early: {stdout}{stderr}")
                time.sleep(0.1)
            else:
                stdout, stderr = process.communicate(timeout=1)
                raise AssertionError(f"rsyslogd fixture did not finish: {stdout}{stderr}")
            assert actual_output.read_text(encoding="utf-8").splitlines() == expected_output
            assert all(line != spam for line in actual_output.read_text(encoding="utf-8").splitlines())
            assert actual_output.stat().st_size < 50 * 1024 * 1024
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=5)
        print("Orange HDMI rsyslog source, exact-filter fixture, and rsyslogd integration checks passed")
    else:
        print("Orange HDMI rsyslog source and exact-filter fixture checks passed; rsyslogd integration skipped")
PY
