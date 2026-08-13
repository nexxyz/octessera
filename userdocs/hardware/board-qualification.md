# Board qualification and status

This page keeps two different kinds of proof from getting mixed together. It is
deliberately not tied to a release number; use the release page for the image
asset and its matching checksum.

## What the labels mean

- **Source/build proof** means the profile, native adapters, image inputs, and
  checks can be inspected or exercised without a finished instrument. It proves
  that the project is wired together in source and that the selected artifacts
  can be checked.
- **Physical-board qualification** means a named board, image, PCB, harness,
  power arrangement, and enclosure have been tested by a person at the bench.
  A passing host build, diagnostic utility, or desktop simulator is not this
  proof.

## Current status

| Board path | Source/build proof | Physical-board qualification |
|---|---|---|
| Raspberry Pi Zero 2 W | The canonical profile and Raspberry image/boot contracts are present and covered by repository checks. See the [board profiles](../../docs/board-profiles.md) and [Raspberry first-boot page](raspberry-pi-first-boot.md). | The current constructor image and assembled PCB path still need physical qualification. Confirm the OLED, controls, DAC, power, USB, and enclosure fit at the bench. |
| Orange Pi Zero 2W | The canonical profile and separate Armbian production/diagnostic paths are present. The exact Orange overlay, device paths, and bring-up gates are recorded in the [Armbian bring-up notes](../../hardware/docs/orange-pi-armbian-bringup.md) and [board profiles](../../docs/board-profiles.md). | Some development-board checks are recorded, but they do not qualify the current production image or a closed enclosure. I2S pin proof, USB role/UDC behavior, the complete control surface, and enclosure fit remain explicit physical gates. |

For both boards, setup-portal and boot-handoff source contracts must not be read
as proof that a freshly flashed card has been qualified. Record the board
revision, image identity, PCB/harness revision, and bench observations when you
run the checks. If a pin, port role, power path, or peripheral result is
unclear, stop and use the exact board reference rather than translating the
Raspberry pin table by position.

## Before you close the case

1. Use the selected board's profile and pinout source: [Raspberry wiring](pinout-and-connections.md#raspberry-pi-zero-2-w), [Orange bring-up notes](../../hardware/docs/orange-pi-armbian-bringup.md#safety-gates-before-connecting-the-octessera-pcb), and the [canonical board profiles](../../docs/board-profiles.md).
2. Complete the open-assembly electrical and runtime checks for your board.
3. Keep unresolved physical checks visible in your build notes. Do not call the
   device qualified because a source check passed.
