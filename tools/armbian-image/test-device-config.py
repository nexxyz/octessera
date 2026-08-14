#!/usr/bin/env python3
import importlib.util
import json
import subprocess
import sys
import tempfile
from importlib.machinery import SourceFileLoader
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/lib/octessera/device_config.py"
STAGER = ROOT / "tools/armbian-image/stage-device-config.py"


def load(path, name):
    spec = importlib.util.spec_from_loader(name, SourceFileLoader(name, str(path)))
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_helper(path, index):
    helper = load(path, f"device_config_{index}")
    with tempfile.TemporaryDirectory() as directory:
        config = Path(directory) / "default.json"
        cases = (
            ({"dac": True, "usb": False, "hdmi": False}, False, (True, False, False, False)),
            ({"dac": False, "usb": True, "hdmi": False}, True, (False, True, False, True)),
            ({"dac": False, "usb": False, "hdmi": True}, False, (False, False, True, False)),
            ({"dac": True, "usb": True, "hdmi": True}, True, (True, True, True, True)),
        )
        for audio, midi, expected in cases:
            config.write_text(json.dumps({"runtimeConfig": {"audioOutputs": audio, "usb": {"midiOutEnabled": midi}}}), encoding="utf-8")
            assert tuple(helper.load_config(config).values()) == expected

        config.write_text('{"runtimeConfig":{"usb":{"audioOut":"both"}}}', encoding="utf-8")
        assert helper.load_config(config)["usb"] is True
        invalid = (
            {"runtimeConfig": {"audioOutputs": {"dac": False, "usb": False, "hdmi": False}}},
            {"runtimeConfig": {"audioOutputs": {"dac": True, "usb": False, "hdmi": False, "extra": False}}},
            {"runtimeConfig": {"audioOutputs": {"dac": True, "usb": False, "hdmi": False}, "usb": {"audioOut": "usb"}}},
            {"runtimeConfig": {"audioOutputs": {"dac": True, "usb": False, "hdmi": False}, "usb": {"midiOutEnabled": 1}}},
        )
        for payload in invalid:
            config.write_text(json.dumps(payload), encoding="utf-8")
            try:
                helper.load_config(config)
            except helper.ConfigError:
                pass
            else:
                raise AssertionError(payload)

        config.write_text('{"runtimeConfig":{"audioOutputs":{"dac":true,"usb":false,"hdmi":false},"audioOutputs":{"dac":true,"usb":false,"hdmi":false}}}', encoding="utf-8")
        try:
            helper.load_config(config)
        except helper.ConfigError:
            pass
        else:
            raise AssertionError("duplicate key accepted")


with tempfile.TemporaryDirectory() as staging:
    subprocess.run([sys.executable, str(STAGER), staging], check=True)
    staged = Path(staging) / "usr/local/lib/octessera/device_config.py"
    assert SOURCE.read_bytes() == staged.read_bytes()
    test_helper(SOURCE, 0)
    test_helper(staged, 1)

print("strict device config validator tests passed")
