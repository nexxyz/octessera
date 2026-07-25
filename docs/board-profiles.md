# Board profiles

Octessera uses explicit board profile IDs at build and artifact boundaries:

- `raspberry-pi-zero-2w` is the supported Raspberry Pi Zero 2 W profile.
- `orange-pi-zero-2w` identifies the Orange Pi Zero 2W bring-up target.

The Raspberry Pi HAL owns the physical pin and device descriptors. Its
canonical Cargo features are `raspberry-pi-zero-2w` and
`hardware-raspberry-pi-zero-2w`; the older `rpi-zero-2w`, `pi-zero`,
`hardware-rpi-zero-2w`, and `hardware-pi` feature names remain compatibility
aliases for now and are covered by CI compile checks. The HAL also exposes the
`orange-pi-zero-2w` profile descriptor and its diagnostic-only OLED/I2C
bring-up backend. There is no Orange `octessera-pi` runtime feature or service
artifact. Encoder/Seesaw inputs and audio/I2S remain unqualified.
Its typed bus descriptors record `/dev/i2c-2` at `5002400.i2c` and
`/dev/spidev1.0` at `5011000.spi`; they do not fill Raspberry GPIO fields.

Raspberry Pi build, deploy, provision, preflight, pi-gen, and Raspberry Pi
Imager packaging tools accept only `raspberry-pi-zero-2w`. They reject
`orange-pi-zero-2w` rather than guessing at pins, GPIO numbering, or an audio
backend. Orange Pi image work stays on the separate Armbian path until
hardware validation supports a real HAL profile.

Pi binaries expose `--print-build-metadata`, and cross-build output includes
`octessera-pi.metadata.json`. Release manifests, installed service metadata,
and device update manifests carry the same canonical ID so a mismatched
binary or artifact fails closed where the host can check it.

The only Orange AArch64 artifact is the diagnostic-only OLED smoke utility; it
is separate from the native Pi runtime and cannot be installed as a service:

```sh
cross build --release --target aarch64-unknown-linux-gnu -p octessera-hal --features orange-pi-zero-2w --bin orange-oled-smoke
sudo ./target/aarch64-unknown-linux-gnu/release/orange-oled-smoke --confirm-active-test
```

One invocation performs one bounded `pattern → black → display-off` operation.
The utility owns cleanup on errors and handled interruption; do not split the
sequence into separate commands.

The second command is an active hardware test. It must not be run against an
unverified device or wiring harness.

The Orange artifact is diagnostic-only and is not runtime-ready. Orange
runtime/HAL integration, deployment, release packaging, and service enablement
remain blocked until real qualified adapters exist.
