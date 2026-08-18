#!/usr/bin/env python3

RASPBERRY_PROFILE = "raspberry-pi-zero-2w"
ORANGE_PROFILE = "orange-pi-zero-2w"

UPDATER_ASSET_PROFILES = {
    RASPBERRY_PROFILE: {
        "archive": "octessera-{version}-{profile}-device-aarch64.zip",
        "checksums": "SHA256SUMS-{profile}-device.txt",
    },
    ORANGE_PROFILE: {
        "archive": "octessera-{version}-{profile}-runtime-updater-aarch64.zip",
        "checksums": "SHA256SUMS-{profile}-runtime-updater.txt",
    },
}


def updater_asset_names(profile: str, version: str) -> tuple[str, str]:
    try:
        contract = UPDATER_ASSET_PROFILES[profile]
    except KeyError as error:
        raise ValueError(f"Unsupported updater board profile: {profile}") from error
    return (
        contract["archive"].format(profile=profile, version=version),
        contract["checksums"].format(profile=profile, version=version),
    )
