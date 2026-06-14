# Native Test Parity Matrix

Status: `in-progress`

This matrix tracks legacy TypeScript tests that still describe shipped native behavior and their Rust/native counterparts.

Legend:

- `covered`: native Rust/Tauri tests cover the behavior.
- `partial`: native tests cover the broad area but not all legacy cases.
- `missing`: no native counterpart yet.
- `legacy-only`: legacy TS path no longer describes shipped behavior.

## Behavior Packages

| Legacy test file | Native status | Target |
| --- | --- | --- |
| `packages/behaviors-none/tests/none.test.ts` | covered | `crates/platform-core/src/behaviors/none.rs` |
| `packages/behaviors-life/tests/life.test.ts` | covered | `crates/platform-core/src/behaviors/life.rs` |
| `packages/behaviors-sequencer/tests/sequencer.test.ts` | covered | `crates/platform-core/src/behaviors/sequencer.rs` |
| `packages/behaviors-keys/tests/keys.test.ts` | covered | `crates/platform-core/src/behaviors/ported/keys.rs` |
| `packages/behaviors-brain/tests/brain.test.ts` | covered | `crates/platform-core/src/behaviors/ported/brain.rs` |
| `packages/behaviors-ant/tests/ant.test.ts` | covered | `crates/platform-core/src/behaviors/ported/ant.rs` |
| `packages/behaviors-bounce/tests/bounce.test.ts` | covered | `crates/platform-core/src/behaviors/ported/bounce.rs` |
| `packages/behaviors-pulse/tests/shapes.test.ts` | covered | `crates/platform-core/src/behaviors/ported/shapes.rs` |
| `packages/behaviors-raindrops/tests/raindrops.test.ts` | covered | `crates/platform-core/src/behaviors/ported/raindrops.rs` |
| `packages/behaviors-dla/tests/dla.test.ts` | covered | `crates/platform-core/src/behaviors/ported/dla.rs` |
| `packages/behaviors-glider/tests/glider.test.ts` | covered | `crates/platform-core/src/behaviors/glider.rs` |

## Core Packages

| Legacy test file | Native status | Target |
| --- | --- | --- |
| `packages/interpretation-core/tests/interpretation.test.ts` | covered | `crates/platform-core/src/interpretation.rs` |
| `packages/mapping-core/tests/mapping.test.ts` | covered | `crates/platform-core/src/mapping.rs` |
| `packages/device-contracts/tests/contracts.test.ts` | covered | `crates/platform-core/src/grid.rs`, `crates/playback-runtime/src/protocol.rs` |
| `packages/musical-events/tests/events.test.ts` | covered | `crates/platform-core/src/events.rs` |
| `packages/behavior-api/tests/registry.test.ts` | legacy-only | TS registration side effects are not part of shipped native behavior loading; native registry coverage lives in `crates/platform-core/src/behaviors/mod.rs`. |

## Platform Core Runtime/Menu

| Legacy test file | Native status | Target |
| --- | --- | --- |
| `packages/platform-core/tests/features-core.test.ts` | covered | `crates/playback-runtime/src/native_runner/tests.rs` |
| `packages/platform-core/tests/features-runtime.test.ts` | covered | `crates/playback-runtime/src/native_runner/tests.rs` |
| `packages/platform-core/tests/features-input-transitions.test.ts` | covered | `crates/platform-core/src/engine.rs`, `crates/playback-runtime/src/native_runner/tests.rs` |
| `packages/platform-core/tests/features-aux.test.ts` | covered | `crates/playback-runtime/src/native_runner/tests.rs` |
| `packages/platform-core/tests/features-toast.test.ts` | covered | `crates/playback-runtime/src/native_runner/tests.rs` |
| `packages/platform-core/tests/logic-core.test.ts` | covered | `crates/platform-core`, `crates/playback-runtime` |
| `packages/platform-core/tests/logic-ui.test.ts` | covered | `crates/playback-runtime/src/native_runner/tests.rs`, `crates/playback-runtime/src/native_menu/tests.rs` |
| `packages/platform-core/tests/menuHelp.test.ts` | covered | `crates/playback-runtime/src/native_runner/tests.rs` |
| `packages/platform-core/tests/grid-domain.test.ts` | covered | `crates/platform-core/src/grid.rs` |
| `packages/platform-core/tests/xy-pad.test.ts` | legacy-only | TS-only `applyXyModulation` helper; shipped native runtime currently covers Dance XY page selection/grid interaction, not TS helper mutation paths. |
| `packages/platform-core/tests/combinedModifier.test.ts` | covered | `crates/playback-runtime/src/native_runner/tests.rs` |

## Runner/Desktop Bridge

| Legacy test file | Native status | Target |
| --- | --- | --- |
| `packages/platform-core-runner/tests/runner.test.ts` | covered | `crates/playback-runtime/src/lib.rs`, `crates/playback-runtime/src/native_runner/tests.rs` |
| `apps/desktop/tests/*.test.ts` | covered | `apps/desktop/src-tauri/src/*_tests.rs`, `apps/desktop/tests/*.test.ts` |

## First Batch Checklist

- [x] Behavior package parity tests ported for deterministic legacy cases.
- [x] Interpretation/mapping missing tests ported.
- [x] Matrix statuses updated after tests land.
- [x] `cargo test -p platform-core` passes.
- [x] Runtime/config payload parity tests ported.
- [x] Aux/input transition parity tests ported.
- [x] Sample/probability/menu/help/OLED/desktop bridge parity covered by native tests or classified as legacy-only.

Resolved behavior package notes:

- `ant`: deterministic movement/wrap/max tests and menu/action availability are covered. Legacy random `spawnAnt` exact placement is intentionally not treated as a stable shipped contract.
- `bounce`: deterministic movement/right-edge/max tests and menu/action availability are covered. Legacy random `addBall` exact placement is intentionally not treated as a stable shipped contract.
- `shapes`: deterministic grid pulse, expansion, config tests, and menu/action availability are covered. Legacy random `spawnPulse` exact placement is intentionally not treated as a stable shipped contract.
- `behavior-api`: legacy TS registration side effects are classified legacy-only because native behaviors are statically registered in Rust for the shipped runtime.
