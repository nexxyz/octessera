# Release assembly

The release artifact entrypoint is `.github/workflows/release-artifacts.yml`.
Create a draft GitHub release for an exact `vX.Y.Z` tag first, with no assets
attached, then dispatch the workflow with that tag:

```bash
gh workflow run release-artifacts.yml -f tag=v<x.y.z>
```

It checks that the tag commit and every application version agree, builds the
desktop artifacts, and calls `release-board-artifacts.yml` with the `tag`,
`version`, and `source_sha` for both board runtimes, images, and device ZIPs.
It then assembles and verifies the custom files listed below, uploads them, and
leaves the release as a draft. Checking the downloaded bytes, the
exact-artifact hardware test from the
[manual hardware test suite](../../hardware/docs/manual-hardware-test-suite.md),
and pressing Publish are still yours to do.

Published versions, tags, and assets are immutable. New bytes require a new
exact semantic version; do not replace bytes for an existing version.

## Custom release assets

The current contract contains exactly thirteen custom root files:

- `octessera-<version>-windows-installer.exe`
- `octessera-<version>-windows-portable.zip` with the legal notice bundle
- `octessera-<version>-ubuntu-amd64.deb`
- `octessera-<version>-ubuntu-x86_64.AppImage`
- `octessera-<version>-raspberry-pi-zero-2w.img.zip` with the Imager manifest
- `octessera-<version>-raspberry-pi-zero-2w.rpi-imager-manifest`
- `octessera-<version>-raspberry-pi-zero-2w-device-aarch64.zip`
- `SHA256SUMS-raspberry-pi-zero-2w-device.txt`
- `octessera-<version>-orange-pi-zero-2w.img.xz`
- `octessera-<version>-orange-pi-zero-2w-device-aarch64.zip`
- `SHA256SUMS-orange-pi-zero-2w-device.txt`
- `octessera-<version>-notices.zip`, the third-party licence notices
- `SHA256SUMS.txt`, lowercase and sorted

Each board's device ZIP is that board's runtime bundle (`octessera-pi`,
`octessera-runtime.json`, `SHA256SUMS`) plus `LICENSE` and `NOTICE`, built by
`tools/device-update/package_device_bundle.py`. The on-device updater downloads
it and checks it against the matching `SHA256SUMS-<profile>-device.txt`.

GitHub's automatic source archives are not custom assets and are not in
`SHA256SUMS.txt`.

## Image staging and update boundaries

Before a local Raspberry constructor run, use the canonical checkout and stage
notices into the disposable stage4 tree:

```bash
export OCTESSERA_REPOSITORY_ROOT="$PWD"
sudo python3 tools/legal/stage_notices.py \
  --repository-root "$OCTESSERA_REPOSITORY_ROOT" \
  --destination-root tools/pi-image/stage4-octessera/files/root
```

The generated `usr/share/doc/octessera/` tree is disposable. Orange staging
follows the same manifest-driven `OCTESSERA_REPOSITORY_ROOT` pattern. Both fixed
image paths install the inactive Wi-Fi foundation, a root-owned Wi-Fi-only
wrapper fixed to `wlan0` and `192.168.42.1`; it is deliberately disabled and
does not serialize credentials or add runtime behavior.

Release images must contain no Wi-Fi credentials, SSH keys, GitHub tokens, host
logs, or local user secrets. SSH is disabled by default. An unconfigured
Raspberry image retains its unmasked vendor SSH units for explicit Raspberry Pi
Imager customization. Imager may configure SSH, hostname, and Wi-Fi, but the
username must remain `pi`.

Both boards update with `/usr/local/sbin/octessera-update`. It picks the newest
published release that has this board's device ZIP, verifies the checksum,
unpacks it to `/opt/octessera/releases/<version>`, and starts
`octessera-update-switch@<version>.service`. The switch moves `current` (and
`previous`), restarts `octessera.service`, and swaps back if the runtime is not
active with its readiness marker within 60 seconds. It updates only the runtime
release; Armbian, kernel, device-tree, and image changes still need a reflash.

Image release materials must account for source duties covering pinned upstream
inputs and Octessera source, patches, configuration, and build scripts. Keep
[`NOTICE`](../../NOTICE),
[`THIRD_PARTY_NOTICES.md`](../../THIRD_PARTY_NOTICES.md),
[`hardware/ATTRIBUTIONS.md`](../../hardware/ATTRIBUTIONS.md), and
[`samples/upstream/LICENSE`](../../samples/upstream/LICENSE) with the release materials.
