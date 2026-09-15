# Technical guide

This is the short route through Octessera's current implementation contracts,
fixed hardware paths, build tools, and provenance. For operating the instrument,
start with the [user manual](../userdocs/README.md).

## Architecture and contracts

- [Runtime boundaries](runtime-boundaries.md) — ownership between the desktop
  UI, native runtime, core, adapters, and realtime audio engine.
- [Menu and controls spec](menu-and-controls-spec.md) — the authoritative
  control, menu, overlay, persistence, and display contract.
- [Menu tree spec](menu-tree-spec.md) — the canonical menu structure.
- [Board profiles](board-profiles.md) — the two fixed board IDs and their
  platform boundaries.
- [Platform capabilities](../resources/platform-capabilities.json) — shared
  dimensions and limits used by native and desktop contracts.

## Hardware and images

- [Hardware pinout and connections](../hardware/docs/pinout-and-connections.md)
  — board wiring, buses, and port ownership.
- [Flash and first boot](../userdocs/hardware/flash-and-first-boot.md) — image
  selection, flashing, network setup, and first power-on.
- [Orange production image and runtime reference](../hardware/docs/orange-pi-production-reference.md)
  — Orange image, service, storage, audio, USB, and updater contracts.
- [Orange Pi input routing](../hardware/docs/orange-pi-input-routing.md) — the
  board-specific UART and encoder-routing overlay.

## Development, build, and deployment

- [Desktop development](workflows/desktop-development.md) — simulator
  development and desktop builds.
- [Pi provisioning](../tools/pi/provision/README.md) — Raspberry OS and boot
  configuration.
- [Board deployment](workflows/deployment.md) — Raspberry deployment and Orange
  input-routing operations.
- [Orange Pi tools](../tools/orange-pi/README.md) — Orange SSH bootstrap and
  cross-build commands.

## Attribution and licensing

- [License](../LICENSE) — terms for original Octessera material.
- [Notice](../NOTICE) — concise standalone-artifact notices.
- [Third-party notices](../THIRD_PARTY_NOTICES.md) — known third-party material.
- [Bundled sample source](../samples/SOURCE.md) — the sample-pack source.
- [Hardware attributions](../hardware/ATTRIBUTIONS.md) — PCB, enclosure, and
  module references.
- [Vendored CPAL provenance](../third_party/cpal-0.15.3/PROVENANCE.md) — the
  source and fixed audio-host modifications for the vendored dependency.
