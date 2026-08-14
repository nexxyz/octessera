#!/usr/bin/env python3
import argparse
import json
import os
import stat
import sys


MAX_CONFIG_BYTES = 1024 * 1024


class ConfigError(ValueError):
    pass


def _object_pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ConfigError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def _reject_constant(value):
    raise ConfigError(f"invalid JSON constant: {value}")


def parse_config(payload):
    if not isinstance(payload, dict) or "runtimeConfig" not in payload:
        raise ConfigError("runtimeConfig must be present")
    runtime = payload["runtimeConfig"]
    if not isinstance(runtime, dict):
        raise ConfigError("runtimeConfig must be an object")

    canonical_present = "audioOutputs" in runtime
    outputs = runtime.get("audioOutputs")
    if canonical_present:
        if not isinstance(outputs, dict) or set(outputs) != {"dac", "usb", "hdmi"}:
            raise ConfigError("audioOutputs must contain exactly dac, usb, and hdmi")
        if any(type(outputs[key]) is not bool for key in outputs):
            raise ConfigError("audioOutputs values must be boolean")
        if not any(outputs.values()):
            raise ConfigError("at least one audio output must be enabled")
        dac = outputs["dac"]
        usb_audio = outputs["usb"]
        hdmi = outputs["hdmi"]
    else:
        dac = usb_audio = hdmi = None

    usb = runtime.get("usb")
    if "usb" not in runtime:
        usb = {}
    if not isinstance(usb, dict):
        raise ConfigError("usb must be an object")
    if "midiOutEnabled" in usb and type(usb["midiOutEnabled"]) is not bool:
        raise ConfigError("usb.midiOutEnabled must be boolean")
    midi = usb.get("midiOutEnabled", False)

    legacy_present = "audioOut" in usb
    if legacy_present:
        legacy = usb["audioOut"]
        if not isinstance(legacy, str) or legacy not in {"jack", "usb", "both"}:
            raise ConfigError("usb.audioOut must be jack, usb, or both")
        legacy_projection = {
            "jack": (True, False),
            "usb": (False, True),
            "both": (True, True),
        }[legacy]
        if canonical_present and (dac, usb_audio) != legacy_projection:
            raise ConfigError("canonical and legacy USB audio settings disagree")
        if not canonical_present:
            dac, usb_audio = legacy_projection
            hdmi = False

    if not canonical_present and not legacy_present:
        raise ConfigError("audioOutputs or legacy usb.audioOut is required")

    return {
        "dac": dac,
        "usb": usb_audio,
        "hdmi": hdmi,
        "midi": midi,
    }


def load_config(path):
    metadata = os.lstat(path)
    if not stat.S_ISREG(metadata.st_mode):
        raise ConfigError("config must be a regular file")
    if metadata.st_size > MAX_CONFIG_BYTES:
        raise ConfigError("config is too large")
    with open(path, "rb") as handle:
        raw = handle.read(MAX_CONFIG_BYTES + 1)
    if len(raw) > MAX_CONFIG_BYTES:
        raise ConfigError("config is too large")
    try:
        payload = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=_object_pairs,
            parse_constant=_reject_constant,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ConfigError(f"invalid JSON: {error}") from error
    return parse_config(payload)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("path")
    args = parser.parse_args()
    state = load_config(args.path)
    print(int(state["usb"]), int(state["midi"]))


if __name__ == "__main__":
    try:
        main()
    except (ConfigError, OSError) as error:
        print(f"invalid device config: {error}", file=sys.stderr)
        raise SystemExit(1)
