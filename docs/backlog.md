# Backlog — Cell Symphony

> Central requirement backlog. Status: `open` | `in-progress` | `closed`.
> Each phase must be completed and manually tested before moving to the next.

---

## Phase 1: Quality & Consistency

*Foundation — fix issues, standardize, polish. No new features.*

---

### REQ-09 — Duck Self-Reference Prevention

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 1 |
| **Priority** | high |
| **Scope** | tiny |
| **Depends on** | — |
| **Source** | line 60 |

Prevent selecting the current FX bus as its own duck source to avoid cyclic routing.

**Decision:** Block new cyclic assignments only. Do not auto-clear existing cyclic configs on load — old configs that happen to be cyclic simply won't route (no audio feedback). No broader routi[...]

**Acceptance:**
- When editing duck source on an FX bus, the bus itself is not listed as an option (or selecting it is rejected).
- Existing cyclic configs remain in data but produce no routing / no audio feedback loop.

**Implementation:** `fxBusMenu.ts:duckSourceOptions(busIdx)` filters out `B${busIdx+1}` from bus options. Call site passes `busIdx`.

---

### REQ-12 — "none" Options + Naming Standardization

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 1 |
| **Priority** | high |
| **Scope** | small |
| **Depends on** | — |
| **Source** | lines 80–81, 85 |

Add "none" instrument type. Add "none" for all no-op selectable elements where non-destructive. Standardize naming: use "none" everywhere, never "no scan" or similar variants.

**"none" instrument semantics:** Consumes a slot, has configurable FX bus routing, produces silence. Visible in lists, appears as a routing target, but generates no audio.

**Acceptance:**
- Instrument type `"none"` exists — silent placeholder, consumes slot, has FX bus config.
- Every enum/dropdown with a no-op uses label `"none"` (not `"no scan"`, `"off"`, `"disabled"` etc.).
- Existing `"no scan"` (and similar) values are migrated to `"none"`.

**Implementation:**
- TS: `platformTypes.ts`, `menuTree.ts`, `storeRuntime.ts`, `coreUtils.ts` updated to handle `"none"` type.
- Rust: `engine.rs` — added `InstrumentKind::None`, `parse_instrument_kind("none")`, early returns in `note_on`/`note_off`/`cc`.
- Display: scan mode `"no scan"` → `"none"` in `coreUtils.ts:formatDisplayValue`, help TSV, lint alias, generated help, `menu-and-controls-spec.md`.
- Help text: instrument type enum help updated with "none" description.

---

### REQ-16 — Parameter Range Standardization

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 1 |
| **Priority** | medium |
| **Scope** | medium |
| **Depends on** | — |
| **Source** | line 123 |

Replace unusable raw technical values with reasonable editor ranges. Remap display values only — engine keeps internal units. Old saves remain compatible.

**Strategy:**
- **Unit in the label** for parameters where the unit is meaningful: delay times, envelope attack/decay/release, note length, compressor/duck attack/release, BPM, dB thresholds/gains, screen sleep[...]
- **Abstract 0–255** for parameters where the unit is not meaningful to the user: filter cutoff, resonance, envelope amounts, key tracking, all EQ params (gains mapped to ±12 dB, mid freq mappe[...]

**Acceptance:**
- Filter cutoff uses 0–255 range in menu/encoder; translated to Hz internally.
- All numeric parameters reviewed for usability — no values that require hundreds of encoder clicks to traverse.
- Hard-unit parameters (ms, dB, BPM) keep the unit in the label, value is a plain number.
- Abstract/dimensionless parameters migrated to 0–255.
- Encoder acceleration + shift+turn for coarse adjust across all numeric params.

**Implementation (Phase 1 pass):**
- Filter cutoff: menu range changed to 0–255 (logarithmic Hz mapping: 80–16000 Hz). `coreUtils.ts`: added `cutoffDisplayToHz`/`cutoffHzToDisplay`. `coalescedAudioConfig.ts`: auto-converts 0-25[...]
- Filter resonance: menu range changed 0–100 → 0–255.
- `cutoffDisplayToHz` exported from platform-core package.

---

### REQ-11 — Quality Pass

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 1 |
| **Priority** | high |
| **Scope** | medium-large |
| **Depends on** | — |
| **Source** | lines 68–76 |

Code quality improvement pass across the codebase. Both targeted hotspot cleanup and broad sweep.

**Targeted hotspots (priority order):**

| # | File | Issues |
|---|------|--------|
| 1 | `storeRuntime.ts` | `sanitizePayload` 231 lines / 124 `as any`; `applyStoreResult` 14-branch pattern match |
| 2 | `inputRouter.ts` | `routeInputWithDeps` 247 lines; 3 functions duplicated with `transportRuntime.ts` |
| 3 | `stateHelpers.ts` | `applyAutoName` 9 manual blocks → data-driven loop; `writeAnyValue` duplicate normalizations |
| 4 | `simulatorRuntime.ts` | 531-line factory closure; 15-branch `execEffect`; magic MIDI hex bytes |
| 5 | `transportRuntime.ts` | Dedupe with inputRouter; `toRuntimeConfigForPart` ternary nesting |
| 6 | `menuTree.ts` | 200+ lines structural duplication in synth/sample instrument trees |
| 7 | `fxBusMenu.ts` | 15 repeated effect-type blocks → data-driven from effect→params map |
| 8 | `coreUtils.ts` | `formatDisplayValue` 18-branch if/else → `Map` lookup |
| 9 | `App.tsx` | Inline magic numbers; NeoKey event handler dedup |
| 10 | `initialState.ts` | ~20 magic numbers → named constants shared with storeRuntime/musicTransforms |

**Cross-cutting deduplication targets:**
- Velocity defaults (120/85/45) duplicated across `initialState.ts`, `storeRuntime.ts`, `musicTransforms.ts`
- `noteBehavior`/`profileFromConfig`/`withScaleSteps` duplicated across `inputRouter.ts` and `transportRuntime.ts`
- 5-way mapping trigger pattern (activate/stable/deactivate/scanned/scanned_empty) duplicated across 3 files

**Acceptance:**
- Magic numbers/strings extracted to named constants (local `const` minimum; parameters into config/capabilities).
- High-complexity functions (cyclomatic, LOC) refactored or broken down.
- Duplicate logic eliminated; repeated operational patterns centralized behind helpers.
- Uncritical "close enough" parameter values normalised to helper defaults unless semantically meaningful.
- Code smells addressed (long parameter lists, mutable shared state, etc.).
- Pragmatic design patterns applied where they reduce complexity.

**Implementation:**
- `storeRuntime.ts`: `sanitizeInstruments`/`sanitizeMixer` extracted as module-level functions; mapping 5-way → `overrideFromPart`/`preferMapping` helpers.
- `inputRouter.ts`: `inputNoteBehavior` delegates to shared `applyNoteBehavior`; `inputScaleSteps` → shared `withScaleSteps`.
- `stateHelpers.ts`: `applyAutoName` 10 manual branches → data-driven rule tables; `syncLegacy`/`syncActive` use `overrideFromPart`/`preferMapping`.
- `transportRuntime.ts`: `toRuntimeConfigForPart` mapping 5-line ternary → single `mergeMapping` call; old `withScaleSteps`/`applyNoteBehavior` removed.
- `coreUtils.ts`: `formatDisplayValue` 18-branch if/else chain → `FORMAT_MAP` lookup table (regex+string entries); mapping helpers added.
- `initialState.ts`: ~20 magic numbers → `runtimeDefaults.ts` constants.
- `runtimeDefaults.ts`: extracted velocity defaults, BPM, note length, brightness, master volume, screen sleep, pitch defaults, velocity levels, MIDI engine defaults.
- `musicTransforms.ts`: `applyNoteBehavior`/`withScaleSteps` extracted, shared by `inputRouter.ts` and `transportRuntime.ts`.
- `storeRuntime.ts`: `BUS_EFFECT_TYPES` extracted to module-level `Set` constant.
- All 129 tests and 13 desktop tests pass. Lint passes (menu-help + quality checks).

---

## Phase 2: Menu, Navigation & Defaults

*Structural improvements to how users find and manage things.*

---

### REQ-13 — Menu Cleanup

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 2 |
| **Priority** | high |
| **Scope** | medium |
| **Depends on** | REQ-11 (clean base to work from) |
| **Source** | lines 84–88 |

Consistent, well-ordered menus.

**Acceptance:**
- Important parameters first (e.g. Mixer above MIDI in instrument menu). Research typical synth/audio UI ordering and apply.
- Part selection is list-based (same pattern as instruments and FX buses) — each part shown as a named entry in a submenu. Selecting one in the menu selects that part and enters that part's lif[...]
- Bus names are displayed in the bus selection list (not just "Bus 1"–"Bus 4").
- Name setting lives inside the bus config, not outside it.

**Implementation (REQ-13c — list-based part selection):**
- `menuTree.ts`: Removed `l1PartNodes()` flat enum; replaced with `l1PartGroup()`/`l2PartGroup()` returning group nodes per part with label `P${idx+1}: ${name}`.
- `menuInput.ts`: `pressMenuInput` now calls `deps.writeAnyValue("activePartIndex", idx)` + `deps.reinitBehaviorState` to select the active part when entering a part group.
- `index.ts`: `pressMenu` passes `writeAnyValue` and `reinitBehaviorState` to `pressMenuInput`.
- Bus names now displayed as `B${idx+1}: ${name}` via `fxBusLabel()` in `fxBusMenu.ts`.
- Name setting lives inside bus config.
- Tests updated to navigate through part groups.

**Implementation (REQ-13 — parameter ordering):**
- Behavior registration order: `none → life → sequencer → keys → brain → ant → bounce → shapes → raindrops → dla → glider`
- L1 part group: Behavior → Step Rate → (behavior config) → Save Grid State → Auto Name → Part Name
- L2 part group: Note Mapping moved before X/Y Axis. Note Mapping internal: Lowest→Highest→Starting→Scale→Root→Out of Range
- Instrument children: Type → Note Behavior → engine → Mixer → Clone/Reset → MIDI → Auto Name → Name
- Synth internal: Oscillator → Filter (flattened) → Volume; Osc params: Wave→Octave→Level→Detune→PW
- Sample: Filter group before Volume
- Audio+Sound merged into Sound; system groups: Presets → Sound → MIDI → UI Settings
- FX bus config: Slot 1 → Slot 2 → Pan Pos → Auto Name → Name
- FX_SLOT_TYPES reordered by category: reverb/delay→time, tremolo→modulation, eq→compressor→dynamics, saturator→drive, bitcrusher→glitch
- Docs updated (`menu-and-controls-spec.md`, `menu-help-texts.tsv`)

---

### REQ-01 — Clone/Reset Part

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 2 |
| **Priority** | medium |
| **Scope** | small |
| **Depends on** | — |
| **Source** | line 2 |

Clone a part (duplicate its behaviour, mapping, triggers) or delete/reset it, via grid interaction. All 8 (or max) part slots always exist.

**Interaction:**
- **Clone:** FN+SHIFT+rightmost column of source part → press target part's left column. Target part receives a copy of source's full config (behaviour, grid, mapping, triggers).
- **Delete/Reset:** FN+SHIFT+BACK (back button) on the selected part → sets it to no-op defaults (grid cleared, behaviour "none", sense "none", no mappings).

**Acceptance:**
- Clone duplicates all part config: behaviour type + params, grid cells, trigger mappings, scan settings.
- Delete/Reset sets part to complete no-op defaults (no grid state, behaviour "none", no triggers).
- All parts remain in the parts list; no slot creation or removal.

**Implementation:**
- `inputRouter.ts`: Added `pendingCloneSource` to `SystemState`. FN+SHIFT+grid_press at x=7 stores source part index with toast. FN+grid_press at x=0 with pendingCloneSource copies part config/st[...]
- `index.ts`: `pendingCloneSource` added to `SystemState` type and initial state.

---

### REQ-02 — Clone/Reset Instrument

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 2 |
| **Priority** | medium |
| **Scope** | small |
| **Depends on** | REQ-13 (menu structure for the action) |
| **Source** | line 3 |

Clone an instrument (duplicate its type, preset, FX routing) or reset it to defaults. Menu-based action inside the instrument's config menu.

**Acceptance:**
- Instrument config menu has "Clone" action — creates a new instrument with identical type, preset, FX bus assignment, and aux send settings.
- Instrument config menu has "Reset" action — restores instrument to factory defaults (type "none", no routing).
- "Clone" appends to the instruments list (uses the first available "none" slot, or adds if all full).

**Implementation:**
- `menuTree.ts`: Clone/Reset actions added to each instrument group (lines 336-337).
- `actions.ts`: `"instrument_clone"` handler deep-clones source into first "none" slot; `"instrument_reset"` handler sets type "none" with defaults.

---

### REQ-14 — Factory Reset Defaults

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 2 |
| **Priority** | medium |
| **Scope** | small |
| **Depends on** | REQ-12 ("none" instrument type), REQ-11 (clean base) |
| **Source** | lines 92–106 |

On factory reset: 2 active parts, rest "none".

**Acceptance:**
- P1: behaviour=life, auto-spawn=12, activate→I1 (synth with nice preset), routed→FX Bus 1 (delay + duck sourcing I2).
- P2: behaviour=sequencer with cells from current default preset, horizontal scan, scanned→I2 (drum kit from current default preset), routed direct.
- All other parts (P3–P8): behaviour "none", no triggers.
- All other instruments (I3–I8): type "none".
- All other FX buses: no effects, no duck.

**P2/I2 details:** Use current default preset content for sequencer cell pattern, sample assignments, and drum mappings.

**Implementation:**
- `stateHelpers.ts:factoryPayload()`: Modified to produce a ConfigPayload matching REQ-14 spec. Overrides parts/instruments/mixer after initial state creation.
- P1: life with randomCellsPerTick=12, activate→I1, routed→fx_bus_1 (delay 280ms + duck sourcing I2).
- P2: sequencer with horizontal scan (rows), scanned→I2, routed direct.
- P3–P8: behaviour "none", no triggers.
- I1: synth with "soft pad" preset (SYNTH_PRESETS[1]).
- I2: synth with "perc hit" preset (SYNTH_PRESETS[7]) as drum kit, routed direct.
- I3–I8: type "none".
- FX Bus 1: slot1=delay (timeMs:280, feedback:38%, mix:45%), slot2=duck (source:I2). All other buses: no effects.

---

### REQ-10 — Audio Load Indicator

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 2 |
| **Priority** | low |
| **Scope** | small |
| **Depends on** | IPC infra: Rust→TS `"audio_load"` event channel (built in this phase before the indicator) |
| **Source** | line 64 |

Audio DSP load / voice-steal indicator in top-right corner of OLED display.

**Infrastructure needed (do first):** No existing load reporting IPC. Must add:
- Back-channel from `rodio-engine-source` (where `smoothed_load_ratio` is computed) → Tauri audio thread → `app.emit("audio_load", { ratio, voiceSteal })`.
- Follow existing `"midi_in"` event pattern.

**Acceptance:**
- Yellow indicator when DSP load moderate or voice stealing active.
- Red indicator when load heavy.
- Nothing displayed when idle.

**Implementation:**
- `realtime-engine`: `SynthEngine::audio_load_status()` reports smoothed DSP load and clears a recent voice-steal flag. Voice stealing is flagged when synth/sample voices are reused or global voi[...]
- `rodio-engine-source`: `EngineSource::with_load_status_tx()` emits throttled load status updates at ~10 Hz after audio block refills.
- `apps/desktop/src-tauri`: audio thread forwards status over an `audio_load` Tauri event with `{ ratio, voiceSteal }`, following the existing MIDI event pattern.
- `apps/desktop`: runtime listens for `audio_load`, stores the latest status in snapshots, and passes it into platform-core frame rendering.
- `platform-core`: OLED renderer draws a top-right indicator. Hidden below 0.60 load when no voice steal occurred, yellow at 0.60+ or recent voice steal, red at 0.85+.

---

## Phase 3: Grid & Scan

*Enhancements to grid display and scanning engine.*

---

### REQ-03 — Ghost Cells

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 3 |
| **Priority** | medium |
| **Scope** | small-medium |
| **Depends on** | — |
| **Source** | line 7 |

Show dimmed/ghosted cells from inactive parts on the grid, toggleable.

**Acceptance:**
- When viewing a part, cells active on other parts appear at drastically reduced brightness.
- Toggle in runtime config to enable/disable ghost cells.
- Off by default (clarity first).

**Implementation:**
- Added `runtimeConfig.ghostCells`, default `false`, with `System > UI Settings > Ghost Cells` toggle.
- `toSimulatorFrame()` collects inactive part cells from each part's behavior state and passes a ghost overlay to LED rendering.
- Active part cells, scan cursor, Fn indicators, and sample assignment overlays retain priority over ghost cells.
- Tests cover default-off behavior and active-cell override.

---

### REQ-04 — Section Scan Mode

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 3 |
| **Priority** | medium |
| **Scope** | medium |
| **Depends on** | — |
| **Source** | lines 11–16 |

Add `Sections` parameter (1, 2, 4, 8) to scan behaviour. `Sections=1` preserves current scan behavior; higher values split the perpendicular axis into lanes and scan each lane in sequence. When l[...]

Also add per-section note mapping: restart note mapping from "First note" after each section boundary (configurable per axis), enabling longer melodies with limited note range.

**Worked example:** 8×8 grid, horizontal/row scan, sections=2 → 2 lanes of 4×8. Lane 1 scans rows 0–3 left-to-right (8 steps), then lane 2 scans rows 4–7 left-to-right (8 steps). Total: 1[...]

**Acceptance:**
- Scan behaviour has Sections parameter (powers of 2, default 1 = current behaviour).
- Scanning progresses through the configured number of lanes, covering all cells sequentially.
- Stop resets scan to origin.
- Note mapping can restart per section (configurable toggle per axis).

**Implementation:**
- Added per-part `l2.scanSections` enum (`1`, `2`, `4`, `8`), default `1`, exposed as `L2: Sense > Part > Sections` when scanning.
- Extended interpretation scan strategies with optional `sections`, preserving current full-row/full-column behavior at `1`.
- Sectioned row scans sweep horizontal lanes; sectioned column scans sweep vertical lanes. Runtime scan index span expands to `gridWidth * sections` or `gridHeight * sections`.
- Existing stop/emergency reset paths reset sectioned scan to origin via `partScanIndex = 0`.
- Added per-axis `Restart Section` toggles under Pitch Steps. X restart applies to column sections; Y restart applies to row sections.
- Tests cover section lane interpretation, wrapping/reverse scan index behavior, and local pitch restart.

---

## Phase 4: Performance & Effects (L4)

*New L4: Touch/Performance layer, grid-triggered effects, master FX, aux integration, signal viz.*

---

### REQ-07 — L4: Touch / Performance Layer

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 4 |
| **Priority** | high |
| **Scope** | medium |
| **Depends on** | REQ-13 (menu hooks for new layer) |
| **Source** | lines 36–39 |

New L4 layer: "Touch" / "Performance". Contains grid-mode pages (switched via Fn+rightmost column) for mixing, panning, and momentary effects, plus BPM as a regular menu parameter.

**Pages (grid modes):**
- **Mix (volume/mute):** Each row/column = an instrument. Y axis = volume (bottom row = mute, top row = max volume). Marker colour = green.
- **Pan:** Each row/column = an instrument. X axis = pan position (left→right), shown with a two-cell marker.
- **Momentary FX:** Grid cells trigger effects per REQ-06.

**Navigation:** FN+rightmost column selects Touch pages (rows = pages). FN+leftmost column selects a part and exits Touch.

**Aux encoders:** User-mapped assignments stay active across all Touch pages.

**BPM:** Regular menu parameter in the Touch section (not a grid mode). Assignable to encoders like any other parameter.

**Acceptance:**
- L4 appears in layer navigation labelled "Touch".
- FN+rightmost column switches pages: Mix, Pan, FX.
- Mix page: volume grid mode with mute at bottom and green markers.
- Pan page: pan position grid mode with two-cell markers.
- BPM parameter exists in Touch menu section.
- FN+rightmost column jumps to Touch from any layer.
- Aux encoder mappings work consistently across all Touch pages.

**Implemented:** `L4: Touch` menu with Touch Page and BPM, Fn+rightmost page selection, Fn+leftmost part selection/Touch exit, Mix volume/mute grid, Pan grid, FX grid rendering, and Touch LED ove[...]

---

### REQ-06 — Grid-Triggered Momentary Effects

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 4 |
| **Priority** | high |
| **Scope** | large |
| **Depends on** | REQ-07 (L4 structure with FX page) |
| **Source** | lines 26–35 |

Assign momentary effects to grid cells in the Touch FX page. All effects are momentary (active only while held). Effects: stutter, freeze (floating hold with reverb), filter-sweep (closing filter[...]

**Concurrency rules:**
- Max concurrent effects = capability setting (default 4, configurable).
- When all slots full, remaining active cells gray out and do not respond to press until a slot frees.
- Same effect type pressed while already active: new press takes over, old cell "released" even if still held. (You cannot have two stutters active simultaneously, but pressing a second stutter c[...]
- Each effect type has its own identifying colour (palette chosen during design proposal — keep in mind more types may be added later).

**Target:** Each effect is assigned per cell with configurable parameters. This implementation targets global output only and emits platform effects for the later realtime DSP bridge.

**Acceptance:**
- All effects momentary (on while held, off on release).
- Max concurrent effects enforced by capability setting.
- Grayed-out cells unresponsive when all slots exhausted.
- Same-type press replaces existing (old cell released).
- Effect types have distinct identifying grid colours.
- Filter-sweep fades in on press, fades out on release.
- Reuse existing filter/FX code where overlapping.

**Implemented:** `L4: Touch > FX Page` effect selection, per-effect parameter menu, Map to Grid assignment flow, persisted per-cell FX assignments, resolved audio-command emission, max concurrent[...]

---

### REQ-05 — Global FX (Master Bus)

| Field | Value |
|-------|-------|
| **Status** | open |
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

### REQ-08 — Aux Encoder Mapping in Performance

| Field | Value |
|-------|-------|
| **Status** | open |
| **Phase** | 4 |
| **Priority** | medium |
| **Scope** | medium |
| **Depends on** | REQ-07 (Performance area exists) + design proposal (see below) |
| **Source** | lines 42–48 |

Aux encoders auto-map to important numeric values, enum selections, and actions for the current context in Touch area. Show mapping indicators on OLED. Outside Touch area, aux encoders do nothing[...]

**Prerequisite:** Write a design proposal/doc covering:
- How "most important" params are determined per context (menu node metadata? whitelist per node type?)
- OLED indicator rendering: labels per aux slot, position on screen
- How manual user binding interacts with auto-mapping
- Edge cases: nested menus, empty groups, no eligible params
- Mapping persistence: are auto-mappings saved per context, or re-derived each time?

**Acceptance:**
- When entering instrument/effect/behaviour detail in Touch area, aux encoders map to the important tweakable parameters.
- Mapping indicators visible on OLED (labels per aux slot).
- Enables intuitive live tweaking without menu diving.
- Outside Touch area, aux encoders do nothing.
- Proposal reviewed and approved before implementation begins.

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
