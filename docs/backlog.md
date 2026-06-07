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

### REQ-20 — Probabilistic Trigger Gate

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 4 |
| **Priority** | medium |
| **Scope** | medium |
| **Depends on** | existing Dance trigger-gate page, Sense per-part config, preset/config persistence |
| **Source** | design follow-up |

Replace the current binary trigger-gate workflow with a probability-driven trigger gate that separates stored per-cell probability editing from live playback mode switching.

**Design:**
- Keep the existing Dance enum/page identity as `trigger-gate`; it remains a trigger gate, but probability-driven.
- Add per-part trigger probability state in Sense: `triggerProbabilityMode = zero | custom | full`, `triggerProbabilityMap` (64 cells), `low` threshold, and `high` threshold.
- Probability map cells are four-state: `zero`, `low`, `high`, `full`.
- The Sense probability map editor owns cell editing; the Dance page only switches each part's active trigger mode.

**Sense editor:**
- Add a per-part `Trigger Probability` group under `L2: Sense`.
- Menu items: current mode, `Low Prob`, `High Prob`, and `Map Probability Grid`.
- Grid editing cycles cell states `zero -> low -> high -> full -> zero`.
- LED colours in the editor: red = `0%`, yellow-family states for custom values, green = `100%`.
- `Shift + cell` applies to row; `Fn + Shift + cell` applies to column.

**Dance page:**
- Replace the current trigger-gate cell editor behavior with per-part mode selection.
- Rows follow existing Fn part-navigation orientation: bottom row is part 0, top row is highest part.
- Columns `0..2` select the row's part mode: `0%` (red), `custom` (yellow), `100%` (green).
- Columns `3..4` stay dark as a safety gap.
- Bottom-row columns `5..7` are always-bright all-parts actions: set all parts to `0%`, `custom`, or `100%`.
- Individual part mode LEDs are bright when selected and dim when not selected.

**Transport behavior:**
- Runtime trigger filtering uses the selected mode per part:
- `full`: always pass.
- `zero`: always block.
- `custom`: resolve the cell state to `0%`, `low`, `high`, or `100%` and probabilistically pass/block the trigger.

**Fn+Play:**
- `Fn+Play` no longer fills or clears the stored gate grid.
- It toggles the active part between `0%` and its previously active mode.
- Restoring must return to the previous mode without modifying the stored probability map.

**Persistence / migration:**
- Update saved config/state defaults for the new mode/map fields.
- Migrate persisted boolean `triggerGates` to the new probability-map format (`true -> full`, `false -> zero`) and restore migrated parts in `custom` mode.

**Acceptance:**
- Each part has a stored 8x8 probability map and configurable low/high thresholds.
- Sense can edit four-state probability cells with row/column gestures.
- Dance `trigger-gate` page switches per-part trigger mode using the 3-column layout and bottom-row all-parts actions.
- Part LEDs use red/yellow/green with bright selected state and dim unselected state; all-parts buttons remain bright.
- `Fn+Play` toggles the active part between `0%` and its previous trigger mode.
- Existing presets/defaults with boolean `triggerGates` load into the new model without data loss.
- Docs/help text/tests are updated to match the new Trigger Probability workflow.

---

### REQ-21 — Strict Descriptive Menu Help Lint

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 4 |
| **Priority** | medium |
| **Scope** | medium |
| **Depends on** | existing `menu-help` generation/lint flow in `platform-core` |
| **Source** | design follow-up |

Tighten menu-help coverage so every concrete menu node resolves to descriptive help text for its actual functionality, not generic fallback copy.

**Problem:**
- The current menu-help linter allows broad fallback rows such as `default_*`, `action:*`, and `key:*` to count as covered.
- This means many menu entries still pass lint while resolving to generic help like "Adjusts a numeric value" instead of function-specific guidance.
- Strict descriptive help should be enforced across `platform-core` menus, while still keeping runtime fallback behavior as a safety net.

**Stage 1: Remove Broad Catch-Alls**
- Remove the broad TSV fallback rows that currently satisfy lint without providing descriptive functionality-specific help:
- `action_any`
- `number_any`
- `enum_any`
- `bool_any`
- `text_any`
- Keep runtime-safe fallback behavior via `resolveMenuHelp()` and existing non-explicit defaults, but stop allowing broad `action:*` / `key:*` rows to satisfy lint coverage.
- Regenerate `menuHelp.generated.ts` and use the existing linter failures as the source of truth for missing descriptive help.

**Stage 2: Canonicalization**
- Canonicalize repeated dynamic keys/paths in lint reporting so repeated part/instrument targets collapse into one actionable class.
- Normalize representative patterns such as `parts.*`, `instruments.*`, `mixer.buses.*`, and `globalFx.slots.*`.

**Stage 3: Specificity Rules**
- Define broad generic rows as runtime fallback only, not acceptable lint coverage for concrete menu nodes.
- Continue allowing semantic wildcard rows such as `key:parts.*.l2.pitch.lowestNote` or `action:preset_load:*`.
- Add checks for obviously generic copy so placeholder prose cannot satisfy strict mode.

**Stage 4: Fill Core Help**
- Add descriptive TSV help rows for currently generic-covered platform-core menus, starting with:
- `L1: Life` part controls and behavior config.
- `L2: Sense` scanning, events, trigger probability, note mapping, and axis modulation.
- Part naming and auto-name behavior.

**Stage 5: Fill Remaining Help**
- Add descriptive TSV help rows for instruments, mixer, FX, Dance, MIDI, saves, defaults, and other remaining menus still covered by generic fallback.

**Stage 6: Strengthen Enum Coverage**
- Expand enum-option lint beyond the small current allowlist so help text must describe the actual current options for enum settings.
- Permit only narrow documented exceptions for dynamic labels where necessary.

**Stage 7: Make Strict Mode Default**
- After generic fallback usage is driven to zero, make strict descriptive checking the default behavior of `lint:menu-help`.
- Keep an explicit temporary local escape hatch only if necessary for development, not for CI.

**Stage 8: Contributor Guidance**
- Document the policy near the TSV/linter workflow: new menu nodes must add descriptive help in the same change; generic catch-alls are fallback only; enum changes must update help text.

**Acceptance:**
- Removing the broad catch-all rows causes `lint:menu-help` to fail on every concrete menu target that still lacks descriptive help.
- Canonicalized lint output is actionable rather than flooded with duplicate part/instrument instances.
- All concrete `platform-core` menu nodes resolve to descriptive TSV help rows rather than broad `action:*` or `key:*` generic catch-alls.
- Enum help lint fails when current enum options are missing from descriptive help.
- Strict descriptive checking becomes the default lint mode once coverage is complete.
- Contributor workflow/docs make the expectation explicit for future menu additions.

---

### REQ-22 — Sense Mapping Menus

| Field | Value |
|-------|-------|
| **Status** | closed |
| **Phase** | 4 |
| **Priority** | medium |
| **Scope** | medium |
| **Depends on** | existing Dance X/Y parameter picker, existing param-mod state, existing aux binding persistence |
| **Source** | design follow-up |

Add explicit menu-based editing for Sense mappings so users can assign X/Y axis targets and aux encoder bindings from `L2: Sense` without relying only on hardware shortcut gestures.

**Goal:**
- Reuse the existing parameter-selection browser currently used by `L4: Dance > X/Y Pad`.
- Do not duplicate the parameter-tree/menu-generation logic.
- Roll out in two stages: aux `turn` first, aux `click` second.

**Stage 1: Turn mappings**
- Add `L2: Sense > Pn > Mappings`.
- Under `Mappings`, add explicit menu-based target selection for:
- `X Axis` param-mod slots 1 and 2.
- `Y Axis` param-mod slots 1 and 2.
- invert toggles for each X/Y slot.
- `Aux Turns` for available aux encoders.
- Use the same shared parameter-browser code path as Dance X/Y target selection.
- Add dedicated setter actions for:
- per-part param-mod slot assignment/clear.
- aux encoder turn binding assignment/clear.
- Preserve existing hardware shortcuts:
- Shift+grid assignment overlay for part param-mod mapping.
- Shift+aux press binding for highlighted menu parameters.

**Stage 2: Click mappings**
- Extend `L2: Sense > Pn > Mappings` with explicit `Aux Clicks` entries.
- Use a shared action picker for click bindings rather than the numeric/enum/bool parameter picker.
- Initial click-picker scope should match the bindable actions already supported by current aux-click assignment behavior.
- Add dedicated setter actions for aux click binding assignment/clear.
- Preserve existing Shift+aux click shortcut behavior.

**Menu shape target:**
- `L2: Sense > Pn > Mappings > X Axis > Slot 1 / Slot 2`
- `L2: Sense > Pn > Mappings > Y Axis > Slot 1 / Slot 2`
- `L2: Sense > Pn > Mappings > Aux Turns > Aux 1..N`
- `L2: Sense > Pn > Mappings > Aux Clicks > Aux 1..N` (Stage 2)

**Implementation notes:**
- Extract or reuse a shared helper around the current Dance X/Y target group builder.
- Use `compactSourcePathFromKey()` for menu detail text so current assignments display as concise source paths.
- Updating aux bindings must keep `runtimeConfig.auxBindings` and `system.auxBindings` in sync.
- Clearing one side of an aux binding must preserve the other side.

**Acceptance:**
- `L2: Sense > Pn > Mappings` exists for each part.
- Stage 1: X/Y slot targets and aux turn targets can be assigned and cleared from menus using the shared parameter browser.
- Stage 1: existing Dance X/Y parameter browser still uses the same code path and behavior.
- Stage 1: existing hardware shortcut workflows still work.
- Stage 2: aux click bindings can be assigned and cleared from menus using a shared action picker.
- Docs/help text/tests are updated for both stages as they land.

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
