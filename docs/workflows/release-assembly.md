# Release assembly

The release artifact entrypoint is `.github/workflows/release-artifacts.yml`.
It builds the custom GitHub release files listed below.

Published versions, tags, and assets are immutable. New bytes require a new
exact semantic version; do not replace bytes for an existing version.

## Custom release assets

The current contract contains exactly fourteen custom root files:

- `octessera-<version>-windows-installer.exe`
- `octessera-<version>-windows-portable.zip` with the legal notice bundle
- `octessera-<version>-ubuntu-amd64.deb`
- `octessera-<version>-ubuntu-x86_64.AppImage`
- `octessera-<version>-raspberry-pi-zero-2w.img.zip` with the Imager manifest
- `octessera-<version>-raspberry-pi-zero-2w.rpi-imager-manifest`
- `octessera-<version>-raspberry-pi-zero-2w-device-aarch64.zip`
- `SHA256SUMS-raspberry-pi-zero-2w-device.txt` for existing Raspberry clients
- `octessera-<version>-orange-pi-zero-2w.img.xz`
- `octessera-<version>-orange-pi-zero-2w-standalone-manual-aarch64.zip`
- `octessera-<version>-orange-pi-zero-2w-runtime-updater-aarch64.zip`
- `SHA256SUMS-orange-pi-zero-2w-runtime-updater.txt`
- `octessera-<version>-release-evidence.zip`
- `SHA256SUMS.txt`, lowercase and sorted

The Raspberry updater ZIP contains exactly `octessera-pi`,
`octessera-device-release.json`, `LICENSE`, and `NOTICE`. The Orange standalone
manual ZIP contains `octessera-pi`, `octessera-runtime.json`, `SHA256SUMS`,
`octessera-device-release.json`, `LICENSE`, and `NOTICE`; it is not an OTA
asset. The Orange runtime-updater ZIP contains exactly
`octessera-pi`, `octessera-device-release.json`, `LICENSE`, and `NOTICE`.

GitHub's automatic source archives are not custom assets and are not in
`SHA256SUMS.txt`. The release workflow generates the release-evidence archive
as one of the custom files above.

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

Raspberry updates use `/usr/local/sbin/octessera-update` and the profile-qualified
device ZIP/checksum. Candidates are staged under
`/opt/octessera/releases/<version>` and guarded Apply/rollback verifies service
identity, restart, stability, and profile readiness. Orange Check/Apply/Rollback
uses the root-owned broker and guarded updater, accepting only the Orange
runtime-updater ZIP/checksum pair. It updates only the managed runtime release
and binary link. Full Armbian, kernel, device-tree, and image replacement is
manual; missing or mismatched profile, asset, manifest, checksum, or health
precondition fails closed. Orange never consumes Raspberry assets or falls back
to the manual ZIP or full-image path.

Image release materials must account for source duties covering pinned upstream
inputs and Octessera source, patches, configuration, and build scripts. Keep
[`NOTICE`](../../NOTICE),
[`THIRD_PARTY_NOTICES.md`](../../THIRD_PARTY_NOTICES.md),
[`hardware/ATTRIBUTIONS.md`](../../hardware/ATTRIBUTIONS.md), and
[`samples/SOURCE.md`](../../samples/SOURCE.md) with the release materials.
