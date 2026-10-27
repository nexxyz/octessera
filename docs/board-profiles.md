# Board profiles

Octessera supports exactly two boards, and uses explicit profile IDs at every
build and artifact boundary:

- `raspberry-pi-zero-2w` for the Raspberry Pi Zero 2 W;
- `orange-pi-zero-2w` for the Orange Pi Zero 2W.

This page collects the board-specific facts. Contracts both boards share
(scheduler, audio routing, OLED boot handoff, setup portal, device apply,
updater) live in [`runtime-boundaries.md`](runtime-boundaries.md).

## Naming

Octessera names fall into five layers:

| Layer | Names |
| --- | --- |
| General native crates | `crates/platform-core`, `crates/playback-runtime`, other board-agnostic crates |
| Desktop simulator | `apps/desktop`, `@octessera/desktop`, `octessera-desktop` |
| Shared hardware host | `apps/pi-zero`, `octessera-pi` (serves both boards) |
| Raspberry Pi Zero 2 W | `raspberry-pi-zero-2w`, `tools/pi` |
| Orange Pi Zero 2W | `orange-pi-zero-2w`, `tools/orange-pi` |

Cargo features:

- HAL (`octessera-hal`): `raspberry-pi-zero-2w` and `orange-pi-zero-2w`.
- App (`octessera-pi`): `hardware-raspberry-pi-zero-2w` and
  `hardware-orange-pi-zero-2w`. Both pull in the routing-tree executor.

## Shared AArch64 target

Both boards are Cortex-A53. `.cargo/config.toml` builds with
`target-cpu=cortex-a53` and `-aes,-sha2`: NEON and CRC are on both boards, but
the Raspberry does not expose the optional AES and SHA2 extensions.

## Build metadata and cross-builds

The shared `tools/pi/build-rpi-opi-cross.ps1` accepts exactly the two IDs. It
selects the matching app feature and, for Raspberry builds, writes
`octessera-pi.metadata.json`; the default is Raspberry. The Raspberry deploy,
provision, preflight, pi-gen, and Raspberry Pi Imager tools accept only
`raspberry-pi-zero-2w` and reject Orange rather than guessing at pins, GPIO
numbering, or an audio backend. Orange image work stays on the Armbian path.

Raspberry binaries expose `--print-build-metadata`, and release manifests,
installed service metadata, and device ZIP names carry the same canonical ID,
so a mismatched binary or artifact fails closed wherever the host can check.
On Orange, the installed
`/opt/octessera/releases/<version>/octessera-runtime.json` is the
production-runtime contract; a deployed development binary is identified by
its `sha256sum`.

## Accounts, stores, and setup

| | Raspberry | Orange |
| --- | --- | --- |
| Image path | pi-gen | Armbian |
| Runtime account | `pi` | locked `octessera-runtime` |
| Setup/admin account | `pi` | `octessera` |
| Presets | `/home/pi/presets` | `/var/lib/octessera/presets` |
| Samples | `/home/pi/samples` | `/var/lib/octessera/samples` |

Both boards expose the same native
`System > Connections > Wi-Fi > Setup Portal` menu and status flow, and the same
standalone `System > Maintenance > Backup / Restore` service on the regular
`wlan0` IPv4 address, port 8081. Desktop reports both as unsupported. Fresh
images never start a hotspot on their own. The board-side mechanics are in the
[setup portal boundary](runtime-boundaries.md#setup-portal-boundary).

## OLED boot handoff

Both boards play the same sweep from `resources/oled/boot-sweep-v1.json` (green,
magenta, cyan, and yellow bands reading left to right on the panel, 30 frames
at 25 fps) and use the same exclusive `/run/octessera-boot` lock and status
protocol. They differ only in the early writer:

- Raspberry's selected initramfs writes one static logo+wordmark frame, then a
  root-installed systemd service runs the sweep through the Pi SPI/GPIO
  adapter.
- Orange's selected initramfs writes one static RGB565 frame with its fixed
  Python closure, then a root-installed Python OLED utility runs the H618
  SPI/GPIO sweep.

Either board may stay blank until its first writer runs. The full contract,
including HDMI terminal and VT ownership, is the
[OLED boot handoff boundary](runtime-boundaries.md#oled-boot-handoff-boundary).
Constructor outputs and repository checks describe the image; they do not
replace looking at the actual panel.

## Audio routes

Jack is always on and is the readiness route. USB Audio and HDMI Audio are
independently selectable mirrors of the Jack mix, so the Pi menus omit the
no-op Jack toggle (desktop keeps its own). A missing, waiting, or faulted
mirror stays local to that route. Mirrors run on independent clocks and can
drift or echo; nothing aligns them.

- The Orange Jack route is exactly `hw:CARD=octesseradac,DEV=0`, built during
  image construction from the AHUB0 vendor dummy-codec route. No manual or
  experimental overlay is involved.
- Both boards use the HDMI connector `/sys/class/drm/card0-HDMI-A-1`;
  Raspberry code pins that card0 identity and never falls back to card1. This
  establishes the connector, not that HDMI audio is audible.
- MIDI goes through the native host adapter, including USB MIDI when the
  configured gadget port is present.

Thread placement (both boards, from `apps/pi-zero/src/audio_priority.rs`): the
runtime and helper threads run on CPU0 under `SCHED_OTHER`, the Jack callback
on CPU1 at verified `SCHED_FIFO` 70, and USB/HDMI mirror callbacks on CPU0 at
60. Orange Capacity workers use CPUs 2 and 3 at 70. Startup names the `DAC` or
`UAC2` sink and rejects the Jack stream if callback promotion is not verified.

## Raspberry USB data role

`usb.dataRole` in `/home/pi/presets/system.json` selects the next-boot role.
`gadget` (the default) enables the image-side Audio/MIDI gadget and the SD2
transfer path; `host` disables both and requires USB Audio and USB MIDI to be
off. Applying a role is a confirmed System-only save, a checked edit of the
exact `[all]` boot-config line, and a reboot. It is not a live switch. An active
SD2 transfer rejects Host before `system.json` is written.

Full image construction or Pi provisioning installs the role helper and boot
contract. Runtime updates and fast binary deploys never touch boot
configuration.

## Orange specifics

### USB gadget

The image-side gadget reads `/var/lib/octessera/presets/system.json`:
`audioOutputs.usb` enables the fixed 44.1 kHz stereo UAC2 function and
`usb.midiOutEnabled` the MIDI function, giving no gadget, MIDI only, UAC2 only,
or combined. HDMI and Jack do not affect it. The staged
`usr/share/octessera/defaults/pi-default.json` is image-build metadata, not an
active default. The Linux Foundation VID/PID values are for local validation
only, not a public product identity. Composer details are in
[Orange Pi tools](../tools/orange-pi/README.md#orange-pi-usb-gadget-composer).

### Service and power

`octessera.service` uses `LimitRTPRIO=70`; its only ambient and bounding
capability is `CAP_SYS_TTY_CONFIG` for native VT leasing, with no
`CAP_SYS_NICE`. Reboot and power-off go through one narrow root-owned socket
that accepts only `reboot\n` and `poweroff\n`; see the
[device apply boundary](runtime-boundaries.md#device-apply-boundary). Check,
Apply, and Rollback go through the root-owned update socket to
`octessera-update`, which takes only the Orange device ZIP and checksum; full
Armbian, kernel, device-tree, and image replacement stays a reflash.

On SIGINT the runtime shows the normal shutdown frame, clears the LEDs with a
bounded 750 ms cleanup/retry (the shared `render_loop.rs` and `seesaw_io.rs`
timeouts), and waits for the OLED-off acknowledgement. Workers are joined only
after that acknowledgement, never after a cleanup timeout. Synchronous device
calls can still outlast the bound.

### Control surface and buses

The NeoTrellis needs its exact validated bus, wiring, and addresses; there is
no alternate bus, address, or fallback. The production image applies the
input-routing overlay before the service starts, so all four encoder switches
work. Before that overlay, SW3's switch line is unavailable because physical
pin 8 (H618 PH0) is UART0 TX; its A/B lines still work. AUX2 A/B (227/269) can
be requested with UART0 active, while its switch (224) is requested only after
the overlay disables UART0. All four Orange encoder descriptors reverse literal
board A/B direction at the Orange event boundary.

The shared Seesaw INT net is physical pin 10, H618 PH1 (UART0 RX), line 225.
Like the Raspberry, the Orange scans the NeoTrellis and NeoKey when INT falls or
stays low, plus a 100 ms safety poll. The line is only free once the overlay
has released UART0; if it cannot be requested, startup fails with a controls
error instead of falling back to polling.

The HAL descriptors record `/dev/i2c-2` at `5002400.i2c`, `/dev/spidev1.0` at
`5011000.spi`, and encoder offsets on `300b000.pinctrl`. The OLED runs at
16 MHz.

Runtime readiness means healthy required audio, initialized control-surface
devices, and the first rendered snapshot.

### Production image

Orange images are named `octessera-<version>-orange-pi-zero-2w.img.xz`, with
a matching SHA-256 file. They contain the hash-bound runtime
bundle (`octessera-pi`, `octessera-runtime.json`, `SHA256SUMS`) and declare
`artifact_kind=production-runtime` and `runtime_ready=true`. There is no
separate diagnostic image; the smoke utilities below are copied to a board
running the production image.

### Smoke utilities

Build and stage a smoke binary together with its exact-name sidecar:

```powershell
./tools/orange-pi/build-opi-cross.ps1 -Binary orange-oled-smoke -Profile release
./tools/orange-pi/build-opi-cross.ps1 -Binary orange-seesaw-smoke -Profile release
```

The builder writes a schema-2 `.metadata.json` beside each smoke ELF, with a
lowercase SHA-256 of the copied binary. Do not run the ELF on Windows (it will
not work, and it will not tell you why in a helpful way). Before a remote test,
copy both files without renaming them, run the binary with
`--print-build-metadata` on the board, and compare the remote `sha256sum` with
the recorded SHA-256. Metadata mode is read-only, hardware-free, and verifies
the running `/proc/self/exe`.

`sudo -n /tmp/orange-seesaw-smoke --confirm-active-test` resets only the proven
NeoTrellis/NeoKey addresses on `/dev/i2c-2` and reads their Seesaw hardware
IDs. It does not configure keypad events, write LEDs, poll keys, touch GPIO,
OLED, SPI, or audio, start a runtime, or install a service. Its deadline and
interruption checks are cooperative: a synchronous I2C call or the reset delay
can outlast the deadline, and an interruption is noticed between transactions.

`sudo -n /tmp/orange-oled-smoke --confirm-active-test` performs one
`pattern → black → display-off` operation with a 3-second operation budget and
a 1-second cleanup budget. Normal shutdown sends black and display-off
together; fallback cleanup sends display-off first. The utility owns cleanup on
errors and handled interruption, so do not split the sequence into separate
commands. Run it only after the passive, staging, and electrical checks, never
against an unverified device or harness.
