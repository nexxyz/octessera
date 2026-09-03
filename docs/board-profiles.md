# Board profiles

Octessera uses explicit board profile IDs at build and artifact boundaries:

- `raspberry-pi-zero-2w` is the supported Raspberry Pi Zero 2 W profile.
- `orange-pi-zero-2w` is the supported Orange Pi Zero 2W production profile.

## Five-layer naming taxonomy

Octessera names are organized into five layers:

- General/native shared crates — `crates/platform-core`, `crates/playback-runtime`, and other board-agnostic native crates.
- Desktop Simulator — `apps/desktop`, `@octessera/desktop`, and `octessera-desktop`.
- Shared hardware host — `apps/pi-zero` and `octessera-pi`, retained compatibility names serving both boards.
- Raspberry Pi Zero 2 W — `raspberry-pi-zero-2w`, `tools/pi`.
- Orange Pi Zero 2W — `orange-pi-zero-2w`, `tools/orange-pi`.

## Feature inventory

- Canonical HAL features: `raspberry-pi-zero-2w` and `orange-pi-zero-2w`.
- Canonical app features: `hardware-raspberry-pi-zero-2w` and
  `hardware-orange-pi-zero-2w`.
- Compatibility aliases: `rpi-zero-2w`, `pi-zero`, `hardware-rpi-zero-2w`, and
  `hardware-pi`. They remain accepted for existing Cargo commands; use the
  canonical names for new commands. No alias removal date is promised.
- Names matching `legacy-hardware-*` are internal rejection markers, not
  user-facing aliases.

Both board profiles expose the same native `System > Configure WiFi` menu
contract and typed setup-portal status flow. The confirmed `Open Portal` action
writes the exact `start\n` marker at
`/run/octessera-setup-request/inbox/start`; one root service then coordinates the
portal and publishes `/run/octessera-setup-status/current.json`. The pinned
patched wifi-connect owns AP, DHCP, HTTP, and network switching. Fresh images do
not start an automatic hotspot. Their fixed accounts, image provisioning paths,
and parent-image preconditions differ: Raspberry uses the Pi image path and `pi`
account, while Orange uses the Armbian path with separate `octessera` setup and
`octessera-runtime` service accounts. Physical setup-portal qualification on
both boards is a FAT activity.
The same System menu exposes standalone `Backup / Restore`; Pi uses the regular
`wlan0` IPv4 service on port 8081, while desktop is unsupported.

## Shared OLED boot handoff and qualification

The OLED boot handoff is also one parity contract. Both boards use the same
mirrored four-band sweep defined by `resources/oled/boot-sweep-v1.json`, the
same exclusive `/run/octessera-boot` lock/status protocol, and the same
acknowledged first-menu handoff. The mounted SSD1351 controller origin travels
leftward while the physical sweep travels left-to-right with a panel-facing
right slash. Canonical bottom-to-top coordinates use
`slanted_origin = bottom_origin - row_y`, so the top-row origin is 127 px less
X than the bottom-row origin. It uses magenta/green/yellow/cyan order, 30
frames, and 25 fps. Raspberry's selected initramfs writes one clean static
logo+wordmark frame before its root-installed systemd service starts the sweep;
the service uses the Pi SPI/GPIO adapter. Orange's selected initramfs writes one
static RGB565 frame with its fixed Python closure before its root-installed
Python OLED utility starts the H618 SPI/GPIO sweep.
Orange readiness additionally applies the selected-route rules: every
non-empty Jack/USB/HDMI set is valid, Jack is required only when selected,
recognized disconnected USB or HDMI may wait, selected faults block readiness,
and no route is a fallback for another. The source/build contract and physical
qualification are separate: constructor outputs and repository checks do not
establish a physical result. The [current artifact record](../userdocs/release-records/v0.8.1.md)
records artifact and automated evidence; physical FAT remains separate. Both
boards may remain blank before their initramfs writer runs; systemd then owns the
only OLED animator. Reboot retains the clean shutdown logo+wordmark.

### HDMI and physical display qualification

`Terminal` leaves `/dev/tty1` with Linux; native grid mode owns a native VT lease
around `/dev/fb0`, without connector forcing or a display server. Missing `fb0` is
nonfatal and retried. The splash observes handoff until `first_menu_rendered`, then
reclaims OLED presentation for a native fatal status when startup fails. Orange/
Raspberry HDMI connector, framebuffer, VT, and OLED behavior are physical FAT
checks; this source contract is not hardware proof.

The board-specific HALs own their physical pin and device descriptors. The HAL
also exposes the `orange-pi-zero-2w` profile descriptor and its diagnostic
OLED/I2C bring-up backend. The Orange production `octessera-pi` runtime uses
the shared 44.1 kHz
rate and supports the OLED, NeoTrellis, NeoKey, all four encoders, persistent
store, samples, MIDI, and the selected audio routes. A selected Jack route
uses exactly `hw:CARD=octesseradac,DEV=0` with verified stereo support. The
production image constructs the AHUB0 vendor dummy-codec route and exact
`octessera-dac` playback card during image construction; it does not depend on a
manual or experimental audio overlay.
The native menu persists Jack Audio, USB Audio, and HDMI Audio independently;
every non-empty output set is valid. Jack is fatal/required only when selected;
recognized disconnected USB or HDMI routes may wait, selected route faults block
readiness, and no route is used as a fallback. Simultaneous physical outputs
use independent unsynchronized clocks and can drift or echo; this phase does
not provide sample alignment. The board adapters use
`/sys/class/drm/card0-HDMI-A-1`; Raspberry code pins that card0 identity and
does not scan or fall back to card1. This establishes connector identity only,
not connected HDMI audio or audible qualification.
MIDI uses the native host adapter, including USB MIDI when the configured gadget
port is present.

The Orange image-side USB gadget reads the persisted default at
`/var/lib/octessera/presets/default.json`. `audioOutputs.usb` enables the fixed
44.1 kHz stereo UAC2 function and `usb.midiOutEnabled` enables the fixed MIDI
function. The valid compositions are no gadget, MIDI only, UAC2 only, and
combined; HDMI and Jack do not change gadget composition. USB Audio and USB
MIDI require an authorized identity and electrical/manual FAT before support.
Linux Foundation VID/PID values are for local validation only, not a public
product identity.
The confirmed device apply lane uses one narrow root-owned socket rather than a
general sudo command path. It accepts only exact `reboot\n` and `poweroff\n`
requests. `reboot\n` validates the saved config before invoking
`/usr/bin/systemctl reboot`; `poweroff\n` invokes only
`/usr/bin/systemctl poweroff` and does not depend on that config validation.
The socket starts after `local-fs.target` only; the runtime separately waits for
musical-default provisioning before it starts.
Both return `accepted\n` only after the fixed command succeeds and return
`rejected\n` for malformed, unknown, extra-byte, or definitively failed
requests.

The Orange control surface requires the exact validated NeoTrellis wiring and
addresses. There is no alternate Trellis bus, address, or hardware fallback.

The production image applies the input-routing overlay before the service
starts, so all four encoder switches are available. Before that overlay, SW3's
switch line is unavailable because physical pin 8 / H618 PH0 is active UART0
TX; its A/B lines remain available. This is a bring-up condition, not the
production control-surface contract. Runtime readiness follows healthy required
audio, initialized control-surface devices, and the first rendered snapshot.
The service uses `LimitRTPRIO=70`; for the DAC-only persistent Jack
qualification, the callback is fixed to CPU1 and both DSP workers are fixed to
CPUs 2 and 3, all at verified `SCHED_FIFO` priority 70. USB/HDMI remain on
legacy, unqualified handling and are disabled for reported measurements; full
fanout is deferred. Its sole ambient and bounding capability is
`CAP_SYS_TTY_CONFIG` for native VT leasing; it does not use `CAP_SYS_NICE` or
other realtime capability elevation. Startup reports the
named `DAC` or `UAC2` sink and rejects the qualified Jack stream when callback
promotion is not verified.
Its typed bus descriptors record `/dev/i2c-2` at `5002400.i2c` and
`/dev/spidev1.0` at `5011000.spi`; its encoder descriptor records H618
`300b000.pinctrl` offsets rather than Raspberry GPIO fields. The Orange OLED
default is 16 MHz; the HAL retains the validated 1/2/4/8/12/16 MHz override
ladder. All four Orange encoder descriptors reverse literal board A/B direction
at the Orange event boundary. AUX2 A/B (227/269) remain requestable with UART0
active, while its switch (224) is requested only after UART0 is disabled by the
dedicated input-routing overlay.

The narrow Pi cross-build tools accept exactly `raspberry-pi-zero-2w` and
`orange-pi-zero-2w`. They select `hardware-raspberry-pi-zero-2w` or
`hardware-orange-pi-zero-2w` respectively and write the same profile-qualified
feature into `octessera-pi.metadata.json`; the default remains Raspberry. The
Raspberry deploy, provision, preflight, pi-gen, and Raspberry Pi Imager tools
still accept only `raspberry-pi-zero-2w`. They reject Orange rather than
guessing at pins, GPIO numbering, or an audio backend. Orange production image
work stays on the separate Armbian path.

Pi binaries expose `--print-build-metadata`, and cross-build output includes
`octessera-pi.metadata.json`. Release manifests, installed service metadata,
and device update manifests carry the same canonical ID so a mismatched
binary or artifact fails closed where the host can check it.

Production image artifacts use the version-qualified name
`octessera-<version>-orange-pi-zero-2w.img.xz`, with matching SHA-256 and image
provenance files. The production image contains the hash-bound runtime bundle
`octessera-pi`, `octessera-runtime.json`, and `SHA256SUMS`; its metadata declares
`artifact_kind=production-runtime` and `runtime_ready=true` for
`orange-pi-zero-2w`.

Diagnostic image mode remains separate and explicit:
`OCTESSERA_IMAGE_MODE=diagnostic`. It contains the OLED/Seesaw smoke utilities
and bring-up tools, but no production runtime bundle or `octessera.service`.
The smoke utilities are diagnostic artifacts, not substitutes for the
production image.

The production service reads its persistent store and samples from
`/var/lib/octessera/presets` and `/var/lib/octessera/samples` as
`octessera-runtime`. The separate interactive `octessera` user is used for
setup and administration; it is not the runtime account.
Stage the canonical binary name together with its adjacent exact-name sidecar:

```powershell
./tools/orange-pi/build-orange-cross.ps1 -Binary orange-oled-smoke -Profile release
./tools/orange-pi/build-orange-cross.ps1 -Binary orange-seesaw-smoke -Profile release
./tools/orange-pi/build-orange-cross.ps1 -Binary octessera-pi -Profile release
```

The builder copies the selected binary and writes its exact-name `.metadata.json`
sidecar beside it. The sidecar is schema 2 with an exact field set and a
lowercase SHA-256 of that copied ELF. The output is a Linux AArch64 ELF and
must not be executed by the Windows host. Before a remote test, copy both files
without renaming them, run the selected binary with
`--print-build-metadata` on the board, and independently compare the remote
`sha256sum` of the binary with the recorded local SHA-256.
Metadata mode is read-only, hardware-free, and verifies the running
`/proc/self/exe`.

`sudo -n /tmp/orange-seesaw-smoke --confirm-active-test` requires explicit confirmation and
only resets the proven NeoTrellis/NeoKey addresses on `/dev/i2c-2` before
reading their valid Seesaw hardware IDs. It does not configure keypad events,
write LEDs, poll keys, request GPIO, access OLED/SPI/audio, start a runtime, or
install a service.

The Seesaw deadline and interruption checks are cooperative. Synchronous
I2C open/ioctl/write/read calls and the reset delay may outlast the deadline;
an interruption is observed between transactions, after the current call
returns.

One OLED invocation performs one cooperative-budgeted `pattern → black →
display-off` operation with a 3-second operation budget and a 1-second cleanup
budget. Normal shutdown performs black and display-off together; fallback
cleanup prioritizes display-off before the black frame. The budget checks do
not turn synchronous SPI/GPIO syscalls into a wall-clock guarantee.
The utility owns cleanup on errors and handled interruption; do not split the
sequence into separate commands.

The metadata command is read-only. The active hardware test is
`sudo -n /tmp/orange-oled-smoke --confirm-active-test` after the separate passive,
staging, and electrical gates; it must not be run against an unverified device
or wiring harness.

The native runtime contract exposes Jack Audio, USB Audio, and HDMI Audio
independently on the Orange profile under the selected-route rules above. The
smoke artifacts remain diagnostic-only. The production runtime routes internal
synth/sample audio through the realtime engine and emits MIDI through the native
host adapter. Orange Check, Apply, and Rollback use the root-owned broker and
guarded updater with the explicit profile-qualified
`octessera-<version>-orange-pi-zero-2w-runtime-updater-aarch64.zip` and
`SHA256SUMS-orange-pi-zero-2w-runtime-updater.txt` pair. Profile, asset,
manifest, checksum, and health failures return typed failure and stop; no
Raspberry asset, standalone manual ZIP, or full-image fallback is selected.
The updater changes only the managed runtime release. Full Armbian, kernel,
device-tree, and image replacement remains manual, and the standalone manual
runtime ZIP is not an OTA asset.
SIGINT
performs the normal shutdown frame, bounded 750 ms black-LED cleanup/retry, and
OLED-off acknowledgement; workers join only after an acknowledgement and are
not joined after a cleanup timeout. Synchronous device calls can still outlast
that cooperative bound.
