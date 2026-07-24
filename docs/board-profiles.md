# Board profiles

Octessera uses explicit board profile IDs at build and artifact boundaries:

- `raspberry-pi-zero-2w` is the supported Raspberry Pi Zero 2 W profile.
- `orange-pi-zero-2w` identifies the Orange Pi Zero 2W bring-up target.

The Raspberry Pi HAL currently owns the physical pin and device descriptors.
Its canonical Cargo features are `raspberry-pi-zero-2w` and
`hardware-raspberry-pi-zero-2w`; the older `rpi-zero-2w`, `pi-zero`,
`hardware-rpi-zero-2w`, and `hardware-pi` feature names remain compatibility
aliases for now and are covered by CI compile checks.

Raspberry Pi build, deploy, provision, preflight, pi-gen, and Raspberry Pi
Imager packaging tools accept only `raspberry-pi-zero-2w`. They reject
`orange-pi-zero-2w` rather than guessing at pins, GPIO numbering, or an audio
backend. Orange Pi image work stays on the separate Armbian path until
hardware validation supports a real HAL profile.

Pi binaries expose `--print-build-metadata`, and cross-build output includes
`octessera-pi.metadata.json`. Release manifests, installed service metadata,
and device update manifests carry the same canonical ID so a mismatched
binary or artifact fails closed where the host can check it.
