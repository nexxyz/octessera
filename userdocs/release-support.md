# Release support

This page is the small, honest answer to “which download is supported?” A
platform is supported only when the exact release asset, target platform, source
SHA, checksum, and manual FAT record are all named together. Source/build proof
is useful, but it is not physical qualification. Do not infer support from an
asset being present on a release page.

No open FAT result is closed by this documentation pass. Until a release record
marks an exact row **FAT-passed**, keep that row **UNQUALIFIED** or leave it out
of public support claims.

For v0.8.1, the Raspberry and Orange constructor/source-bound evidence now
exists. It does not replace physical FAT for the exact draft artifacts. Retain
the Raspberry mounted-image and kernel proof with that exact draft; trusted
parent machinery remains frozen legacy recovery until FAT closes.

## Support matrix

| Platform | Asset type and intended use | Source/build evidence (not FAT) | Manual FAT status | Known limitations |
|---|---|---|---|---|
| Desktop | Windows installer or portable ZIP; Ubuntu DEB or AppImage for the hardware-free simulator. | Desktop package build, simulator tests, and package/legal checks in the [desktop workflow](../docs/workflows/desktop-development.md#hardware-free-verification-matrix). | **UNQUALIFIED — exact package launch record is still required.** | macOS distribution is paused. An unsigned Windows build may show the normal Windows warning. Desktop cannot qualify GPIO, OLED, DAC, power, or USB gadget behavior. Desktop Configure WiFi/Data Backup/Restore transfer remains unsupported. |
| Raspberry Pi Zero 2 W | The exact versioned Raspberry image ZIP and its Imager manifest for a fresh install; the profile-qualified device ZIP only where its updater path is explicitly recorded. | v0.8.1 Raspberry constructor/source-bound evidence, including retained mounted-image and kernel proof for the exact draft, plus Raspberry profile, native cross-build, image-contract, sample, and sanitization checks in the [image workflow](../docs/workflows/image-construction-and-proof.md#source-bound-constructor-procedure). | **UNQUALIFIED — exact image, PCB, power path, controls, and audio FAT are still required.** | Raspberry full-image update remains manual. USB Audio/MIDI is experimental local-bench validation only. Simultaneous physical outputs use unsynchronized clocks and may drift or echo. The current enclosure has been built and validated on the available unit, but remains a v21 test-fit design rather than a production-final mechanical product. |
| Orange Pi Zero 2W | The exact versioned production Armbian image for full image installation, plus the profile-qualified runtime-only updater ZIP `octessera-<version>-orange-pi-zero-2w-runtime-updater-aarch64.zip` and `SHA256SUMS-orange-pi-zero-2w-runtime-updater.txt`. The standalone manual runtime ZIP remains manual and is not an OTA asset. | v0.8.1 Orange constructor/source-bound evidence, plus Orange profile, Armbian image/kernel proof, native cross-build, and staged sample checks in the [Orange bring-up procedure](../hardware/docs/orange-pi-armbian-bringup.md). | **UNQUALIFIED — exact image, PCB, power path, controls, and audio FAT are still required.** | Runtime-only Check/Apply/Rollback use the guarded updater; full Armbian, kernel, device-tree, and image replacement remains manual. USB Audio/MIDI is experimental local-bench validation only. Profile or asset mismatches fail closed without a Raspberry, manual-ZIP, or image fallback. Simultaneous physical outputs use unsynchronized clocks and may drift or echo. The current enclosure has been built and validated on the available unit, but remains a v21 test-fit design rather than a production-final mechanical product. |

The exact asset names, current fourteen-file custom release count, checksum files,
and ZIP contracts remain in [Release assembly](../docs/workflows/release-assembly.md#custom-release-assets).
Those names describe packaging; they do not turn an artifact into a supported
hardware release.

Data Backup/Restore is a Pi-only local transfer path during `Configure WiFi`.
Board-side export, reflash, media restore, and physical confirmation still need
the corresponding manual FAT evidence; this feature does not qualify an image
or board release by itself.

## USB policy

USB Audio and USB MIDI are **not public first-release support claims**. They are
experimental, local bench-validation paths until both conditions below are met:

1. Octessera has an authorized USB identity. The current Linux Foundation VID/PID
   values are local-validation values only, not a public product identity. This
   project does not invent or publish replacement IDs.
2. The exact image and assembled board pass the electrical and manual FAT gates:
   port role, VBUS/CC/UDC behavior, no-backfeed behavior, host enumeration, and
   the intended audio/MIDI exercise.

USB Audio and USB MIDI defaults remain disabled. A desktop toggle or a native
capability check is not permission to advertise the feature or to skip the
power checks. Before connecting a host to an instrument already powered from
the enclosure USB-C input, use a data-only cable or power-isolating adapter.
Software cannot prevent a host cable from back-feeding 5V while retaining data.
Read [safety and power](hardware/safety-and-power.md) first.

## Release-owner checklist

The [release workflow](../docs/workflows/release-assembly.md#owner-handoff)
should end with a populated draft. Keep it unpublished while the owner performs
this checklist; publication is a separate, explicit human decision.

- [ ] Choose a unique semantic version, tag, and release name. Confirm every
      manifest, package version, board profile, and release metadata agrees.
- [ ] Confirm the workflow used the clean, exact source SHA and that its CI gates
      passed against that SHA. Record the tag-to-commit identity and retain the
      Raspberry mounted-image/kernel proof for the exact draft.
- [ ] Confirm the populated draft is still a draft and has not been announced or
      published.
- [ ] Compare the draft with the exact current asset contract: fourteen custom
      root assets, exact names, no extras, and no missing files. Do not count
      GitHub’s automatic source archives as custom assets.
- [ ] Verify every checksum file and the root `SHA256SUMS.txt` against the exact
      bytes. Keep the checksums with the release evidence.
- [ ] Open the Raspberry updater ZIP and both Orange runtime ZIPs and verify their
      exact contents and modes. The Raspberry ZIP contains `octessera-pi`,
      `octessera-device-release.json`, `LICENSE`, and `NOTICE`. The Orange
      standalone ZIP contains `octessera-pi`, `octessera-runtime.json`,
      `SHA256SUMS`, `octessera-device-release.json`, `LICENSE`, and `NOTICE`.
      The Orange runtime-updater ZIP is named
      `octessera-<version>-orange-pi-zero-2w-runtime-updater-aarch64.zip` and
      contains `octessera-pi`, `octessera-device-release.json`, `LICENSE`, and
      `NOTICE`; its checksum is
      `SHA256SUMS-orange-pi-zero-2w-runtime-updater.txt`.
- [ ] Inspect the portable desktop ZIP: `octessera.exe`, the complete 320-file
      artifact inventory (318 WAV sampler rows plus two AIFF attribution rows),
      its checksum file, and the legal notice tree are present; verify the legal
      bundle rather than trusting its filename.
- [ ] Verify the release-evidence ZIP contains the per-platform checksums,
      image/kernel evidence, runtime evidence, and generated legal notice bundle.
      Keep source-duty review separate from any claim of legal compliance.
- [ ] Confirm the 318-file WAV sampler library and canonical default patch are
      present in the desktop package and both image-construction outputs, with
      the complete 320-row attribution inventory accounted for. Confirm user
      samples remain usable on each intended path.
- [ ] Run desktop package launch/FAT on the exact named asset. Record OS,
      filename, SHA-256, launch result, and known warnings.
- [ ] Run Raspberry FAT on the exact image and assembled board: power, OLED,
      controls, DAC/audio, ports, samples, and enclosure fit. Record the exact
      image SHA and PCB/board revisions. The mounted-image and kernel proof must
      remain attached to this exact draft; it is not a substitute for FAT.
- [ ] Run Orange FAT on the exact image and assembled board with the same
      evidence, using Orange-specific pin, port, and recovery checks. Do not
      substitute a runtime-only respin or the frozen trusted-parent recovery
      lane for constructor qualification.
- [ ] If USB is exercised, record the authorized identity and the electrical and
      manual no-backfeed/host tests. Otherwise keep USB Audio/MIDI experimental
      and out of supported-release claims.
- [ ] Have a human review source duties for the pinned upstream inputs, source,
      patches, configuration, and build scripts. Do not call that review legal
      compliance.
- [ ] Record known limitations: paused macOS, full Orange Armbian/kernel/
      device-tree/image replacement remains manual, the standalone manual ZIP
      is not an OTA asset, unsynchronized multi-output drift, any unsigned
      Windows warning, desktop Data Backup/Restore transfer remains unsupported,
      the v21 test-fit enclosure boundary, and the boundary between runtime-only
      Raspberry full-image update remains manual; record the boundary
      between runtime-only updates and constructor qualification. The available enclosure builds
      have been validated; this is not a promise of production-final mechanical
      support.
- [ ] Only after the exact evidence is attached, the draft remains correct, and
      a human has reviewed the result: explicitly publish. Otherwise leave it
      populated and unpublished.

## What this page does not promise

These records do not claim physical qualification, product safety, or legal
compliance. They tell you where the evidence boundary is, so a handmade little
instrument is not declared ready merely because a build finished.
