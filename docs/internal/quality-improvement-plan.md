# Quality Improvement Plan

Octessera is a hobbyist instrument. This backlog covers only the desktop
simulator and the fixed Raspberry Pi and Orange Pi boards; it does not propose
enterprise release machinery or general hardware abstractions.

## Current status

- Completed on PC: F1 sample-browser parent ownership, F10 configuration
  persistence fixtures, D7 shared sample decoding, D8 shared Rust capability
  loading, D9 generated Rust palette ownership, D2 shared engine finalization,
  D4 exhaustive grid projections, T2 persisted configuration DTO and transaction
  ownership work, T5 syntax-aware JS/TS quality metrics, and T6 direct rodio CI
  gates. F2's bounded desktop platform-request admission is complete.
- T1's Seesaw-specific transport seam and deterministic driver tests are
  complete. Remaining Raspberry and Orange physical qualification work is
  tracked only in [docs/open-work.md](../open-work.md).
- T8's canonical internal Cargo migration is complete; compatibility aliases
  remain retained for existing commands and deployments.
- T2 persisted configuration DTO and transaction ownership work is complete,
  including F10 migration, validation, opacity, and ownership coverage.
- Configuration transaction terminology remains where it names the existing
  atomic apply/persist boundary; Phase 3 makes no behavior-free vocabulary
  rename.
- D3 native NeoKey colors and D6 native transient timing are complete after
  desktop/Pi replay, both production cross-builds, and the portable desktop
  rebuild. Physical color, brightness, and timing observation remains deferred.
  D5 native OLED frame production and the Stage 2 consumer cutover are complete
  in software. Positive snapshot revisions, changed-frame-only publication,
  retained-last-good/black-first cache policy, sticky conflict recovery, and
  native-only Pi/desktop consumers pass the playback, board-profile, strict
  audit, and documentation gates. Physical display qualification remains
  deferred.

## Current validation environment

- Orange Pi is reachable and may be used now for SSH, hash-bound diagnostic
  artifacts, transient units, logs, readiness checks, automated on-device tests,
  and the attended observation needed for the planned manual FAT. Preserve and
  restore the running service around any exclusive hardware access.
- Raspberry Pi is down. Raspberry implementation, host tests, feature checks,
  and AArch64 cross-builds remain available, but every Raspberry on-device claim
  is deferred.
- Manual FAT is planned and pending, not unavailable. Orange button/encoder
  actuation, cabling, listening, and visual LED/OLED confirmation can be
  scheduled now; Raspberry physical intervention remains deferred while that
  board is down. The current FAT action list lives only in [docs/open-work.md](../open-work.md).
- Parity changes may proceed when PC tests cover both profiles and Orange can
  supply useful automated runtime evidence. Record the missing Raspberry and
  physical evidence explicitly instead of blocking independent work.
- The repository has hash-bound Orange cross-build and diagnostic primitives but
  no standard production deploy command. Use reviewed transient/diagnostic flows
  rather than inventing an ad hoc service replacement.

## Non-actionable implementation context

The following completed software slices are retained as historical context, not
as a current action list. Current FAT actions live only in [docs/open-work.md](../open-work.md).

1. F2 platform-request saturation/recovery is complete.
2. T8's canonical internal Cargo migration is complete; retain compatibility
   aliases and continue validating both cross-build profiles.
3. T2 persisted configuration DTO and transaction ownership work is complete
   using F10 fixtures and automated config/load-store evidence. Defer Raspberry
   persistence validation to the [two-board FAT orchestrator](../../userdocs/hardware/fat-quick-run.md).
4. D3 NeoKey semantic colors and D6 transient timing are complete. D5 native
   OLED production and the Stage 2 consumer cutover are software-complete after
   the full playback, board-profile, strict audit, and documentation gate. Do
   not infer physical display qualification from software evidence; defer
   physical appearance.

## Functional quality

Current FAT actions are maintained only in [docs/open-work.md](../open-work.md);
begin there with the [two-board FAT orchestrator](../../userdocs/hardware/fat-quick-run.md).
This plan does not duplicate the FAT action list.

**Non-actionable software status:** D5 Stage 2 is software-complete. Positive `oledFrameRevision` on every emitted
PlaybackRuntime snapshot, changed-frame-only OLED messages, black-first and
retain-last-good Pi/Orange publication, sticky conflict recovery, and
native-only Pi/desktop consumers pass the full playback, board-profile, strict
audit, and documentation checks. Physical display qualification remains
deferred.

## Duplication reduction

1. **Create one non-realtime audio-config compiler in realtime-engine.** **Action:** Define the shared normalization/preparation boundary and migrate one host adapter at a time while leaving path resolution and device I/O in adapters. **Evidence:** `crates/realtime-engine/src/synth/audio_config.rs`; `apps/desktop/src-tauri/src/audio_config.rs`; `apps/desktop/src-tauri/src/audio_prep_config.rs`; `apps/pi-zero/src/audio_config_parse.rs`; `apps/pi-zero/src/host_audio_prep.rs`. **Value:** Removes drift between desktop and Pi audio configuration. **Risk:** High; double application, sample churn, and routing regressions are possible. **Sequence:** Migrate one adapter at a time after functional configuration fixtures.
2. **Make NeoKey LED colors canonical in runtime snapshots.** **Complete:** Native snapshots now publish unscaled `neoKeyLeds`; Pi and desktop consume them with one basis-point scaling rule and golden states.
3. **Retire semantic OLED rendering from normal consumers.** **D5 Stage 2:**
   normal Pi and desktop runtime consumers use accepted native pixel frames and
   positive snapshot references; desktop TypeScript remains a snapshot/input
   adapter and has no playback or control fallback. The semantic renderer is
   retained only for parity/reference coverage and adapter-owned lifecycle or
   fault paths where applicable. The software gate is complete; physical
   display qualification remains deferred.
4. **Move transient indicator timing from TypeScript to playback-runtime.** **Complete:** Native monotonic deadlines, pending transition snapshots, timed host expiry requests, and atomic pulse removal are covered by no-sleep native and adapter tests.
5. **Pilot desktop wire DTO generation from the Rust protocol.** **Action:** Generate one desktop status-union DTO from the native protocol and compare fixtures before expanding coverage. **Evidence:** `packages/device-contracts/src/runtimeProtocol.ts`; `crates/playback-runtime/src/protocol` modules. **Value:** Reduces manual bridge drift. **Risk:** High; wire-shape or generated-type changes can break desktop adapters. **Sequence:** Pilot one status union with wire fixtures before broader generation.

## Broader technical debt

1. **Improve domain cohesion inside NativeRunner.** **Action:** Extract only bounded domain modules from `native_runner.rs` and sibling impl modules, preserving message ordering and ownership boundaries. **Evidence:** `crates/playback-runtime/src/native_runner.rs`; `crates/playback-runtime/src/native_runner/`. **Value:** Makes future runtime changes easier to review. **Risk:** Broad extraction can hide state coupling or alter dispatch order. **Sequence:** Incremental slices only when a cohesive runtime-owned boundary is clear.
2. **Add a profile-aware hardware diagnostic harness.** **Action:** Extend the fixed-board diagnostic entry point with board descriptors and bounded component checks. **Evidence:** `apps/pi-zero/src/hardware_test.rs`; `docs/board-profiles.md` board descriptors. **Value:** Makes manual board qualification repeatable. **Risk:** Hardware-dependent and easy to over-generalize. **Sequence:** After the fixed-board smoke evidence and before the manual run.
3. **Retire legacy Cargo feature aliases.** **Action:** Inventory aliases, migrate supported profiles and CI, then remove only unused compatibility names. **Evidence:** `docs/board-profiles.md`; workspace/CI feature declarations. **Value:** Reduces build-profile ambiguity. **Risk:** A hidden deployment or cross-build can lose its feature. **Sequence:** Inventory first; remove after profile checks.
4. **Contain and reassess vendored CPAL.** **Action:** Document the exact reason and boundary for the vendored CPAL copy, then reassess removal only after PCM qualification. **Evidence:** `Cargo.toml`; `third_party/cpal-0.15.3/PROVENANCE.md`. **Value:** Keeps the audio dependency intentional and maintainable. **Risk:** Removing it before qualification could change exact PCM behavior. **Sequence:** Never remove before exact PCM qualification.
5. **Retire trusted-parent respin machinery after constructor-image qualification.** **Action:** Remove or narrow the respin workflow only when constructor image derivations and recovery evidence are qualified. **Evidence:** `tools/image-respin`; `resources/image-construction/`; respin workflow. **Value:** Reduces maintenance after the image contract is proven. **Risk:** Premature removal could lose recovery or reproducibility. **Sequence:** Preserve recovery until constructor-image qualification is complete.
