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

    if "audioOutputs" not in runtime:
        raise ConfigError("audioOutputs is required")
    outputs = runtime["audioOutputs"]
    if not isinstance(outputs, dict) or set(outputs) != {"dac", "usb", "hdmi"}:
        raise ConfigError("audioOutputs must contain exactly dac, usb, and hdmi")
    if any(type(outputs[key]) is not bool for key in outputs):
        raise ConfigError("audioOutputs values must be boolean")
    if not any(outputs.values()):
        raise ConfigError("at least one audio output must be enabled")
    dac = outputs["dac"]
    usb_audio = outputs["usb"]
    hdmi = outputs["hdmi"]
    if not dac:
        raise ConfigError("Jack Audio is always on")

    if "usb" not in runtime:
        raise ConfigError("usb is required")
    usb = runtime["usb"]
    if not isinstance(usb, dict):
        raise ConfigError("usb must be an object")
    if set(usb) - {"midiOutEnabled", "dataRole"}:
        raise ConfigError("usb contains unsupported fields")
    if set(usb) != {"midiOutEnabled", "dataRole"}:
        raise ConfigError("usb must contain exactly midiOutEnabled and dataRole")
    if type(usb["midiOutEnabled"]) is not bool:
        raise ConfigError("usb.midiOutEnabled must be boolean")
    midi = usb["midiOutEnabled"]
    data_role = usb["dataRole"]
    if type(data_role) is not str or data_role not in ("gadget", "host"):
        raise ConfigError("usb.dataRole must be gadget or host")

    if data_role == "host" and (usb_audio or midi):
        raise ConfigError("host USB data role requires USB audio and MIDI output to be disabled")

    return {
        "dac": dac,
        "usb": usb_audio,
        "hdmi": hdmi,
        "midi": midi,
    }


def parse_data_role(payload):
    parse_config(payload)
    return payload["runtimeConfig"]["usb"]["dataRole"]


def load_config(path):
    return parse_config(_load_payload(path))


def load_data_role(path):
    return parse_data_role(_load_payload(path))


def _load_payload(path):
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
    return payload


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--data-role", action="store_true")
    parser.add_argument("path")
    args = parser.parse_args()
    if args.data_role:
        print(load_data_role(args.path))
    else:
        state = load_config(args.path)
        print(int(state["usb"]), int(state["midi"]))


if __name__ == "__main__":
    try:
        main()
    except (ConfigError, OSError) as error:
        print(f"invalid device config: {error}", file=sys.stderr)
        raise SystemExit(1)
