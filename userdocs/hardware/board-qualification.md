# Board qualification and status

This page keeps two different kinds of proof from getting mixed together. It is
deliberately not tied to a release number; use the release page for the image
asset and its matching checksum.

For immediate symptoms, use [troubleshooting](../troubleshooting.md). For power,
USB backfeed, orientation, and enclosure stop conditions, read [safety and
power](safety-and-power.md).

The public release boundary is summarized in the [release support matrix](../release-support.md).
An exact image or a clean source/build result is not a supported release until
the matching manual FAT record exists.

The [v0.8.1 release record](../release-records/v0.8.1.md) contains the exact
artifact hashes and automated result for the current draft.

## What the labels mean

- **Source/build proof** means the profile, native adapters, image inputs, and
  checks can be inspected or exercised without a finished instrument. It proves
  that the project is wired together in source and that the selected artifacts
  can be checked.
- **Physical-board qualification** means a named board, image, PCB, harness,
  power arrangement, and enclosure have been tested by a person at the bench.
  A passing host build, diagnostic utility, or desktop simulator is not this
  proof.

## Published images and current constructor

The v0.8.1 source-defined constructor and release validation are recorded in the
[release record](../release-records/v0.8.1.md). Run `33037951901` is only the
[Orange strict diagnostic-image build](https://github.com/nexxyz/octessera/actions/runs/33037951901);
run `33045139129` is the [full release run for both production images and the 14
uploaded assets](https://github.com/nexxyz/octessera/actions/runs/33045139129).
Those older runs are source/build evidence, not proof that their exact image
bytes were flashed to an assembled board. The newer reviewed Orange 0.8.1
current parent did pass its recorded physical constructor-parent scope. It is an
exact nonpublishing respin input, not an official release image; a runtime-only
respin and the remaining hardware matrix have not yet been evidenced. Historical
v0.7.5 images remain usable for their documented runtime and setup paths, but
they do not prove the current constructor layer.

## Current status

| Board path | Source/build proof | Physical-board qualification |
|---|---|---|
| Raspberry Pi Zero 2 W | The v0.8.1 source-bound constructor validation and canonical profile checks are recorded in the [release record](../release-records/v0.8.1.md). See the [board profiles](../../docs/board-profiles.md) and [Raspberry first-boot page](raspberry-pi-first-boot.md). | **UNQUALIFIED / OPEN — no exact image and assembled-board FAT result is closed here.** Confirm the OLED, controls, DAC, power/no-backfeed, USB role, and enclosure fit at the bench. |
| Orange Pi Zero 2W | The exact validated constructor parent and its source-bound proof are recorded in the [release record](../release-records/v0.8.1.md). The reviewed [Armbian bring-up procedure](../../hardware/docs/orange-pi-armbian-bringup.md) records the remaining gates. | **CONSTRUCTOR PARENT VALIDATED / FULL HARDWARE OPEN.** Exact boot, setup, OLED, controls, Jack audio, service, reboot, board fit, and automated FAT passed. Runtime respin, HDMI, USB, SD2, and remaining soak/recovery checks are open. |

USB Audio and USB MIDI remain experimental/local bench validation on both board
paths. The Linux Foundation VID/PID values used by the local composers are not a
public product identity. Do not advertise USB support until an authorized
identity and the electrical/manual FAT gates are recorded for the exact image.

For both boards, setup-portal and boot-handoff source contracts must not be read
as proof that a freshly flashed card has been qualified. Record the board
revision, image identity, PCB/harness revision, and bench observations when you
run the checks. If a pin, port role, power path, or peripheral result is
unclear, stop and use the exact board reference rather than translating the
Raspberry pin table by position.

## Before you close the case

1. Use the selected board's profile and pinout source: [Raspberry wiring](pinout-and-connections.md#raspberry-pi-zero-2-w), [Orange bring-up notes](../../hardware/docs/orange-pi-armbian-bringup.md#safety-gates-before-connecting-the-octessera-pcb), and the [canonical board profiles](../../docs/board-profiles.md).
2. Complete the open-assembly electrical and runtime checks for your board. Use
   the [assembly manual](assembly-manual.md) and [enclosure notes](enclosure.md)
   for the active v21 test-fit sequence.
3. Keep unresolved physical checks visible in your build notes. Do not call the
   device qualified because a source check passed.
