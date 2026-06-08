# Backlog — Cell Symphony

> Central requirement backlog. Status: `open` | `in-progress` | `closed`.
> Each phase must be completed and manually tested before moving to the next.


### REQ-15 — Signal Path Visualization

| Field | Value |
|-------|-------|
| **Status** | open |
| **Phase** | 4 |
| **Priority** | low |
| **Scope** | medium |
| **Depends on** | stable L2 Sense trigger mappings, L3 instrument mixer routing, FX bus routing, Global FX routing |
| **Source** | lines 109–118 |

OLED graphical display of the active configuration and musical signal path: `L1: Life` behavior -> `L2: Sense` interpretation/mapping -> `L3: Voice` instrument/audio routing -> `L4: Dance` live performance overlays -> master output.

**Configuration flow:**
- `L1: Life` defines the active part behavior, step rate, behavior-specific config, and part identity.
- Behavior output is the upstream source for the visualization: the diagram starts from each active part's selected behavior, not from raw audio.
- `L2: Sense` defines how behavior/grid state is interpreted into trigger intents, including scanning, event triggers, trigger probability, note mapping, and X/Y axis modulation.
- `L3: Voice` defines the destination instruments, their type and note behavior, instrument mixer routing, FX buses, and Global FX.
- `L4: Dance` defines live performance controls layered on top of the configured path, including mix, pan, trigger-gate, X/Y modulation, and momentary FX targets inserted into the audio path.
- The visualization must make it clear which parts of the graph come from stored configuration versus live performance overlay state.

**Signal path logic:**
- Parts do not output audio directly. Each active part interprets behavior grid activity through `L2: Sense` into trigger intents.
- Part nodes should identify the active `L1: Life` behavior driving that part.
- Sense mappings route trigger intents into instrument slots by trigger kind:
- `activate`, `stable`, and `deactivate` mappings when `Event Triggers` is enabled.
- `scanned` and `scanned_empty` mappings when `Scan Mode=scanning`.
- `L2: Sense` note mapping and axis modulation affect the musical events sent into the destination instrument slot and should be represented as Sense-stage transformation between part behavior and instrument target.
- Mappings with `Action=none` do not create a visible route.
- Mappings targeting an instrument whose `Type=none` are hidden.
- Synth and sampler instruments are internal audio sources:
- `Route=direct`: instrument post-fader output is panned by instrument `Pan Pos` and summed into the main mix.
- `Route=fx_bus_n`: instrument post-fader output is sent exclusively to FX Bus N.
- FX buses run `Slot 1` then `Slot 2`; slots with `Type=none` are passthrough.
- FX bus output is panned by bus `Pan Pos` and summed into the main mix.
- Global FX runs after direct and bus outputs are summed, in `Slot 1..N` order.
- Global momentary FX is applied after Global FX.
- Master output applies after the full main mix/global FX/momentary FX chain.
- MIDI instruments emit external MIDI/control data and are shown as terminal external-MIDI routes, not as internal audio routes.

**Layout rules:**
- Display is for the 128x128 OLED.
- Show only active routes.
- Use the existing `L1`/`L2`/`L3`/`L4` section color scheme consistently across boxes, highlights, and connector accents.
- `L1: Life` color identifies part/behavior source nodes.
- `L2: Sense` color identifies interpretation, mapping, trigger-probability, note-mapping, and modulation nodes.
- `L3: Voice` color identifies instruments, mixer routing, FX buses, Global FX, and output nodes.
- `L4: Dance` color is white and identifies live overlay nodes such as trigger-gate state, X/Y modulation, mix/pan performance control, and momentary FX targets.
- When a node reflects both stored config and live overlay state, use the owning stage color for the box and the currently active overlay stage color as the highlight/accent.
- Hide parts with no enabled non-`none` Sense mappings.
- Hide instruments with `Type=none`.
- Hide FX buses that have no routed instruments.
- Hide FX slots with `Type=none`, but still show the bus if routed instruments pass through it.
- Always show Global FX only when at least one global slot has `Type != none`; otherwise show the summed main mix flowing directly to output.
- Auto-layout from top to bottom:
- `L1: Life` part/behavior source
- `L2: Sense` interpretation and mapping stage
- Parts
- Instrument slots
- Direct main mix or FX buses
- Global FX, when active
- Momentary FX, when active
- Output
- `L4: Dance` overlay indicators should be shown adjacent to the stage they affect rather than as a separate disconnected graph.
- Use boxes for entities and arrows for signal/event flow.
- When the diagram becomes crowded, abbreviate labels to compact IDs:
- `P1`, `P2`, etc. for parts
- `I1`, `I2`, etc. for instruments
- `B1`, `B2`, etc. for FX buses
- short FX IDs such as `rv`, `dl`, `dk`, `eq`, `cmp`, `sat`, `dst`
- `GFX` for Global FX
- `OUT` for master output
- Prefer hiding inactive entities before abbreviating active entities.
- If the full active graph still cannot fit at readable scale, collapse repeated parallel routes into grouped edges where possible.

**Navigation:**
- The diagram is navigable with the main encoder.
- Encoder turn moves highlight between visible boxes.
- Encoder press enters the highlighted entity's canonical menu:
- Behavior source -> `L1: Life > Pn`
- Part -> `L2: Sense > Pn`
- Sense transform node -> the relevant `L2: Sense > Pn` subgroup
- Instrument -> `L3: Voice > Instruments > Instrument n`
- FX Bus -> `L3: Voice > FX Buses > Bus n`
- FX Bus slot -> that bus slot's FX menu
- Global FX -> `L3: Voice > Global FX`
- Global FX slot -> that global slot's FX menu
- Dance overlay node -> relevant `L4: Dance` submenu or mapped target context
- Output -> relevant master/sound output menu
- Back exits the diagram view to the previous menu context.

**Acceptance:**
- Shows the full configuration flow from `L1: Life` behavior selection through `L2: Sense`, `L3: Voice`, and `L4: Dance` overlays.
- Shows active part-to-instrument event routes from L2 Sense mappings.
- Shows where `L2: Sense` note mapping, trigger probability, scanning/events, and modulation affect the route.
- Shows internal audio routes from synth/sampler instruments through direct output or FX buses.
- Shows FX bus slot order, bus pan/output, Global FX order, momentary FX position, and master output.
- Uses the established `L1`/`L2`/`L3`/`L4` section colors to visually distinguish source, interpretation, voice, and performance-overlay stages.
- MIDI instruments are represented as external MIDI terminal routes, not routed through internal audio FX.
- `none` mappings, `Type=none` instruments, unused buses, and `Type=none` FX slots are hidden.
- Crowded diagrams use compact IDs and grouping while remaining readable.
- Navigation can enter each visible entity's canonical config menu.
- Fits on the 128x128 OLED at readable scale for typical use-cases: 2-4 active parts, 2-4 active instruments, 1-2 active buses, and 0-2 active Global FX slots.

## Phase 5: Advanced / Hardware

*Hardware-specific features and tooling.*

---

### REQ-16 — Rust-Owned Realtime Playback Runtime

| Field | Value |
|-------|-------|
| **Status** | open |
| **Phase** | 5 |
| **Priority** | high |
| **Scope** | large |
| **Depends on** | REQ-06, REQ-05, stable platform-core engine-event boundary |
| **Source** | architecture follow-up |

Migrate realtime execution ownership from the desktop JavaScript runtime toward Rust. `platform-core` remains the canonical control/state machine for menu, grid semantics, behavior transitions, a[...]

This runtime must be designed for hardware parity, not as a desktop-only optimization. Desktop is only a stand-in for the Raspberry Pi-based hardware target, so any realtime playback ownership moved into Rust should be implemented once in a shared runtime layer that both hosts use wherever reasonably possible.

**Target ownership:**
- Rust owns transport clock timing, BPM timing, PPQN/MIDI clock timing, audio callback timing, MIDI output scheduling, and block/sample-accurate engine event dispatch.
- `platform-core` emits resolved engine/audio events and config updates, not backend-specific scheduling instructions.
- Desktop and Pi hosts remain dumb adapters: render simulator frames or hardware displays, collect hardware-like input, and forward platform effects to storage/MIDI/audio backends.

**Shared runtime shape:**
- Prefer a shared Rust realtime runtime crate used by both `apps/desktop` and `apps/pi-zero`, with host-specific code limited to transport adapters, device I/O, and UI/display integration.
- Keep `realtime-engine` focused on DSP/audio primitives unless expanding it is clearly simpler than introducing a separate shared runtime crate.
- Do not create separate desktop and Pi timing implementations unless a concrete hardware or API limitation makes that unavoidable.

**Migration path:**
- Establish generic engine/audio command boundary for resolved platform effects.
- Move momentary FX DSP and command handling into Rust.
- Move MIDI output scheduling from desktop JS into Rust.
- Move transport clock / PPQN tick ownership into Rust while keeping platform-core deterministic and externally stepped.
- Revisit behavior/scan tick scheduling once the Rust clock boundary is stable.
- Route both desktop and Pi through the same Rust realtime runtime boundary before adding host-specific fallback paths.

**Acceptance:**
- Desktop no longer owns realtime MIDI/audio scheduling semantics.
- Rust runtime can run transport/MIDI/audio timing without browser timers.
- Hardware host can reuse the same platform-core state machine and Rust realtime runtime without desktop-specific logic.
- Desktop and Pi share one realtime playback implementation in Rust wherever reasonably possible, with differences isolated to host adapters.

---

### REQ-19 — Migrate Platform Core to Rust

| Field | Value |
|-------|-------|
| **Status** | open |
| **Phase** | 5 |
| **Priority** | high |
| **Scope** | very-large |
| **Depends on** | Phases 1–4 (stable design baseline) |
| **Source** | architecture follow-up |

Migrate `platform-core` and `behavior-api` from TypeScript to Rust. This is the single source of truth for all core logic (cellular automaton, behavior algorithms, state machine, grid semantics, menu tree, synthesis dispatch). The Tauri desktop UI becomes a thin wrapper; the Pi Zero device shares the identical Rust binary with custom PCB I/O and OTA updates via reboot.

**Target architecture:**
- **Rust core:** All behaviors, state machine, menu navigation, grid logic, configuration, serialization (via `serde`).
- **Desktop (Tauri):** TS UI layer, calls Rust core via IPC to render frames, handles input, forwards audio events and MIDI.
- **Hardware (Pi Zero):** Same Rust core binary, custom PCB drivers for LED, buttons, encoders, audio I/O instead of Tauri.
- **Single canonical implementation** everywhere — no divergence between device and desktop.

**Migration steps:**

1. **Scoping & Analysis** (`sub-19-01`): Agents analyze `platform-core` and `behavior-api` packages; document TS semantics, identify Rust equivalents, propose crate structure and module boundaries.

2. **Rust project structure** (`sub-19-02`): Create workspace in `/crates` with sub-crates for core, behaviors, menu, serialization, bridge FFI. Integrate with existing `Cargo.toml`.

3. **Behavior migration** (`sub-19-03`): Translate behavior algorithms (life, sequencer, ant, glider, etc.) to Rust with trait-based composition. Preserve algorithmic correctness via property testing (quickcheck).

4. **State machine & menu** (`sub-19-04`): Rewrite menu tree, state transitions, input routing (from `inputRouter.ts`), and store mutations (from `storeRuntime.ts`) as idiomatic Rust with zero-copy state updates.

5. **Serialization & config** (`sub-19-05`): Implement `serde` schemas for all config types; ensure backward compatibility with existing `.cell` saves. Validate via migration tests.

6. **Desktop bridge** (`sub-19-06`): Create FFI layer; Tauri calls Rust core to process input, request frame renders, receive state snapshots. Desktop remains thin—no business logic.

7. **Testing & validation** (`sub-19-07`): Parity tests (same input produces identical output on TS and Rust), integration tests on both desktop and device, regression test suite for all behaviors.

8. **Optimization** (`sub-19-08`): Profile and optimize hot paths; measure binary size; ensure Pi Zero build is lean (<50 MB including dependencies).

9. **OTA update infrastructure** (`sub-19-09`): Build, sign, and deploy Rust binaries to device; verify checksums; reboot cycle. No hot-reload needed—firmware update model.

10. **Sunset TS core** (`sub-19-10`): Remove old `platform-core` and `behavior-api` TS packages once Rust version is production-ready. Retain Tauri UI bridge only.

**Acceptance:**
- Rust core compiles cleanly for x86-64 (desktop), ARMv6 (Pi Zero), and other targets.
- All behaviors execute identically on desktop simulator and hardware device.
- Existing `.cell` save files load and play back correctly.
- Tauri UI renders identical OLED frames to desktop simulator.
- Device OTA updates deploy and boot correctly.
- No performance regression on Pi Zero audio synthesis (same or better than current).
- TS/JS minimal and read-only (UI only); no business logic in JavaScript.

---

### REQ-17 — Hardware Test Harness

| Field | Value |
|-------|-------|
| **Status** | open |
| **Phase** | 5 |
| **Priority** | medium |
| **Scope** | medium |
| **Depends on** | — |
| **Source** | line 52 |

Tool launched on Raspberry Pi that guides through testing every button, grid element, encoder, and audio output — to verify hardware assembly per PCB design. *(Placeholder — details to be spe[...]

**Acceptance:**
- Step-by-step guided tests: "Press button A1", "Turn encoder 1 clockwise", etc.
- Grid: "Tap each cell", "Verify colour X at Y,Z".
- Audio: play back a test sample through output.
- Reports pass/fail per test.

---

### REQ-18 — Over-the-Air Updates

| Field | Value |
|-------|-------|
| **Status** | open |
| **Phase** | 5 |
| **Priority** | low |
| **Scope** | small |
| **Depends on** | — |
| **Source** | line 56 |

"Update from GitHub" function on hardware — fast, dynamic update to latest firmware/software. *(Placeholder — details to be specified at Phase 5.)*

**Acceptance:**
- Single action triggers check for updates from GitHub.
- Downloads and applies update automatically.
- Rollback on failure.
- Progress indication on OLED.
