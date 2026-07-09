# Menu and Controls Spec (Authoritative)

This is the entry point for the canonical menu/control spec. The full menu tree is split into `docs/menu-tree-spec.md`; that file is part of this authoritative spec and must stay in sync with native menu changes.

Context-help copy source: `resources/menu-help-texts.tsv` (required header row). Each row provides a title plus two short text fields. Keep one idea per text field; the runtime may join and wrap them for the target display.
Platform capability source: `resources/platform-capabilities.json`; generated TypeScript and Rust constants must stay in sync with it.

## Cheat Sheet

| Combo | Function | Notes |
|---|---|---|
| Shift + Space | Emergency Stop | Internal sync: panic + stop/reset.
| Shift + Space (external sync) | Resync arm | External sync: does not emergency-stop transport.
| Shift + Back | Clear active layer | Re-initializes current active layer behavior state.
| Shift + Fn | Combined modifier | Acts as its own logical button; Fn and Shift are inactive while both physical buttons are held.
| Combined modifier + Main press | Context help | Opens help for highlighted menu entry.
| Fn + leftmost grid column | Navigate parts (1..8) | Mirrors `L1: Life > Part`.
| Fn held + leftmost column LEDs | Navigation indicators | Gray = available layers, green = current active layer.
| Fn + rightmost grid column | Navigate Dance pages | Opens `L4: Dance` and enables Dance page if currently off; exits Dance if already active.
| Sample assign + Shift + cell | Row assign step | Applies current selected-cell assign step to the whole row.
| Sample assign + combined modifier + cell | Column assign step | Applies current selected-cell assign step to the whole column.
| Fn + Aux press | Alternate aux binding | Binds the focused bindable value as that aux Turn target, or focused action as its `!` press target.

## Control Mapping

| Control | Simulator Key | Function |
|---|---|---|
| Main encoder turn | ← → | Move cursor / adjust values |
| Main encoder press | Enter | Enter group / enter/exit edit / trigger action |
| Back button | Backspace / Esc | Go back / exit edit / clear grid (with Shift) |
| Space button | Space | Play / Pause |
| Shift + Space | Shift+Space | Emergency stop (panic + reset scan origin) |
| Shift + Back | Shift+Backspace / Shift+Esc | Clear grid (re-initialize behavior) |
| Aux encoder 1-3 turn | (simulated) | Adjust bound turn mapping |
| Aux encoder 1-3 press | (simulated) | Trigger bound press mapping |
| Fn + Aux encoder press | Fn + (simulated) | Alternate action: bind current value as Turn target or current action as `!` press target |
| Shift + Fn | Shift+Ctrl | Combined modifier; acts as its own logical button and disables Fn/Shift functions while both are held |
| Combined modifier + Main press | Shift+Ctrl+Enter | Context help for highlighted entry |
| Fn + leftmost grid column | Ctrl + leftmost grid column | Navigate active part (1..8); hold Fn to see part indicators |
| Fn + rightmost grid column | Ctrl + rightmost grid column | Navigate/activate L4 Dance page; hold Fn to see page indicators |
| Sample assign mode + Shift + cell press | Shift + cell | Apply current assign toggle/level step to entire row |
| Sample assign mode + combined modifier + cell press | Shift+Ctrl + cell | Apply current assign toggle/level step to entire column |

Simulator grid drag behavior follows the active behavior's declared interaction mode. Paint behaviors drag-toggle/draw cells for editing; momentary behaviors such as Keys release the previous cell when the pointer enters another cell, matching a single finger sliding across grid buttons.

Help popup behavior:

- Main encoder turn scrolls help text
- Main encoder press closes help

## Transport States

- Play: `▶` (green flash on beat, red flash on measure)
- Pause: `⏸`
- Stop (emergency): `■`

## Menu Tree

The full native menu tree lives in [`menu-tree-spec.md`](menu-tree-spec.md). Keep that file in sync with native menu/control changes.

## OLED Display

- 128×128 pixel, simulated in desktop app
- 20 characters × 8 lines of text (5×7 font, 16px line height)
- Top line: title bar (colored by section)
- Canonical section colors: `L1: Life` = life color, `L2: Sense` = sense color, `L3: Voice` = voice color, `L4: Dance` = white, `System` = sepia.
- Body lines 2-8: menu items use a `> ` marker and inverted highlight on the selected row, and `* ` when editing; while browsing, selected value rows stay compact on one row (for example `> Cutoff 127`) instead of adding a separate value row
- Native menu snapshots include rendered-row scroll metadata (`scrollOffset`, `totalRows`, `visibleRows`) for the current body window. Desktop renders this as a 1-2 px scrollbar inside the OLED body only when total rendered rows exceed visible body rows; it does not consume text columns and is omitted for splash/help/confirm overlays unless menu metadata is present.
- Context help for every submenu, parameter, and action must resolve to a specific row from `resources/menu-help-texts.tsv`; generic fallback help is not allowed and native tests must fail on missing coverage.
- Platform-sized menu/runtime limits such as part count, instrument count, sample slots, bus count, global FX slots, touch-FX concurrency, scan section counts, OLED size, and pan position count come from `resources/platform-capabilities.json`.
- Splash graphics use provided logo assets: regular logo for startup/wakeup, sepia logo for sleep/shutdown.
- Bottom-right corner: transport icon (`▶` / `⏸` / `■`), hidden while a footer toast is active
- Transport color: stop is red, pause is white, play is white at rest and flashes green on beats or orange on measures. The NeoKey Play button uses the same stopped/paused/playing flash semantics, but its playing rest state stays dark green rather than white.
- Event dot: briefly shown when notes fire, hidden while a footer toast is active; turns red when recent voice stealing occurred
- Top-right audio load indicator: hidden when idle, yellow when DSP load is moderate or recent voice stealing occurred, red when DSP load is heavy
- Toast text: displayed at bottom for feedback messages

Value editing semantics:

- Number/enum/bool rows enter edit mode on main press
- Navigation memory is limited to `System`, `System > Sound`, and `System > UI`. It is native, ephemeral, cleared on menu rebuild, and does not apply to any other menu, dynamic list, sample browser, preset list, MIDI port list, parameter picker, help, confirm dialog, or assignment overlay.
- `System > Sound > Output Buffer` persists Pi output buffer frames as `runtimeConfig.sound.audioOutputBufferFrames` with choices `64/128/256/512/1024/2048`, default `256`. Changing it shows `Restart device to apply`; leaving the edited row opens the standard `Confirm Reboot` dialog. Audio is not reopened live. On Pi startup, `OCTESSERA_AUDIO_OUTPUT_BUFFER_FRAMES` remains the higher-priority override.
- Browsing selected values are shown on the selected label row; edit mode uses a separate value-focused row for clarity.
- Bool behaves like a 2-option enum (`off`/`on`) and changes on encoder turn, not immediate row press
- Named target selectors (instrument slot, part index, mixer route) display their computed names via `formatDisplayValue()` (e.g. `I1: synth`, `P3: rain`, `fx_bus_2`)
- Behavior `none` hides L1 Step Rate, dynamic behavior config rows, and Reset while preserving stored values. Instrument Type `none` hides Note Mode, engine-specific params, mixer/MIDI rows, and Slot Actions while preserving stored config.
- Parameter target pickers mirror the main menu root order (`L1: Life`, `L2: Sense`, `L3: Voice`, `L4: Dance`, `System`) so modulation, Aux, and XY target browsing use the same mental model as normal navigation. Within `L1: Life`, Behavior targets are generated per part: parts with behavior `none` expose no behavior targets; real behavior parts expose their own Step Rate as `parts.N.algorithmStep` and config fields/actions as `parts.N.l1.behaviorConfig.*`.
- When `Number Style` is `bar` or `bar+numbers`, bounded sound/control/behavior number items render with a smooth geometric bar (filled rectangle) alongside the numeric value
- Bar display applies automatically to FX params, synth/sample shaping controls, mixer volume/pan, touch FX controls, system sound/UI controls, L2 axis controls, and behavior controls such as spawn interval/count, threshold, lifespan, and radius
- Selector-like numeric rows stay plain text, including MIDI channels, instrument/sample slots, part selectors, and MIDI note ranges
- Structural selector edits apply immediately while the row is in edit mode through key-specific fast paths. This covers behavior type, instrument type, instrument route, FX bus slot type, and master FX slot type. Dynamic parameter rows also apply immediately while editing.
- Bar value text uses compact units where useful: `%`, `ms`/`s`, `Hz`, `bpm`, `dB`, semitones/cents, and pan as `L15`/`C`/`R15`; ambiguous internal `0..1` ranges display as `0..100`
- `L2: Sense > Swing` is a global groove amount. `0%` is straight timing. Swing delays internal off-beat step/scan progression and catches up before the next beat; external MIDI clock output remains straight.

Action row markers:

- `!` prefix means the row is an action item

## Grid LED Behavior (NeoKey per-key RGB)

Each cell in the 8×8 grid is mapped to an LED with color based on its `CellTriggerType`:

| Condition | Color |
|---|---|
| Cell off | Off (0, 0, 0) |
| `activate` | Bright white |
| `stable` | Green |
| `deactivate` | Dim white |
| `scanned` | Red (only if scan mode is "scanning") |

Brightness is scaled by the Grid Bright setting.

Overrides:

- While Fn is held for navigation: leftmost column shows part selectors (gray) and active part (green).
- While sample assignment mode is active: grid shows assignment overlay (selected-slot colors, other-slot dim white, unassigned dark).
- While any Dance Page (`mix`, `pan`, `fx`, `trigger-gate`, `transpose`, `xy`) is active: grid shows the Dance performance overlay instead of active behavior cells. Dance Transpose uses the left column for eligible part selection, Shift + left column to enable/disable all eligible parts, and columns 1..7 as a three-octave piano offset picker for synth and enabled MIDI note targets only.
- When Ghost Cells is on, inactive parts' active cells render as very dim green behind the active part. Active part cells and sample assignment overlays take priority.
- Active context changes use OLED toast/status feedback, for example `Part: P3 rain` or `Dance: fx`; these toasts do not change LED overlay priority. Modal help/confirm displays keep display priority over context feedback.
- Holding Shift, Fn, or Shift+Fn for more than one second without another mapped action shows a concise hint toast (`Shift: map/edit`, `Fn: nav/alt`, or `Help: Sh+Fn+Enter`). Startup uses the same chord wording: `Help: Sh+Fn+Enter`. Existing toasts, help/confirm dialogs, assignment overlays, and consumed mappings suppress the hint.

## Sectioned Scanning

- `Sections=1` preserves current scan behavior: `columns` scans one full column per step; `rows` scans one full row per step.
- `Sections=2`, `4`, or `8` split the perpendicular axis into that many lanes and scan each lane in sequence.
- For `rows` with `Sections=2`, each lane is 4 rows tall; the scan ray moves left-to-right across lane 1, then lane 2. Total steps: `gridWidth * sections`.
- For `columns` with `Sections=2`, each lane is 4 columns wide; the scan ray moves bottom-to-top/top-to-bottom by row across each lane. Total steps: `gridHeight * sections`.
- Stop/emergency reset scan index to origin.
- `Restart Section` on Pitch Steps makes pitch stepping local to the lane for the matching scan orientation: X restart applies to column sections; Y restart applies to row sections.
- Note mapping builds the concrete notes in `Low Note..High Note` that match `Scale` and `Root`, chooses the nearest scale note to `Start Note` as the zero-degree index, and applies X/Y pitch steps before clamp/wrap. `wrap` wraps within that concrete scale-note list, so wrapped notes must remain in scale.

## Auto-Save

- Location: System > Saves > Default > Auto Save
- Location: System > Saves > Default > Backups
- When enabled: native menu edits and aux-bound value changes emit deferred `store_save_default` effects; fast audio-facing edits update state/audio immediately and coalesce `ConfigPayload` generation for about 150ms so storage writes the latest settled value instead of saving every intermediate encoder step
- Disabled by default
- Toggling Auto Save on triggers an immediate save when you exit that menu row
- Explicit Save Default is always immediate and cancels any pending deferred default save
- Backups are enabled by default. When any persistent config changes, runtime may emit `store_save_backup` at most once every five minutes; hosts keep the latest 20 `bak-{timestamp}.json` files.
- Confirmed shutdown/reboot emits `store_save_recovery`; Pi writes the latest recovery payload synchronously before setting the power request.
- Loading default, preset, or factory config stops transport, resets position, and sends MIDI panic/equivalent note clearing before applying the loaded config.

## Aux Encoder Binding

- Each aux encoder has two independent custom slots:
  - turn slot: bound to value parameters (number/enum/bool)
  - press slot: bound to actions
- Fn + aux press is an alternate action on a bindable item that binds/overwrites the relevant custom slot:
  - while editing a value item: binds Turn slot
  - while selecting an action item: binds `!` press slot
- In the Fn-held aux overlay, plain labels are turn targets and `!Label` entries are press actions; `/` means both slots are present for that encoder.
- Regular aux press triggers the press slot action (if any)
- Regular aux turn adjusts the turn slot value (if any)
- `Auto Map` lives under `System > UI`. When enabled, context-sensitive auto mappings fill unbound aux slots for the active menu context; custom aux bindings keep precedence when present.
- In supported contexts, focused menu rows show auto-map indicators like `1-Cutoff` and `1!Assign`, preserving selection markers on focused rows such as `> 1!Assign`.
- If no slot is bound, toast shows `S#: No binding` or `T#: No binding`
- Turn toasts show current value, e.g. `T1: Spawn Count: 3`
- Shared route currently implemented:
  - `trigger.life.spawn_now` resolves per behavior (sequencer has no implementation)
- Enum turning is clamped (no wrap)
- Bool turning is clamped with directional behavior (`-1 => Off`, `+1 => On`)
- `activeBehavior` and `behaviorConfig.*` updates re-initialize behavior state
- All aux value changes schedule the deferred auto-save when enabled

### Stale (Inactive) Binding Detection

- Bindings are **not** automatically removed when the target context changes
- If a bound target becomes inactive, the input is ignored and a scoped `not active` toast is shown
- The binding remains intact so the user can re-activate the target later

#### Turn (Stale Target)
- **FX param**: param does not exist for the current slot type, e.g. `T1: B1 Time ms not active`
- **Instrument subtree**: instrument type changed away from the bound subtree, e.g. `T1: I1 Filter cutoff not active`
- **Part scan field**: `scanMode` is not `"scanning"`, e.g. `T1: P1 Scan Direction not active`
- **Behavior config param**: param is not in the current behavior's `configMenu()`, e.g. `T1: P1 Spawn Count not active`

#### Press (Stale Action)
- **Spawn route**: current behavior has no spawn action, e.g. `S1: P1 Spawn Now not active`
- **Concrete action**: action type is not in current behavior's `configMenu()`, e.g. `S1: P1 Spawn Random not active`

#### Scope Prefixes
- `B<N+1>` — bus number (1-indexed)
- `I<N+1>` — instrument number (1-indexed)
- `P<N+1>` — part number (1-indexed)
- Global behavior config uses active part scope `P<active+1>`

### Toast Scrolling

- Toast messages are rendered on a single OLED bottom line (max 17 chars visible)
- Messages longer than 17 chars scroll horizontally:
  - Hold at start: 700ms
  - Scroll at 120ms per character
  - Hold at end once the final window is reached
- `startedAtMs` tracks the original toast creation time; extending a visible toast preserves the scroll position

## Config Persistence (ConfigPayload)

- Native `ConfigPayload` is produced and consumed by `crates/playback-runtime/src/native_runner.rs`.
- It stores active behavior, per-part behavior/config/state, Sense settings, mapping, instruments, mixer, FX, Dance settings, MIDI settings, UI settings, and persistence flags.
- Restore accepts current payloads and supported older saved shapes, sanitizes external compatibility data, then applies only native-owned runtime/core fields.
- Behavior state is restored when saved and compatible; behavior changes initialize the new behavior state through the native behavior engine.
- Transport timing accumulators are reset on restore so loaded configs start from a deterministic runtime position.

## Brightness Behavior

- OLED Bright scales OLED display intensity in host display adapters.
- Grid Bright scales matrix LED RGB intensity.
- Button Bright scales NeoKey button LED intensity.

## Modulation Behavior

- Pitch modulation is additive across axes (`X Steps + Y Steps`).
- Axis pitch steps are signed (`-16..16`).
- Pitch note generation uses scale-degree stepping (not post-quantize).
- `Velocity` lane modulates outgoing `note_on` velocity.
- `Filter Cutoff` lane emits CC74 (mapped to lowpass cutoff).
- `Filter Resonance` lane emits CC71 (mapped to lowpass resonance).
- `Grid Offs` rotates axis indexing (offset=5 => cell 5 treated as first, then wraps).
- `Grid Offs` bounds are derived: `-(GRID_SIZE-1) .. +(GRID_SIZE-1)` → `-7..7`.

## Edit Marker

- Selected editable value line uses compact marker: `*Value`.
- In text edit mode: `*` prefix and cursor shown within the text.

## Native Behavior Contract

Native behaviors implement the Rust `BehaviorEngine` trait in `crates/platform-core/src/behavior.rs` and are registered from `crates/platform-core/src/behaviors/`.

Behavior engines provide:

- stable behavior id
- initial state from config
- input and tick transitions
- render model for the grid
- serialization/deserialization for saved state
- optional behavior config menu rows
- optional immediate input-transition interpretation
- optional grid interaction mode such as paint or momentary

All behaviors use `CellTriggerType`: `activate`, `stable`, `deactivate`, `scanned`, or `none`.

### Input Events

`DeviceInput` supports `grid_press` and `grid_release` events. Behaviors that do not handle `grid_release` simply ignore it. `keys` uses press→activate and release→deactivate semantics; `looper` uses the same live semantics and can overdub step-quantized press/release events into its loop.

Looper uses a `Punch In/Out` action instead of an editable mode row. Pressing it toggles between overdub and play, preserves the recorded loop and live playback state, and shows `Looper: Overdub` or `Looper: Play`.

When a behavior enables immediate input-transition interpretation, `platform-core` interprets grid changes from input through the same Sense/mapping pipeline used during tick, producing immediate musical events. `keys` and `looper` use this to provide immediate finger-drumming response.

## 4 Trigger Types

| Type | Source | When |
|---|---|---|
| `activate` | Algorithm | Cell becomes active (birth, shape hits cell, etc.) |
| `stable` | Algorithm | Cell stays active (alive, inside shape interior, etc.) |
| `deactivate` | Algorithm | Cell becomes inactive (death, shape leaves cell, etc.) |
| `scanned` | Scanning layer | Cell found active during scan (only in "scanning" mode) |

Scan mode "none" generates NO `scanned` triggers. Only "scanning" mode (column/row) generates `scanned` triggers.
`State Notes` only controls non-scan state-note events; `scanned` triggers remain active while scanning.

## Maintenance Rule

Any control/menu/runtime behavior change must update this document in the same commit.
