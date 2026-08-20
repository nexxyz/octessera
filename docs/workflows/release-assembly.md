# Release assembly

Explicit GitHub releases are built only by
`.github/workflows/release-artifacts.yml`. Tag pushes and intermediate CI builds
must not publish release assets. Stop at a populated draft; publication is a
separate human decision described in the [release support matrix](../../userdocs/release-support.md).

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

macOS distribution is paused until it can be signed and notarized. GitHub's
automatic source archives are not custom assets and are not in
`SHA256SUMS.txt`. The final gate checks the portable notice proof, ZIP contents
and modes, image/kernel evidence, runtime identity, sample/default coverage,
and root names/checksums.

## Owner handoff

1. Bump versions in Rust manifests, `package.json` files, and
   `apps/desktop/src-tauri/tauri.conf.json`; run `corepack pnpm install`.
2. Run local validation and rebuild the portable desktop executable when
   desktop-visible behavior changed.
3. Commit and push release-prep changes.
4. Create a unique empty draft release such as `v0.5.0`.
5. Run `Release Artifacts` manually with that tag. The workflow checks the tag
   semver against package metadata.
6. Stop at the populated draft. Use the [release support checklist](../../userdocs/release-support.md)
   to verify names/count, checksums, manifests, ZIP contents, samples,
   desktop launch, per-board FAT, source duties, and limitations. Do not
   announce or publish until a human explicitly makes that decision.

For the v0.8.1 draft, retain Raspberry mounted-image and kernel proof with the
exact draft and image SHA. Constructor evidence exists for both boards, but
physical FAT remains the retirement gate. Trusted-parent machinery is frozen
legacy recovery and is not a v0.8.1 qualification path.

## Image staging and update boundaries

Before a local Raspberry constructor run, use the canonical checkout and stage
notices into the disposable stage4 tree:

```bash
export OCTESSERA_REPOSITORY_ROOT="$PWD"
sudo python3 tools/legal/stage_notices.py \
  --repository-root "$OCTESSERA_REPOSITORY_ROOT" \
  --destination-root tools/pi-image/stage4-octessera/files/root
```

Remove the generated `usr/share/doc/octessera/` tree after the local run;
workflows stage it only in a disposable checkout. Orange staging follows the
same manifest-driven `OCTESSERA_REPOSITORY_ROOT` pattern. Both fixed image paths
install the inactive Wi-Fi foundation, a root-owned Wi-Fi-only wrapper fixed to
`wlan0` and `192.168.42.1`; it is deliberately disabled and does not serialize
credentials or add runtime behavior.

Release images must contain no Wi-Fi credentials, SSH keys, GitHub tokens, host
logs, or local user secrets. SSH is disabled by default.

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

Before a future public board-image release, review source duties for pinned
upstream inputs and Octessera source, patches, configuration, and build scripts
in [`../release-licensing.md`](../release-licensing.md).
