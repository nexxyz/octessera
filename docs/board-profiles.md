# Board profiles

Octessera uses explicit board profile IDs at build and artifact boundaries:

- `raspberry-pi-zero-2w` is the supported Raspberry Pi Zero 2 W profile.
- `orange-pi-zero-2w` identifies the Orange Pi Zero 2W bring-up target.

The board-specific HALs own their physical pin and device descriptors. The Raspberry
canonical Cargo features are `raspberry-pi-zero-2w` and
`hardware-raspberry-pi-zero-2w`; the older `rpi-zero-2w`, `pi-zero`,
`hardware-rpi-zero-2w`, and `hardware-pi` feature names remain compatibility
aliases for now and are covered by CI compile checks. The HAL also exposes the
`orange-pi-zero-2w` profile descriptor and its diagnostic OLED/I2C bring-up
backend. The Orange `octessera-pi` feature is a foreground runtime candidate,
not a service or deployment-ready artifact. Its gpiocdev v2 backend qualifies
SW1, SW2, and SW4 for both-edge A/B input and active-low switch events with
pull-ups. SW3's switch line is explicitly faulted/excluded because physical
pin 8 / H618 PH0 is active UART0 TX until boot routing changes. Audio/I2S
uses the shared 44.1 kHz runtime rate and requires exactly one CPAL output
device named `hw:CARD=octesseradac,DEV=0` with verified stereo support. There
is no default or HDMI fallback. Internal MIDI events are ignored; explicit MIDI
platform actions return typed unavailable status. USB remains unavailable.
Its typed bus descriptors record `/dev/i2c-2` at `5002400.i2c` and
`/dev/spidev1.0` at `5011000.spi`; its encoder descriptor records H618
`300b000.pinctrl` offsets rather than Raspberry GPIO fields.

Raspberry Pi build, deploy, provision, preflight, pi-gen, and Raspberry Pi
Imager packaging tools accept only `raspberry-pi-zero-2w`. They reject
`orange-pi-zero-2w` rather than guessing at pins, GPIO numbering, or an audio
backend. Orange Pi image work stays on the separate Armbian path until
hardware validation supports a real HAL profile.

Pi binaries expose `--print-build-metadata`, and cross-build output includes
`octessera-pi.metadata.json`. Release manifests, installed service metadata,
and device update manifests carry the same canonical ID so a mismatched
binary or artifact fails closed where the host can check it.

The Orange AArch64 artifacts are the diagnostic-only OLED and Seesaw smoke
utilities plus the hash-bound `octessera-pi` `runtime-candidate`. Every artifact
is `runtime_ready=false`; the builder still rejects deployment/service-ready
metadata and never installs or enables a service.
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

The smoke artifacts remain diagnostic-only. The foreground candidate is a
hash-bound `runtime-candidate`, not a normal runtime: it requires the exact
CPAL output device `hw:CARD=octesseradac,DEV=0` at the shared 44.1 kHz runtime
rate with verified stereo support, routes musical events/audio commands/silence
through the existing realtime engine, and ignores internal MIDI events.
Explicit MIDI platform actions return typed unavailable status. There is no
default or HDMI fallback.
Its qualified encoder path emits the existing `HardwareEvent` contract into
native runtime input; it has no updater, service, reboot, SD-transfer, USB, or
MIDI enumeration. SIGINT
performs the normal shutdown frame, bounded 750 ms black-LED cleanup/retry, and
OLED-off acknowledgement; workers join only after an acknowledgement and are
not joined after a cleanup timeout. Synchronous device calls can still outlast
that cooperative bound. Do not deploy it as a service or use it as evidence
for audio, USB, encoder, or normal background-runtime qualification.
