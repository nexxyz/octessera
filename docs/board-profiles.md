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

Canonical feature ownership is internal: `raspberry-pi-zero-2w` owns the
Raspberry HAL implementation and dependencies, and
`hardware-raspberry-pi-zero-2w` owns the Raspberry app's canonical HAL feature
and runtime selection. The deprecated compatibility aliases `rpi-zero-2w`,
`pi-zero`, `hardware-rpi-zero-2w`, and `hardware-pi` expand to those canonical
owners. They remain accepted for existing Cargo commands and are covered by CI;
use canonical names for new commands. No alias removal date is promised.

Both board profiles expose the same native `System > Configure WiFi` menu
contract and typed setup-portal status flow. Their fixed accounts, image
provisioning paths, and parent-image preconditions differ: Raspberry uses the
Pi image path and `pi` account, while Orange uses the Armbian path with separate
`octessera` setup and `octessera-runtime` service accounts. Physical setup-portal
qualification on both boards remains pending.

The OLED boot handoff is also one parity contract. Both boards use the same
cyan/yellow/green/magenta four-band sweep defined by
`resources/oled/boot-sweep-v1.json`, the same exclusive `/run/octessera-boot`
lock/status protocol, and the same acknowledged first-menu handoff. Raspberry
embeds the native boot utility in initramfs and uses the Pi SPI/GPIO adapter;
Orange carries its fixed Python OLED utility and closure for H618 SPI/GPIO.
Orange readiness additionally waits for healthy internal DAC status. These
source paths are implemented, but their boot services, hooks, and selected
initramfs outputs still require a new constructor image and physical
qualification on both boards.

Orange runtime startup allows three attempts in a 30-second systemd start-limit
window: the initial start and two five-second failure retries. After
`start-limit-hit`, run `sudo systemctl reset-failed octessera.service` and then
`sudo systemctl start octessera.service`. The OLED boot-loop handoff uses a
monotonic 30-second deadline starting immediately after handoff start. Timeout,
signal, and unexpected post-ownership failures attempt a 32768-byte black RGB565
frame and display-off; either cleanup operation may fail, but both are attempted
and the handoff is marked failed for native recovery.

Both constructors also stage the same interactive terminal welcome without
changing PAM or update-motd. Raspberry declares its UART inactive in the
selected boot layout (`enable_uart=0`, no serial-console kernel token, and
masked serial-getty units). This is an image safety state, not a post-boot
UART release utility or ownership handoff. Orange keeps its UART0 release in
the reviewed input-routing path.

The board-specific HALs own their physical pin and device descriptors. The
canonical Raspberry Cargo feature owners are `raspberry-pi-zero-2w` and
`hardware-raspberry-pi-zero-2w`; the deprecated `rpi-zero-2w`, `pi-zero`,
`hardware-rpi-zero-2w`, and `hardware-pi` feature names remain compatibility
aliases and are covered by CI compile checks. The HAL also exposes the
`orange-pi-zero-2w` profile descriptor and its diagnostic OLED/I2C bring-up
backend. The Orange production `octessera-pi` runtime uses the shared 44.1 kHz
rate and supports the OLED, NeoTrellis, NeoKey, all four encoders, persistent
store, samples, MIDI, and the internal DAC. Audio requires exactly one CPAL
output device named `hw:CARD=octesseradac,DEV=0` with verified stereo support.
There is no default or HDMI fallback. USB UAC2 is an optional companion
(`audioOut=both`); `audioOut=usb` is rejected because the internal DAC is
required. MIDI uses the native host adapter, including USB MIDI when the
configured gadget port is present.

The Orange control surface requires the exact validated NeoTrellis wiring and
addresses. There is no alternate Trellis bus, address, or hardware fallback.

The production image applies the input-routing overlay before the service
starts, so all four encoder switches are available. Before that overlay, SW3's
switch line is unavailable because physical pin 8 / H618 PH0 is active UART0
TX; its A/B lines remain available. This is a bring-up condition, not the
production control-surface contract. Runtime readiness follows healthy required
audio, initialized control-surface devices, and the first rendered snapshot.
The service uses `LimitRTPRIO=70`; only CPAL callback threads may be promoted to
verified `SCHED_FIFO` priority 70. It does not use `CAP_SYS_NICE`, ambient
capabilities, or other realtime capability elevation. Startup reports the
named `DAC` or `UAC2` sink and rejects an Orange stream when callback promotion
is not verified.
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

The 0.7.5 production image artifact is
`octessera-0.7.5-orange-pi-zero-2w.img.xz`, with its matching SHA-256 and image
provenance files. Its explicit image metadata is
`OCTESSERA_IMAGE_MODE=production`. The image contains the hash-bound runtime
bundle `octessera-pi`, `octessera-runtime.json`, and `SHA256SUMS`; the metadata
declares `artifact_kind=production-runtime` and `runtime_ready=true` for
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

The smoke artifacts remain diagnostic-only. The production runtime routes
internal synth/sample audio through the realtime engine and emits MIDI through
the native host adapter. It requires the exact internal DAC; USB UAC2 is only an
optional companion and `audioOut=usb` is rejected. Orange update check, apply,
rollback, and OTA remain unsupported and return typed unavailable status before
an updater or network path is touched. The production service has no Orange
device-update path; use a verified production image artifact for image updates.
SIGINT
performs the normal shutdown frame, bounded 750 ms black-LED cleanup/retry, and
OLED-off acknowledgement; workers join only after an acknowledgement and are
not joined after a cleanup timeout. Synchronous device calls can still outlast
that cooperative bound.
