# Runtime Boundaries

This project keeps UI/hardware emulation separate from logic and processing.

Authoritative menu/control behavior spec: `docs/menu-and-controls-spec.md`.

## Layer Responsibilities

- UI layer (`apps/desktop/src/`)
  - renders simulator snapshot data
  - captures user interaction and emits `DeviceInput`
  - contains no transport/menu/audio/interpretation logic

- Runtime orchestration layer (`apps/desktop/src/runtime`)
  - owns lifecycle (`start`/`stop`)
  - schedules ticks through `runtimeScheduler`
  - applies core state transitions (`routeInput`, `tick`)
  - publishes snapshots and musical events
  - owns MIDI input/output via Tauri bridges only (no Web MIDI)

- Core logic layer (`packages/platform-core`, `packages/interpretation-core`, `packages/mapping-core`, `packages/behavior-api`, all behavior packages)
  - deterministic simulation, menu/control state, interpretation, mapping
  - no UI framework code
  - no platform-specific I/O

- Output adapters (`apps/desktop/src/runtime/outputAdapters/`)
  - desktop audio sink maps musical events to native Tauri/rodio
  - MIDI output via `tauriMidi.ts` (Tauri→midir)

- Realtime audio engine (`crates/realtime-engine`, `crates/rodio-engine-source`)
  - owns all internal musical audio rendering, instrument route/pan, FX bus sends, FX bus processing, sidechain ducking, and final stereo mix
  - receives platform-decoded sample buffers and control events; it does not perform file I/O or sample decoding in the audio callback
  - is the only path for synth/sample instrument audio before device output

## Dependency Rules

- UI may import runtime modules and type contracts only.
- UI must not call `tick`, `routeInput`, or native audio/MIDI bridges directly.
- Runtime may import core and output/input adapters.
- Core packages must stay platform-agnostic.
- Platform adapters must not create independent musical audio sinks that bypass the realtime engine mixer.

## Data Flow

1. UI interaction -> `DeviceInput`
2. Runtime receives input -> `platform-core` transition
3. Runtime scheduler triggers tick -> `platform-core` processing
4. Runtime publishes snapshot -> UI render (OLED + NeoKey LEDs)
5. Runtime publishes musical events -> output adapters (audio/MIDI)

## Audio Routing Contract

- Internal synth and sample instruments must enter the realtime engine before audio output.
- Instrument `Route=direct` bypasses FX bus processing and pans directly into the main mix.
- Instrument `Route=fx_bus_n` enters the selected FX bus, runs its slot FX in order, then pans into the main mix.
- MIDI instruments emit external MIDI/control data and are not an internal audio source unless a future audio return path is added.
- Sample browser preview is musical audio and must route through the selected instrument slot, pan, volume, FX bus, and master output path.
- Host adapters must forward `sound.voiceStealingMode` to the realtime audio policy.
- `gridBrightness` is applied by core LED frame rendering; `displayBrightness` and `buttonBrightness` are applied by the host display/button LED adapters.

## Grid Coordinate Contract

- Core logic uses a world-space grid origin at lower-left: `(0,0)` is bottom-left, `y` increases upward.
- UI/hardware-facing layers may use screen-space coordinates (top-left origin), but conversion is only allowed at boundaries.
- In code, grid coordinate conversion must go through the centralized grid domain helpers (`gridDomain.ts`) rather than ad-hoc math.
