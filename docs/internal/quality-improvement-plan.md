# Quality Improvement Plan

Octessera is a hobbyist instrument. This backlog covers only the desktop
simulator and the fixed Raspberry Pi and Orange Pi boards; it does not propose
enterprise release machinery or general hardware abstractions.

## Current status

- Completed on PC: F1 sample-browser parent ownership, F10 configuration
  persistence fixtures, D7 shared sample decoding, D8 shared Rust capability
  loading, D9 generated Rust palette ownership, D2 shared engine finalization,
  D4 exhaustive grid projections, T5 syntax-aware JS/TS quality metrics, and T6
  direct rodio CI gates. F2 now has bounded desktop platform-request admission;
  its policy-sensitive queues remain open work.
- T1's Seesaw-specific transport seam and deterministic driver tests are
  complete. Raspberry and Orange physical control/LED/latency smokes remain
  unqualified and stay in `docs/open-work.md`.
- T8's canonical internal Cargo migration is complete; compatibility aliases
  remain retained for existing commands and deployments.
- T2's outer/application configuration DTO slice is complete, including F10
  migration, validation, opacity, and ownership coverage. Nested
  runtime/layer/instrument/mixer/device typing remains.
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
  artifacts, transient units, logs, readiness checks, and automated on-device
  tests that require no physical observation. Preserve and restore the running
  service around any exclusive hardware access.
- Raspberry Pi is down. Raspberry implementation, host tests, feature checks,
  and AArch64 cross-builds remain available, but every Raspberry on-device claim
  is deferred.
- Physical intervention is unavailable. Button/encoder actuation, cabling,
  listening, and visual LED/OLED confirmation remain deferred on both boards.
- Parity changes may proceed when PC tests cover both profiles and Orange can
  supply useful automated runtime evidence. Record the missing Raspberry and
  physical evidence explicitly instead of blocking independent work.
- The repository has hash-bound Orange cross-build and diagnostic primitives but
  no standard production deploy command. Use reviewed transient/diagnostic flows
  rather than inventing an ad hoc service replacement.

## Execution order under these constraints

1. Run the automated Orange portion of the T1 Seesaw smoke: exact bus/address
   identity, hardware IDs, bounded reads/writes, timeout behavior, shutdown, and
   service restoration. Defer physical inputs, coordinates, and LED appearance.
2. D9 palette generation is complete with exact generated-byte checks.
3. D2 tick/input finalization is complete with platform/runtime replays.
4. F2 platform-request saturation/recovery is complete. Remaining queues await
   explicit loss, retry, emergency, and shutdown semantics in `docs/open-work.md`.
5. D4 grid projection is complete with exhaustive PC fixtures and Orange-profile
   synthetic mapping tests. Physical orientation remains deferred.
6. T8's canonical internal Cargo migration is complete; retain compatibility
   aliases and continue validating both cross-build profiles.
7. T2's outer/application DTO slice is complete using F10 fixtures and
   automated config/load-store evidence. Continue with nested
   runtime/layer/instrument/mixer/device typing; defer Raspberry persistence
   validation.
8. D3 NeoKey semantic colors and D6 transient timing are complete. D5 native
   OLED production and the Stage 2 consumer cutover are software-complete after
   the full playback, board-profile, strict audit, and documentation gate. Do
   not infer physical display qualification from software evidence; defer
   physical appearance.

## Functional quality

D5 Stage 2 is software-complete. Positive `oledFrameRevision` on every emitted
PlaybackRuntime snapshot, changed-frame-only OLED messages, black-first and
retain-last-good Pi/Orange publication, sticky conflict recovery, and
native-only Pi/desktop consumers pass the full playback, board-profile, strict
audit, and documentation checks. Physical display qualification remains
deferred.

1. **Remove the duplicate Pi sample-browser parent row.** **Action:** Keep Pi `sample_entries` limited to folders/files and let the native sample-browser menu own the single `..` row and selection path. **Evidence:** `apps/pi-zero/src/sample_browser.rs::sample_entries`; `crates/playback-runtime/src/native_menu/sample_browser_menu.rs`. **Value:** Low-risk immediate parity. **Risk:** A row-count or focus regression could affect navigation. **Sequence:** Immediate; verify browser fixtures before other sample-browser work.
2. **Bound desktop producer queues.** **Action:** Add bounded, typed admission at the producer boundaries without weakening panic or stop operations. **Evidence:** `apps/desktop/src-tauri/src/runtime_worker.rs`; `apps/desktop/src-tauri/src/desktop_platform_service.rs`; `apps/desktop/src-tauri/src/audio_prep_service.rs`. **Value:** High; prevents unbounded pressure from becoming a runtime failure. **Risk:** High event-drop risk; coalescing and ordering must remain explicit. **Sequence:** Admission and saturation tests first, then the smallest queue changes.
3. **Hardware-qualify the exact selected audio routes on both boards.** **Action:** Run the fixed Jack, USB Audio, and HDMI output checks where selected, record exact device identities, and exercise endpoint loss/recovery and independent clocks. **Evidence:** `docs/open-work.md`; `docs/board-profiles.md`. **Value:** Establishes the real audio paths before dependent claims. **Risk:** Hardware-dependent; cabling, image, and ALSA state can block the run. **Sequence:** Before sample or FX qualification.
4. **Qualify Pi sample preview, assignment, banks, corrupt files, and revisions.** **Action:** Exercise preview and assignment through host preparation, including malformed files and revision replacement while preserving the prior valid bank. **Evidence:** `apps/pi-zero/src/host_audio_prep.rs`; `host_audio_preview_prep.rs`; `audio_config_parse.rs` and `audio_config_parse/`. **Value:** Protects the sample workflow and failure behavior. **Risk:** Hardware-dependent after DAC qualification; decode and storage timing can expose races. **Sequence:** After item 3 and before broader audio UX claims.
5. **Replay complete controls, coordinates, Fn, encoders, and overlays.** **Action:** Run the fixed control-surface replay for grid coordinates, modifiers, encoder actions, and overlay priority. **Evidence:** `crates/hal/src/neotrellis.rs`; `docs/menu-and-controls-spec.md`. **Value:** Confirms hardware/software input parity. **Risk:** Hardware-dependent; coordinate or modifier drift can be musically visible. **Sequence:** After board input bring-up and before relying on simulator-only controls.
6. **Physically qualify normal OLED rows, clipping, help, and brightness.** **Action:** Check normal rendering, long rows, help layouts, clipping, and brightness on the target display. **Evidence:** `apps/pi-zero/src/render.rs`; `crates/playback-runtime/src/native_runner.rs` display snapshot; `docs/menu-and-controls-spec.md`. **Value:** Confirms the user-facing display contract. **Risk:** Hardware-dependent after display verification; pixel geometry may reveal hidden overlap. **Sequence:** Before boot-handoff qualification.
7. **Qualify fresh-image boot OLED handoff, restart, locks, and sleep.** **Action:** Exercise the boot-to-runtime handoff and failure/restart paths on fresh images, including lock cleanup and sleep/resume. **Evidence:** `apps/pi-zero/src/boot_oled_handoff_unix.rs`; `resources/oled/boot-sweep-v1.json`. **Value:** Protects first boot and lifecycle reliability. **Risk:** Hardware/image-dependent; orphaned writers or locks can strand the device. **Sequence:** After item 6 and with fresh constructor images.
8. **Qualify the setup portal on both boards.** **Action:** Test AP creation, captive setup, credential application, reconnect, already-running attachment, timeout, failure, and secret absence. **Evidence:** `apps/pi-zero/src/setup_portal*.rs`; `docs/board-profiles.md`; `docs/open-work.md`. **Value:** Validates practical bring-up without a keyboarded rescue path. **Risk:** Hardware/network-dependent; network state and partial writes complicate recovery. **Sequence:** After both board images and runtime service bring-up.
9. **Qualify external MIDI routing and panic.** **Action:** Exercise external MIDI output, selection, disconnect handling, and `panic_external_midi` with a real external device. **Evidence:** `apps/pi-zero/src/host_adapter.rs::panic_external_midi`; `apps/pi-zero/src/midi_host.rs`. **Value:** Confirms the external-instrument safety path. **Risk:** External-device-dependent; failure must not be hidden as internal audio success. **Sequence:** After controls are qualified and before MIDI user guidance.
10. **Add configuration migration and round-trip fixtures.** **Action:** Cover legacy migration, validation, apply payloads, defaults, and serialization round trips with bounded fixtures. **Evidence:** `crates/playback-runtime/src/native_runner/config_schema.rs`; `modulation_migration.rs`; `apply_payload_*.rs`. **Value:** Protects saved work and future configuration cleanup. **Risk:** Schema compatibility regressions can invalidate user data. **Sequence:** Before nested DTO or configuration refactors.

## Duplication reduction

1. **Create one non-realtime audio-config compiler in realtime-engine.** **Action:** Define the shared normalization/preparation boundary and migrate one host adapter at a time while leaving path resolution and device I/O in adapters. **Evidence:** `crates/realtime-engine/src/synth/audio_config.rs`; `apps/desktop/src-tauri/src/audio_config.rs`; `apps/desktop/src-tauri/src/audio_prep_config.rs`; `apps/pi-zero/src/audio_config_parse.rs`; `apps/pi-zero/src/host_audio_prep.rs`. **Value:** Removes drift between desktop and Pi audio configuration. **Risk:** High; double application, sample churn, and routing regressions are possible. **Sequence:** Migrate one adapter at a time after functional configuration fixtures.
2. **Consolidate platform-core tick and input finalization.** **Action:** Extract shared finalization from the tick and input paths while preserving marker and input distinctions. **Evidence:** `crates/platform-core/src/engine.rs` tick/input finalization. **Value:** Reduces duplicated interpretation and post-processing rules. **Risk:** Held-note, metadata, or marker parity can regress. **Sequence:** Pair tick/input metadata tests first; then make the smallest extraction.
3. **Make NeoKey LED colors canonical in runtime snapshots.** **Complete:** Native snapshots now publish unscaled `neoKeyLeds`; Pi and desktop consume them with one basis-point scaling rule and golden states.
4. **Generate the canonical grid projection.** **Action:** Generate one shared projection for world/display conversion and keep the lower-left semantics explicit at each adapter. **Evidence:** `packages/device-contracts/src/gridDomain.ts`; `crates/playback-runtime/src/native_runner/grid_coords.rs`; `crates/hal/src` grid mapping. **Value:** Removes coordinate duplication and parity drift. **Risk:** A flipped axis or display index would alter every cell. **Sequence:** Exhaustive 64-cell fixtures first; then migrate adapters.
5. **Retire semantic OLED rendering from normal consumers.** **D5 Stage 2:**
   normal Pi and desktop runtime consumers use accepted native pixel frames and
   positive snapshot references; desktop TypeScript remains a snapshot/input
   adapter and has no playback or control fallback. The semantic renderer is
   retained only for parity/reference coverage and adapter-owned lifecycle or
   fault paths where applicable. The software gate is complete; physical
   display qualification remains deferred.
6. **Move transient indicator timing from TypeScript to playback-runtime.** **Complete:** Native monotonic deadlines, pending transition snapshots, timed host expiry requests, and atomic pulse removal are covered by no-sleep native and adapter tests.
7. **Share sample decoding in rodio-engine-source.** **Action:** Let `rodio-engine-source` own shared non-realtime file opening/decoding while hosts retain path resolution, containment, caching, preparation, and error policy. **Evidence:** `crates/rodio-engine-source/src/sample_decode.rs`; desktop and Pi audio preparation adapters. **Value:** Reduces duplicate decode behavior at low risk. **Risk:** Format, error, and buffer-lifetime differences can affect previews. **Sequence:** Compare existing decoder fixtures, then migrate one adapter at a time.
8. **Consolidate platform-capability parsing.** **Action:** Use one Rust build-time source loader and primitive validator while keeping crate-specific Rust emission and the Node generator independent. **Evidence:** `crates/platform-capabilities-build`; `crates/platform-core/build.rs`; `crates/realtime-engine/build.rs`. **Value:** Prevents duplicated Rust build-script validation drift. **Risk:** Generated output or build-profile changes can break contracts. **Sequence:** Inventory and golden-compare current outputs before consolidation.
9. **Generate Rust palette constants from the existing palette generator.** **Action:** Extend the existing generator/build boundary to produce Rust constants and compare them with current palette values. **Evidence:** `crates/platform-core/build.rs`; `tools/resources/generate-display-palette.mjs`. **Value:** Keeps OLED, simulator, and native palette values aligned. **Risk:** Color or brightness changes are immediately visible on hardware. **Sequence:** Golden comparison first; regenerate only after exact equality.
10. **Pilot desktop wire DTO generation from the Rust protocol.** **Action:** Generate one desktop status-union DTO from the native protocol and compare fixtures before expanding coverage. **Evidence:** `packages/device-contracts/src/runtimeProtocol.ts`; `crates/playback-runtime/src/protocol` modules. **Value:** Reduces manual bridge drift. **Risk:** High; wire-shape or generated-type changes can break desktop adapters. **Sequence:** Pilot one status union with wire fixtures before broader generation.

## Broader technical debt

1. **Add a fakeable Seesaw-specific I2C transport.** **Action:** Inject a narrow address/write/write-read/timing transport into the fixed NeoTrellis and NeoKey drivers; do not create a generic HAL. **Evidence:** `crates/hal/src/neotrellis.rs`; `crates/hal/src/neokey.rs`. **Value:** Enables deterministic host tests for exact transactions and failures. **Risk:** Hardware behavior can diverge. **Sequence:** Host transport tests first; board smoke last.
2. **Complete nested persisted NativeRunner DTOs.** **Action:** Type the remaining runtime, layer, instrument, mixer, and device payloads while retaining validated extension JSON for behavior-specific fields; the outer/application DTO slice is complete. **Evidence:** `crates/playback-runtime/src/native_runner/config_dto.rs`; `config.rs`; `config_schema.rs`; `apply_payload_*.rs`. **Value:** Makes the remaining saved configuration contracts explicit. **Risk:** Migration and unknown-field compatibility can break user data. **Sequence:** After functional migration/round-trip fixtures and the outer/application slice.
3. **Make NativeRunner configuration transactions explicit.** **Action:** Extract one transaction-owned aggregate and emit one classified audio update plan per committed configuration change. **Evidence:** `crates/playback-runtime/src/native_runner.rs`; `apply_payload.rs`; configuration transaction tests. **Value:** Prevents partial live-state updates and duplicate audio revisions. **Risk:** High live-state risk around audio, persistence, and menu dispatch. **Sequence:** After item 2; preserve the current behavior with transaction fixtures.
4. **Improve domain cohesion inside NativeRunner.** **Action:** Extract only bounded domain modules from `native_runner.rs` and sibling impl modules, preserving message ordering and ownership boundaries. **Evidence:** `crates/playback-runtime/src/native_runner.rs`; `crates/playback-runtime/src/native_runner/`. **Value:** Makes future runtime changes easier to review. **Risk:** Broad extraction can hide state coupling or alter dispatch order. **Sequence:** Incremental slices after DTOs and transactions.
5. **Replace regex quality measurements with syntax-aware measurements.** **Action:** Change `scanFunctions` to use a syntax-aware parser and validate it with representative fixtures. **Evidence:** `tools/quality/quality-audit.mjs::scanFunctions`. **Value:** Makes quality warnings trustworthy without changing product behavior. **Risk:** Parser coverage or metric drift can create noisy audit results. **Sequence:** Fixture validation first; then replace the scanner.
6. **Run direct rodio-engine-source tests and Clippy in CI.** **Action:** Add the crate's queue, telemetry, retirement, and direct-render checks to the CI lane. **Evidence:** `ci.yml`; `crates/rodio-engine-source/src/queue_tests.rs`; `telemetry.rs`; retirement tests. **Value:** Catches realtime-source regressions earlier. **Risk:** Linux ALSA dependencies may affect CI setup, but tests need no audio device. **Sequence:** Establish host-only CI coverage before any device smoke requirement.
7. **Add a profile-aware hardware diagnostic harness.** **Action:** Extend the fixed-board diagnostic entry point with board descriptors and bounded component checks. **Evidence:** `apps/pi-zero/src/hardware_test.rs`; `docs/board-profiles.md` board descriptors. **Value:** Makes manual board qualification repeatable. **Risk:** Hardware-dependent and easy to over-generalize. **Sequence:** After the fake transport and before the manual run.
8. **Retire legacy Cargo feature aliases.** **Action:** Inventory aliases, migrate supported profiles and CI, then remove only unused compatibility names. **Evidence:** `docs/board-profiles.md`; workspace/CI feature declarations. **Value:** Reduces build-profile ambiguity. **Risk:** A hidden deployment or cross-build can lose its feature. **Sequence:** Inventory first; remove after profile checks.
9. **Contain and reassess vendored CPAL.** **Action:** Document the exact reason and boundary for the vendored CPAL copy, then reassess removal only after PCM qualification. **Evidence:** `Cargo.toml`; `third_party/cpal-0.15.3/PROVENANCE.md`. **Value:** Keeps the audio dependency intentional and maintainable. **Risk:** Removing it before qualification could change exact PCM behavior. **Sequence:** Never remove before exact PCM qualification.
10. **Retire trusted-parent respin machinery after constructor-image qualification.** **Action:** Remove or narrow the respin workflow only when constructor image derivations and recovery evidence are qualified. **Evidence:** `tools/image-respin`; `resources/image-construction/`; respin workflow. **Value:** Reduces maintenance after the image contract is proven. **Risk:** Premature removal could lose recovery or reproducibility. **Sequence:** Preserve recovery until constructor-image qualification is complete.
