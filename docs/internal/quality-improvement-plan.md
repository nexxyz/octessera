# Quality Improvement Plan

This page tracks active technical debt for the desktop simulator and the two
fixed Pi boards. Physical FAT work is maintained in [open-work.md](../open-work.md).

## Deferred until after FAT

- **Shared audio-config compiler:** Create one non-realtime normalization and
  preparation boundary in `realtime-engine`, then migrate host adapters without
  moving path resolution or device I/O out of the adapters. This is high risk:
  double application, sample churn, and routing regressions are possible.
- **Desktop wire-DTO pilot:** Generate one desktop status-union DTO from the Rust
  protocol and compare fixtures before expanding coverage. Defer this high-risk
  bridge change until after FAT.

## Review-only

- **NativeRunner cohesion:** The composition/state root is already decomposed.
  Review only; inspect `crates/playback-runtime/src/native_runner.rs`, its
  focused `native_runner/` modules, and parity/order-sensitive `device_input.rs`.
  No cohesive extraction was identified. A future extraction requires a clear
  ownership cluster plus characterization tests.

## Documented dispositions

- The existing `apps/pi-zero/src/fat_diagnostic` is the canonical profile-aware
  fixed-board diagnostic owner. The current exact v0.8.1 FAT uses this passive
  harness unchanged; no pre-FAT replacement or extension is justified.
- Cargo alias inventory and documentation are done in the stable board-profile
  docs. Compatibility aliases remain accepted for existing commands; no removal
  is planned.
- The vendored CPAL rationale is documented in
  [runtime-boundaries.md](../runtime-boundaries.md). Reassess it only after exact
  PCM FAT; do not remove or rebase it before then.

## Post-FAT

- Keep the Orange current-parent respin lane nonpublishing and boot-neutral until
  exact constructor-image qualification and FAT close.
