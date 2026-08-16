# CPAL 0.15.3 provenance

- Package: `cpal` `0.15.3` from crates.io.
- Crates.io package checksum: `873dab07c8f743075e57f524c583985fbaf745602acbe916a01539364369a779`.
- Package source VCS SHA: `ac6cbb2ba55e61665a35ab88ae136a83380d1354`.
- Upstream release: RustAudio/CPAL tag `v0.15.3`, commit
  `5ad71d7ed96fd11dc37cd97205283595320b64ee`.
- License: Apache-2.0, retained in `LICENSE`.

The vendored tree contains the package above plus the following exact modified
files. Each change is limited to Octessera's fixed hardware/audio contracts:

- `src/error.rs` — adds typed busy, unsupported, and fault variants used to
  report ALSA device and stream failures without collapsing them into generic
  backend errors.
- `src/lib.rs` — defines the fixed Raspberry Pi and Orange Pi PCM identities
  and the shared exact-output allowlist.
- `src/host/alsa/enumerate.rs` — preserves null-hint filtering and only yields
  ALSA devices whose handles open successfully, using simpler iterator flow.
- `src/host/alsa/mod.rs` — implements exact PCM opening, ALSA errno
  classification, pause/play semantics, interrupt-safe worker wakeups, and
  non-panicking worker teardown.
- `src/host/wasapi/device.rs` — caches the future WASAPI audio client and
  process-wide device enumerator used by the desktop audio adapter.
