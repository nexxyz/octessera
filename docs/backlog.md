# Backlog — Cell Symphony

> Central requirement backlog. Status: `open` | `in-progress` | `closed`.
> Each phase must be completed and manually tested before moving to the next.

---

### REQ-20 - Position-Marker Bar Display Style

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 4 |
| **Priority** | low |
| **Scope** | small |
| **Depends on** | - |
| **Source** | design discussion |

Add a new `displayStyle: "marker"` for numeric parameters that are bipolar or positional (not 0->max filling). Renders a thin horizontal track with a vertical position indicator, replacing the misleading filled bar.

**Marker rendering** (vs current filled bar):
```
Current filled bar:   [////////// ]  (Pan Pos = 20 looks 62% full, misleads)
Marker bar:           [-----|-----]  (same value, actual position shown)
```

**Parameters to mark (by file):**

`menuTree.ts`:
- `*.mixer.panPos` (0-32, center=16)
- `*.synth.osc{1,2}.detuneCents` (-50-50)
- `*.synth.filter.envAmountPct` (-100-100)
- `*.sample.tuneSemis` (-24-24)
- `*.sample.filter.envAmountPct` (-100-100)
- `touchFx.selected.params.semitones` (-24-24)
- `touchFx.selected.params.cents` (-100-100)

`fxBusMenu.ts`:
- `mixer.buses.*.panPos` (0-32)
- `mixer.buses.*.slot{1,2}.params.lowGainDb` (-12-12)
- `mixer.buses.*.slot{1,2}.params.midGainDb` (-12-12)
- `mixer.buses.*.slot{1,2}.params.highGainDb` (-12-12)
- `mixer.buses.*.slot{1,2}.params.feedback` (mod delay, -0.95-0.95)

**Implementation:**
- `platformTypes.ts`: Add `"marker"` to `displayStyle` union on number variant; add optional `style?: "fill" | "marker"` to `BarValue`.
- `menuPresentation.ts`: `shouldUseNumberBar` returns `true` for `"marker"` (same as `"bar"`).
- `menuView.ts`: Propagate `item.displayStyle` into `barValues[i].style` as `"marker"`.
- `oledRender.ts`: When `bar.style === "marker"`, draw 1px track + 2px vertical marker at `frac` position; else use current fill behavior.
- `menuTree.ts`, `fxBusMenu.ts`: Add `displayStyle: "marker"` to ~12 bipolar/positional params listed above.

**Acceptance:**
- Pan Pos, Detune, Env Amt, Tune Semis, Semitones/Cents show a centered marker bar instead of a filled bar on OLED.
- EQ gain dBs and mod delay Feedback show marker bars.
- Volume, Brightness, Mix %, and other 0->max params remain as filled bars (unchanged).
- All 179 tests pass, lint passes, typecheck passes.

---

### REQ-05 — Global FX (Master Bus)

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 4 |
| **Priority** | medium |
| **Scope** | medium |
| **Depends on** | REQ-07 (L3 Voice area as parent) |
| **Source** | lines 19–23 |

Add master/global FX section in L3:Voice parallel to Instruments. Post-instrument/pre-output. Effects: vinyl simulator (warm saturation, crackling, uneven/warped pitch), EQ, compressor.

**Acceptance:**
- Global FX section in L3:Voice, operates post-instrument/pre-output.
- Vinyl simulator: saturation amount, crackle level, warp depth.
- EQ: low/mid/high bands or parametric.
- Compressor: threshold, ratio, attack, release, gain makeup.
- Reuse existing FX infrastructure, refactor where overlapping (no separate filter implementations).

---

### REQ-15 — Signal Path Visualization

| Field | Value |
|-------|-------|
| **Status** | open |
| **Phase** | 4 |
| **Priority** | low |
| **Scope** | medium |
| **Depends on** | REQ-07 + REQ-06 + REQ-05 (routing fully defined) |
| **Source** | lines 109–118 |

OLED graphical display of signal/routing paths: parts → instruments → FX buses → output.

**Layout rules:**
- Always show only active routes — entities with "none" or no mappings are hidden.
- When diagram becomes too crowded for the 128×64 OLED, abbreviate names to compact IDs (e.g. "I1" instead of "I1: Drums", "P1" instead of "P1: Atmosphere", "dk" for duck, "rv" for reverb). No s[...]
- Auto-layout from top to bottom: Parts → Instruments → FX Buses → Output.
- Navigable: highlight a box via encoder, press to enter that entity's menu.

**Acceptance:**
- Shows active part→instrument→FX routing as boxes/arrows on OLED.
- "None" entities hidden.
- Crowded diagrams use abbreviated IDs instead of full names.
- Navigable: highlight and click to enter entity config.
- Fits on OLED at readable scale for typical use-cases (2–4 parts, 2–4 instruments, 1–2 buses).

---

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

**Target ownership:**
- Rust owns transport clock timing, BPM timing, PPQN/MIDI clock timing, audio callback timing, MIDI output scheduling, and block/sample-accurate engine event dispatch.
- `platform-core` emits resolved engine/audio events and config updates, not backend-specific scheduling instructions.
- Desktop remains a dumb host: render simulator frames, collect hardware-like input, and forward platform effects to storage/MIDI/audio backends.

**Migration path:**
- Establish generic engine/audio command boundary for resolved platform effects.
- Move momentary FX DSP and command handling into Rust.
- Move MIDI output scheduling from desktop JS into Rust.
- Move transport clock / PPQN tick ownership into Rust while keeping platform-core deterministic and externally stepped.
- Revisit behavior/scan tick scheduling once the Rust clock boundary is stable.

**Acceptance:**
- Desktop no longer owns realtime MIDI/audio scheduling semantics.
- Rust runtime can run transport/MIDI/audio timing without browser timers.
- Hardware host can reuse the same platform-core state machine and Rust realtime runtime without desktop-specific logic.

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
