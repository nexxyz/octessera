# Technical guide

This is the short route through Octessera's implementation contracts, fixed
hardware paths, build tools, and provenance. It is written for people changing
the code. For playing or building the instrument, start with the
[user guide](../userdocs/README.md) instead; it is friendlier.

## Repository map

- `apps/`: platform applications and hardware adapters.
- `crates/`: native Rust core, runtime, and audio crates.
- `packages/`: shared TypeScript packages and desktop-facing contracts.
- `config/` + `resources/`: shipped defaults, generated configuration, and shared data.
- [`hardware/`](../hardware/README.md): canonical PCB, enclosure, and hardware documentation.
- `userdocs/`: end-user build, controls, and operation guides.
- `docs/`: technical specifications, workflows, and contributor references.
- `tools/`: build, image, hardware, release, and verification tooling.
- `samples/`: the bundled WAV sample library and its upstream licence.
- `licenses/` + `third_party/`: generated dependency inventories and vendored/source provenance.
- `assets/`: shared branding and device visual assets.
- `userpatches/`: Armbian image overlays and device files; it stays at the root because Armbian consumes this conventional path.

## Architecture and contracts

- [Runtime boundaries](runtime-boundaries.md): ownership between the desktop
  UI, native runtime, core, adapters, and realtime audio engine.
- [Menu and controls spec](menu-and-controls-spec.md): the authoritative
  control, menu, overlay, persistence, and display contract.
- [Menu tree spec](menu-tree-spec.md): the canonical menu structure.
- [Board profiles](board-profiles.md): the two fixed board IDs and their
  platform boundaries.
- [Platform capabilities](../resources/platform-capabilities.json): shared
  dimensions and limits used by native and desktop contracts.

## Hardware and images

- [Hardware pinout and connections](../hardware/docs/pinout-and-connections.md):
  board wiring, buses, and port ownership.
- [Flash and first boot](../userdocs/hardware/flash-and-first-boot.md): image
  selection, flashing, network setup, and first power-on.
- [Orange production image and runtime reference](../hardware/docs/orange-pi-production-reference.md):
  Orange image, service, storage, audio, USB, and updater contracts.
- [Orange Pi input routing](../hardware/docs/orange-pi-input-routing.md): the
  board-specific UART and encoder-routing overlay.

## Development, build, and deployment

- [Development workflows](development-workflows.md): install, checks,
  generators, and the pre-push hook.
- [Desktop development](workflows/desktop-development.md): simulator
  development and desktop builds.
- [Pi development and profiling](workflows/pi-development-and-profiling.md):
  host builds, the shared-path timing smoke, and board profiling.
- [Pi provisioning](../tools/pi/provision/README.md): Raspberry OS and boot
  configuration.
- [Board deployment](workflows/deployment.md): Raspberry and Orange
  development deployment and Orange input routing.
- [Orange Pi tools](../tools/orange-pi/README.md): Orange SSH bootstrap,
  cross-builds, and study tools.
- [Image construction](workflows/image-construction-and-proof.md) and
  [release assembly](workflows/release-assembly.md): building board images
  and release assets.

## Attribution and licensing

- [License](../LICENSE): terms for original Octessera material.
- [Notice](../NOTICE): concise standalone-artifact notices.
- [Third-party notices](../THIRD_PARTY_NOTICES.md): known third-party material.
- [Bundled sample pack](https://github.com/stargatedaw/stargate-sample-pack): the attribution-free source of the bundled samples.
- [Hardware attributions](../hardware/ATTRIBUTIONS.md): PCB, enclosure, and
  module references.
- [Vendored CPAL provenance](../third_party/cpal-0.15.3/PROVENANCE.md): the
  source and fixed audio-host modifications for the vendored dependency.
