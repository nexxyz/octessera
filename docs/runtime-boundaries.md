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
  - applies native core behavior transitions through `platform-core`
  - publishes snapshots, platform effects, audio commands, MIDI events, and runtime status
  - owns MIDI input/output through host adapters only; Tauri/midir and Pi MIDI device access stay outside canonical runtime crates

- Core logic layer (`crates/platform-core`)
  - deterministic behavior execution, grid state, interpretation, mapping, transforms, and native part engine logic
  - generated platform capability constants from `resources/platform-capabilities.json`
  - no UI framework code
  - no platform-specific I/O
  - no desktop, Pi, Tauri, Node runner, storage, MIDI-device, filesystem, or hardware adapter code

- Output adapters (`apps/desktop/src-tauri/src/`)
  - desktop audio sink maps native events/audio commands to the realtime engine and rodio source
  - MIDI input/output uses Tauri-side midir adapters
  - storage, sample-browser filesystem access, and sample decoding are host adapter responsibilities

- Realtime audio engine (`crates/realtime-engine`, `crates/rodio-engine-source`)
  - owns all internal musical audio rendering, instrument route/pan, FX bus sends, FX bus processing, sidechain ducking, and final stereo mix
  - generates synth slot/sample/pan constants from `resources/platform-capabilities.json`
  - receives platform-decoded sample buffers and control events; it does not perform file I/O or sample decoding in the audio callback
  - is the only path for synth/sample instrument audio before device output

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
3. Rust runtime advances transport pulses -> native behavior/menu processing
4. Runtime publishes snapshot -> UI render (OLED + NeoKey LEDs)
5. Runtime publishes musical events/audio commands/platform effects -> host adapters (audio/MIDI/storage)

## Shared Runtime Contract

- The shared Pi/desktop playback seam is defined by `crates/playback-runtime/src/protocol.rs` and mirrored by the UI/device contract types where needed.
- Host -> runner messages are limited to `device_input`, `transport_pulse_step`, split MIDI realtime wire messages (`midi_realtime_clock`, `midi_realtime_start`, `midi_realtime_continue`, `midi_realtime_stop`), and `runtime_result`.
- Runner -> host messages are limited to `snapshot`, `platform_effects`, `musical_events`, `midi_events`, `audio_commands`, `ui_pulse`, and `runtime_status`.
- Shared fixtures for this seam live in `SHARED_RUNTIME_CONTRACT_FIXTURES` so both hosts can validate the same contract examples.
- `transport_pulse_step` is the deterministic PPQN advancement boundary; hosts must not substitute wall-clock timer semantics above this seam.
- External MIDI realtime (`clock`, `start`, `continue`, `stop`) remains explicit at the boundary and is not inferred from UI/runtime scheduling code. Desktop MIDI input is routed natively from the host adapter into the runtime worker; UI code must not observe raw MIDI bytes for display or transport state.
- `runtime_result` carries host-side outcomes for storage, MIDI port enumeration/selection, and sample-browser operations back into the shared runner.
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
