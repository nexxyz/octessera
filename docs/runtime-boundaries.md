# Runtime Boundaries

This contributor contract keeps UI, host adapters, native runtime logic, core behavior logic, and audio rendering in separate layers.

Authoritative menu/control behavior spec: `docs/menu-and-controls-spec.md`.

## Layer Responsibilities

- UI layer (`apps/desktop/src/`)
  - renders runtime snapshot data
  - captures user interaction and emits `DeviceInput`
  - contains no transport/menu/audio/interpretation logic

- Runtime orchestration layer (`crates/playback-runtime`, `apps/desktop/src-tauri/src/runtime_worker.rs`)
  - owns lifecycle (`start`/`stop`)
  - schedules transport pulses and realtime status through Rust runtime code
  - owns native menu state, config payloads, snapshots, platform effects, and `NativeRunner`
  - owns schema-v2 global modulation state, legacy migration, keyed Link LFO menu/binding paths, canonical global Play XY serialization, and transient global-LFO endpoint composition; it emits resolved audio commands while the realtime engine owns rendering
  - `PlaybackRuntime::dispatch` is the canonical host-message/result observation path; desktop and Pi loops schedule work and render its presented output
  - consumes typed runtime-config changes published by `NativeRunner` during canonical dispatch; hosts must not derive playback scheduling config from snapshots
  - maps typed adapter failure facts to recovery policy and owns the best-effort stop-and-silence operation
  - requires every host adapter to implement internal-audio silence and external-MIDI panic explicitly; safety cannot fall through to a successful no-op
  - classifies worker emission and persistence faults as retain/retry outcomes instead of safety-stop failures
  - applies native core behavior transitions through `platform-core`
  - publishes snapshots, platform effects, audio commands, MIDI events, runtime status, and native-owned modal frames
  - owns MIDI input/output through host adapters only; Tauri/midir and Pi MIDI device access stay outside canonical runtime crates

- Core logic layer (`crates/platform-core`)
  - deterministic behavior execution, grid state, interpretation, mapping, transforms, and native layer engine logic
  - generated platform capability constants from `resources/platform-capabilities.json`
  - generated display palette constants from `resources/display-palette.json` so runtime, Pi, and desktop adapters share color values without moving UI policy into the core
  - no UI framework code
  - no platform-specific I/O
  - no desktop, Pi, Tauri, Node runner, storage, MIDI-device, filesystem, or hardware adapter code

- Output adapters (`apps/desktop/src-tauri/src/`)
  - desktop audio sink maps native events/audio commands to the realtime engine and rodio source
  - MIDI input/output uses Tauri-side midir adapters
  - storage, sample-browser filesystem access, and sample decoding are host adapter responsibilities
  - Pi device-update effects are executed by the host updater, which owns profile-qualified asset selection, checksum/manifest validation, candidate health guarding, and automatic fallback; `NativeRunner` owns menu/action semantics and confirmation
  - returns typed failure facts and carries runtime request/revision identity through asynchronous platform/audio-prep jobs; it does not choose recovery policy

- Realtime audio engine (`crates/realtime-engine`, `crates/rodio-engine-source`)
  - owns all internal musical audio rendering, instrument route/pan, FX bus sends, FX bus processing, sidechain ducking, and final stereo mix
  - generates synth slot/sample/pan constants from `resources/platform-capabilities.json`
  - receives platform-decoded sample buffers and control events; it does not perform file I/O or sample decoding in the audio callback
  - receives an explicit `AllNotesOff` internal command for clearing synth, sample, and preview voices; internal safety does not use MIDI CC 120/123
  - is the only path for synth/sample instrument audio before device output
  - shared JSON audio configuration normalization and FX shape/type validation live in `realtime-engine`; desktop and Pi retain sample path resolution, file decoding, caching, and host queueing
  - desktop and Pi return the same typed audio-command/config failures, preserve revision identity for full-config preparation, and route `SamplePreview` through the selected realtime instrument path

## Dependency Rules

- UI may import type contracts and render snapshots only.
- UI must not call native core, transport, audio, MIDI, or storage bridges directly.
- Runtime may import native core and output/input adapters.
- Core crates must stay platform-agnostic.
- `crates/platform-core` and `crates/playback-runtime` must not depend on Tauri, HAL, Pi hardware crates, Node runner processes, storage implementations, or host filesystem/sample-browser adapters.
- Platform adapters must not create independent musical audio sinks that bypass the realtime engine mixer.

## Data Flow

1. UI interaction -> `DeviceInput`
2. Runtime receives input -> native `platform-core` transition through `NativeRunner`
3. `NativeRunner` publishes typed runtime-config changes -> `PlaybackRuntime` updates transport/MIDI scheduling state
4. Rust runtime advances transport pulses -> native behavior/menu processing
5. Runtime publishes snapshot -> UI render (OLED + NeoKey LEDs)
6. Runtime publishes musical events/audio commands/platform effects -> host adapters (audio/MIDI/storage)

## Shared Runtime Contract

- The shared Pi/desktop playback seam is defined by `crates/playback-runtime/src/protocol.rs` and mirrored by the UI/device contract types where needed.
- Host -> runner messages are limited to `device_input`, `transport_pulse_step`, split MIDI realtime wire messages (`midi_realtime_clock`, `midi_realtime_start`, `midi_realtime_continue`, `midi_realtime_stop`), and `runtime_result`.
- Runner -> host messages are limited to `snapshot`, `platform_effects`, `musical_events`, `midi_events`, `audio_commands`, `ui_pulse`, and `runtime_status`.
- Shared fixtures for this seam live in `SHARED_RUNTIME_CONTRACT_FIXTURES` so both hosts can validate the same contract examples.
- `transport_pulse_step` is the deterministic PPQN advancement boundary; hosts must not substitute wall-clock timer semantics above this seam.
- External MIDI realtime (`clock`, `start`, `continue`, `stop`) remains explicit at the boundary and is not inferred from UI/runtime scheduling code. Desktop MIDI input is routed natively from the host adapter into the runtime worker; UI code must not observe raw MIDI bytes for display or transport state.
- `runtime_result` carries host-side outcomes for storage, MIDI port enumeration/selection, sample-browser operations, device-update status, and asynchronously identified sanitized system-info requests back into the shared runner.
- `SystemInfoRequest` is a typed platform request; adapters return typed `SystemInfoResult` or identified `SystemInfoError` values. The native runner owns loading/error/unavailable presentation, row formatting, clipping, scrolling, and dismissal. Desktop UI renders only the resulting snapshot/OLED frame.
- `NativeRunner` may emit an internal typed runtime-config change during dispatch; `PlaybackRuntime` consumes it before returning presented host messages. It is not a host adapter responsibility and is never reconstructed from snapshot fields.
- Central modulation processing is the single native path for held tick/XY sources and global LFO output: behavior ticks and active XY captures update held sources, global LFOs advance only at 24 PPQN, and the process sums persistent, held, and LFO contributions, clamps once, and applies once per target endpoint. An active LFO step processes only dirty LFO endpoints plus other contributors sharing those keys; unrelated held grid/XY sources are retained without being resolved, reapplied, or cloned. Ordinary menu/Aux base edits rebase and recompose only the edited key/endpoint, preserving held sources so clearing a source restores the new base; changed persistent targets from one tick share one revision and delayed autosave request. Config/patch transactions reset candidate modulation state, install every persistent owner, then resample active XY so its captured base is the loaded owner value. Enabled targeted LFO phases advance and wrap even at depth zero; no other PPQN or wall-clock path may advance an LFO or reapply a held source.
- Background audio preparation returns identified typed success/failure results through `runtime_result`; prep failures retain the last good runtime/audio state.
- Sample-bank preparation is atomic on both hosts: every configured sample path must resolve and decode before a new bank is queued; unresolved or undecodable samples return typed `sample` failures and leave the previous banks and signatures in place. Sample preview resolution/decoding runs on the audio-prep worker, never on the runtime or host-adapter thread, and reports success only after the prepared preview reaches the audio queue.
- Pi audio preparation treats superseded revisions as cancellation: no stale-prep fault is returned or latched.
- Identified asynchronous results retain their request ID/revision through the runner round trip; `PlaybackRuntime` observes each result once and clears only the matching fault.
- Emission and persistence faults clear only after the corresponding native emission or identified save/recovery acknowledgement succeeds. Native save confirmation/toast feedback is emitted after that acknowledgement, not when a save request is queued.
- Stop-and-silence recovery independently attempts runner transport stop, internal synth/sample silence, and external MIDI panic on both hosts.
- `snapshot` is the runtime display/input-facing state payload; `musical_events`, `midi_events`, `platform_effects`, and `audio_commands` are the resolved outputs that Rust schedules or dispatches.

## Audio Routing Contract

- Internal synth and sample instruments must enter the realtime engine before audio output.
- Instrument `Route=direct` bypasses FX bus processing and pans directly into the main mix.
- Instrument `Route=fx_bus_n` enters the selected FX bus, runs its slot FX in order, then pans into the main mix.
- MIDI instruments emit external MIDI/control data and are not an internal audio source unless an audio return path is explicitly added.
- MIDI-only instrument notes and CCs use the `midi_events` path and must not call host internal-audio musical event handling.
- Sample browser preview is musical audio and must route through the selected instrument slot, pan, volume, FX bus, and master output path.
- Runtime audio config commands carry `sound.voiceStealingMode`; host adapters forward it to the realtime audio policy.
- `gridBrightness` is applied by core LED frame rendering; `displayBrightness` and `buttonBrightness` are applied by the host display/button LED adapters.

## Grid Coordinate Contract

- Core logic uses a world-space grid origin at lower-left: `(0,0)` is bottom-left, `y` increases upward.
- UI/hardware-facing layers may use screen-space coordinates (top-left origin), but conversion is only allowed at boundaries.
- In code, grid coordinate conversion must go through the centralized grid domain helpers (`gridDomain.ts`) rather than ad-hoc math.
