# Menu and Controls Spec (Authoritative)

This is the single source of truth for menu structure, control mappings, and parameter behavior.

Context-help copy source: `packages/platform-core/resources/menu-help-texts.tsv` (required header row).

## Cheat Sheet

| Combo | Function | Notes |
|---|---|---|
| Shift + Space | Emergency Stop | Internal sync: panic + stop/reset.
| Shift + Space (external sync) | Resync arm | External sync: does not emergency-stop transport.
| Shift + Back | Clear active layer | Re-initializes current active layer behavior state.
| Shift + Fn + Main press | Context help | Opens help for highlighted menu entry.
| Fn + leftmost grid column | Select active part (1..8) | Mirrors `L1: Life > Part`.
| Fn held + leftmost column LEDs | Layer indicators | Gray = available layers, green = current active layer.
| Fn + rightmost grid column | Toggle Touch | Opens `L4: Touch` and enables Touch page if currently off; exits Touch if already active.
| Sample assign + Shift + cell | Row assign step | Applies current selected-cell assign step to the whole row.
| Sample assign + Fn + Shift + cell | Column assign step | Applies current selected-cell assign step to the whole column.
| Shift + Aux press | Bind/unbind aux mapping | Opens bind/unbind flow for focused item.

## Control Mapping

| Control | Simulator Key | Function |
|---|---|---|
| Main encoder turn | ← → | Move cursor / adjust values |
| Main encoder press | Enter | Enter group / enter/exit edit / trigger action |
| Back button | Backspace / Esc | Go back / exit edit / clear grid (with Shift) |
| Space button | Space | Play / Pause |
| Shift + Space | Shift+Space | Emergency stop (panic + reset scan origin) |
| Shift + Back | Shift+Backspace / Shift+Esc | Clear grid (re-initialize behavior) |
| Aux encoder 1-4 turn | (simulated) | Adjust bound turn mapping |
| Aux encoder 1-4 press | (simulated) | Trigger bound press mapping |
| Shift + Aux encoder press | Shift + (simulated) | Bind current item / open unbind confirm |
| Shift + Fn + Main press | Shift+Ctrl+Enter | Context help for highlighted entry |
| Fn + leftmost grid column | Ctrl + leftmost grid column | Select active part (1..8); indicators show only while Fn is held (gray=available, green=active) |
| Fn + rightmost grid column | Ctrl + rightmost grid column | Toggle L4 Touch performance layer |
| Sample assign mode + Shift + cell press | Shift + cell | Apply current assign toggle/level step to entire row |
| Sample assign mode + Fn + Shift + cell press | Fn + Shift + cell | Apply current assign toggle/level step to entire column |

Simulator grid drag behavior follows the active behavior's declared interaction mode. Paint behaviors drag-toggle/draw cells for editing; momentary behaviors such as Keys release the previous cell when the pointer enters another cell, matching a single finger sliding across grid buttons.

Help popup behavior:

- Main encoder turn scrolls help text
- Main encoder press closes help

## Transport States

- Play: `▶` (green flash on beat, red flash on measure)
- Pause: `⏸`
- Stop (emergency): `■`

## Menu Tree (Full)

### Root Menu

```
Root (group)
├── L1: Life (group)
├── L2: Sense (group)
├── L3: Voice (group)
├── L4: Touch (group)
├── [spacer] (visual separator)
├── Playback (group)
└── System (group)
```

### L1: Life

```
L1: Life
├── P1: ... (group)                              ← one group per part, label computed via partLabel()
│   ├── Behavior: [none | life | sequencer | keys | brain | ant | bounce | shapes | raindrops | dla | glider] ← controls which algorithm runs this part
│   ├── Step Rate: [1/16, 1/8, 1/4, 1/2, 1/1]   ← controls how often onTick() is called
│   ├── ... per-behavior dynamic config from behavior's configMenu()
│   ├── Save Grid State: [on | off]              ← controls whether this part's current grid/runtime state is stored in preset/default saves
│   ├── Auto Name: [on | off]                    ← on: name auto-derives from behavior ID; off: name is manual text
│   └── Part Name: (text, max 32)                ← display name; editing sets Auto Name off
├── P2: ... (group)
└── P3: ... (group)                              ← up to partCount parts total
```

When Auto Name is on, the part name is derived from the active behavior ID (e.g. `life`, `brain`). Editing the Part Name text field switches Auto Name off.
Part selectors (Fn+column selection, L2 Sense Part selector) display the computed part label (e.g. `P1: life`, `P2: rain`).

Behavior-specific config items (from `configMenu()`):

| Behavior | Config Items | Type/Options |
|---|---|---|
| none | *(none)* | — |
| life | Spawn Count: [0..20] | number, step 1 (default 12) |
| life | Spawn Interval: [1..20] | number, step 1 (default 1) |
| life | !Spawn Random [S] | action, shared route `trigger.life.spawn_now` |
| sequencer | *(none)* | — |
| keys | Quantize: [immediate, step] | enum |
| brain | Fire Threshold: [1..6] | number, step 1 |
| brain | !Seed Random [S] | action, shared route `trigger.life.spawn_now` |
| ant | Max Ants: [1..10] | number, step 1 |
| ant | !Spawn Ant [S] | action, shared route `trigger.life.spawn_now` |
| bounce | Max Balls: [1..20] | number, step 1 |
| bounce | !Add Ball [S] | action, shared route `trigger.life.spawn_now` |
| shapes | Shape: [ring, heart, star, plus, x] | enum |
| shapes | Expansion Speed: [1..5] | number, step 1 |
| shapes | Auto Spawn Int: [0=off, 10, 20, 50] | enum |
| shapes | !Spawn Pulse [S] | action, shared route `trigger.life.spawn_now` |
| raindrops | !Drop Now [S] | action, shared route `trigger.life.spawn_now` |
| dla | !Seed Cluster [S] | action, shared route `trigger.life.spawn_now` |
| glider | Glider Spawn Int: [0=off, 1, 2, 4, 8, 16] | enum |
| glider | !Spawn Glider [S] | action, shared route `trigger.life.spawn_now` |

### L2: Sense

```
L2: Sense
├── P1: ... (group)                              ← one group per part
│   ├── Scan Mode: [none | scanning]
│   ├── Scan Axis: [rows | columns]           ← visible when scanning
│   ├── Scan Unit: [1/16, 1/8, 1/4, 1/2, 1/1] ← visible when scanning
│   ├── Scan Direction: [forward | reverse]    ← visible when scanning
│   ├── Sections: [1 | 2 | 4 | 8]              ← visible when scanning; 1=current behavior, higher values scan smaller lanes
│   ├── Event Triggers: [on | off]
│   ├── State Notes: [on | off]                    default on (all parts)
│   ├── Instrument Targets (group)
│   │   ├── Activate Action: [none | note_on | note_off]
│   │   ├── Activate Instrument: [1..8]
│   │   ├── Stable Action: [none | note_on | note_off]
│   │   ├── Stable Instrument: [1..8]
│   │   ├── Deactivate Action: [none | note_on | note_off]
│   │   ├── Deactivate Instrument: [1..8]
│   │   ├── Scanned Action: [none | note_on | note_off]
│   │   ├── Scanned Instrument: [1..8]
│   │   ├── Scanned Empty Action: [none | note_on | note_off]
│   │   └── Scanned Empty Instrument: [1..8]
│   ├── Note Mapping (group)
│   │   ├── Lowest Note: [0..127] step 1
│   │   ├── Highest Note: [0..127] step 1
│   │   ├── Starting Note: [0..127] step 1
│   │   ├── Scale: [chromatic | major | natural_minor | dorian | mixolydian | major_pentatonic | minor_pentatonic | harmonic_minor]
│   │   ├── Root: [C | C# | D | D# | E | F | F# | G | G# | A | A# | B]
│   │   └── Out of Range: [clamp | wrap]
│   ├── X Axis (group)
│   │   ├── Pitch Steps (group)
│   │   │   ├── Enabled: [on | off]
│   │   │   ├── Steps: [-16..16] step 1       ← visible when enabled
│   │   │   └── Restart Section: [on | off]   ← visible when enabled; restarts pitch within column sections
│   │   ├── Velocity (group)
│   │   │   ├── Enabled: [on | off]
│   │   │   ├── From: [0..127] step 1         ← visible when enabled
│   │   │   ├── To: [0..127] step 1
│   │   │   ├── Grid Offset: [-7..7] step 1
│   │   │   └── Curve: [linear | curve]
│   │   ├── Filter Cutoff (group)
│   │   │   ├── Enabled: [on | off]
│   │   │   ├── From: [0..127] step 1
│   │   │   ├── To: [0..127] step 1
│   │   │   ├── Grid Offset: [-7..7] step 1
│   │   │   └── Curve: [linear | curve]
│   │   └── Filter Resonance (group)
│   │       ├── Enabled: [on | off]
│   │       ├── From: [0..127] step 1
│   │       ├── To: [0..127] step 1
│   │       ├── Grid Offset: [-7..7] step 1
│   │       └── Curve: [linear | curve]
│   └── Y Axis (group)
│       └── (same sub-structure as X Axis, keys use y.* prefix, defaults: Pitch Steps steps=1; Restart Section affects row sections)
├── P2: ... (group)
└── P3: ... (group)
```

### L3: Voice

```
L3: Voice
└── Instruments (group)
    ├── Instrument 1..8 (group)                ← compact label e.g. `I1: synth`, `I2: drums` via instrumentLabel()
    │   ├── Type: [none | synth | sample | midi]
    │   ├── Note Behavior: [oneshot | hold] default oneshot
    │   ├── Synth (group, visible when type=synth)
    │   │   ├── Preset > Load (group)      ← per-slot synth preset load with confirm
    │   │   ├── Oscillator (group)
    │   │   │   ├── Osc 1 (group)          ← Wave, Octave, Level, Detune, Pulse Width
    │   │   │   └── Osc 2 (group)          ← same sub-items
    │   │   ├── Filter (group)             ← flat: Type, Cutoff, Res, Env Amt, Key Track, Envelope
    │   │   └── Volume (group)             ← Amp (Gain, Vel Sens) + Envelope (ADSR)
    │   ├── Sample (group, visible when type=sample)
    │   │   ├── Sample Slot: [1..8]
    │   │   ├── Choose Sample (group)      ← browses `samples/` tree (wav only)
     │   │   ├── !1-Assign (action)           ← enters grid assignment mode for selected sample slot
    │   │   ├── Velocity Levels: [on | off]
    │   │   ├── Level High / Medium / Low: [1..127] (visible when Velocity Levels=on)
    │   │   ├── Base Velocity: [1..127]    ← used when Velocity Levels=off
    │   │   ├── Tune Semis: [-24..24]
    │   │   ├── Filter (group)             ← sample filter + filter envelope (before Volume)
    │   │   └── Volume (group)             ← sample amp + amp envelope
    │   ├── Note Settings (group, visible when type=midi)
    │   │   ├── Velocity: [1..127]
    │   │   └── Duration: [10..2000] ms
    │   ├── Mixer (group)
    │   │   ├── Route: [direct | fx_bus_1..fx_bus_N] default direct (N from platform capabilities)
    │   │   ├── Volume: [0..100] default 100
    │   │   └── Pan Pos: [0..gridWidth-1] quantized
    │   ├── !Clone (action)               ← duplicates instrument config to next free slot
    │   ├── !Reset (action)               ← resets instrument to factory defaults
    │   ├── MIDI (group)
    │   │   ├── Enabled: [on | off]       default off
    │   │   └── Channel: [1..16]
    │   ├── Auto Name: [on | off]         ← on: name auto-derives from Type; off: name is manual text
    │   └── Name: (text, max 32)          ← display name; editing sets Auto Name off
    └── FX Buses (group)
        ├── Bus 1..4 (group)
        │   ├── Slot 1 (group)
        │   │   ├── Type: [none | reverb | delay | tremolo | chorus | flanger | vibrato | auto_pan | filter_lfo | wah | eq | compressor | duck | saturator | distortion | bitcrusher | glitch] default none
        │   │   └── (effect params, visible per Type)
        │   ├── Slot 2 (group)
        │   │   ├── Type: [same options] default none
        │   │   └── (effect params, visible per Type)
        │   ├── Pan Pos: [0..gridWidth-1] quantized
        │   ├── Auto Name: [on | off]     ← on: name auto-derives from FX slot types; off: name is manual text
        │   └── Name: (text, max 32)      ← display name; editing sets Auto Name off
        └── ... (per bus)
```

Routing semantics:

- Instrument `Volume` is a post-voice per-slot fader controlled by `L4: Touch > mix`.
- Instrument `Route=direct` sends post-fader output to main mix using instrument `Pan Pos`.
- Instrument `Route=fx_bus_n` sends post-fader output to the selected FX bus (exclusive send).
- Internal synth and sample instruments use the same route/pan/bus-FX mixer path; MIDI instruments emit external MIDI and are not processed by audio FX.
- Each bus runs `Slot 1` then `Slot 2` in order; with `none` selected this is passthrough.
- Selecting a slot `Type` initializes that effect's editable parameter defaults immediately; loaded presets/defaults with missing or invalid effect params are repaired to those defaults.
- Bus output is then panned by bus `Pan Pos` and summed to main mix.
- `duck` source options are stable and capability-sized: `I1..I{instrumentCount}` and `B1..B{busCount}`.
- `auto-pan` modulates the bus stereo output position after the slot chain.
- FX bus naming mode: `auto` builds from assigned slot types (e.g. `delay+reverb`, or `fx` when all slots are empty); `custom` allows free text; other modes set a fixed name (`rhythm`, `melody`, `texture`, `fx`).

Sample assignment mode semantics:

- Enter via `L3: Voice > Instruments > Instrument N > Sample > Assign`
- Back exits assignment mode
- One sample assignment per cell (new assignment replaces old cell assignment)
- With Velocity Levels ON, selected-slot cell presses cycle: `Off -> High(red) -> Medium(yellow) -> Low(green) -> Off`
- With Velocity Levels OFF, selected-slot cell presses toggle: `Off <-> Assigned(white)`
- Cells assigned to other sample slots are shown as dim white during assignment editing
- Shift + cell applies the same toggle/step to the whole row
- Fn + Shift + cell applies the same toggle/step to the whole column
- In `Choose Sample`, Space previews the highlighted wav file (folders and `..` are no-op)
- Sample preview is a direct audition path; assigned sample playback from grid/musical events follows instrument route/pan and bus FX.

Part runtime behavior:

- All 8 parts run in parallel while transport is running.
- Switching active part never clears/reset any part state automatically.
- Switching part shows the selected part's current state immediately.
- `Save Grid State` affects preset/default save payload persistence only.

### L4: Touch

```
L4: Touch
├── Touch Page: [none | mix | pan | fx]
├── BPM: [40..240] step 1  default 120
└── FX Page (group)
    ├── FX Type: [none | stutter | freeze | filter_sweep | pitch_shift]
    ├── Target: [master | fx_bus_1..N | instrument_1..N]
    ├── Stutter params (visible when FX Type = stutter)
    │   ├── Rate Hz: [1..32]
    │   └── Depth: [0..100] (% wet mix)
    ├── Freeze params (visible when FX Type = freeze)
    │   ├── Release Ms: [10..5000]  (release fade duration)
    │   └── Mix: [0..100] (% wet mix)
    ├── Filter Sweep params (visible when FX Type = filter_sweep)
    │   ├── Cutoff: [0..100]  (target cutoff amount)
    │   ├── Res: [0..100]
    │   ├── Sweep In: [10..3000] ms  (sweep to target on press)
    │   └── Sweep Out: [10..3000] ms  (sweep back to open on release)
    ├── Pitch Shift params (visible when FX Type = pitch_shift)
    │   ├── Semitones: [-24..24]
    │   ├── Cents: [-100..100] (fine detune, added to semitones)
    │   └── Mix: [0..100] (% wet mix)
    └── Map to Grid (action)
```

Touch layer behavior:

- Fn + rightmost grid column selects Touch pages by row: row 0 = mix, row 1 = pan, row 2 = fx. Lower rows are unused.
- Fn + leftmost grid column selects the active displayed part and exits Touch by setting Touch Page to `none`. Menu position is not changed by part selection.
- When Fn is held, the left grid column shows part-selection options and the right grid column shows Touch page options. The active part and selected Touch page are highlighted; parts whose behavior is not `none` have a dim indicator; `none` parts stay dark. All other cells (columns 1 through 6) are dimmed to 25% brightness to make the navigation columns unambiguous.
- `mix`: each column is an instrument; y=0 mutes, y=7 sets 100%, intermediate rows quantize per-slot `Mixer > Volume`.
- `mix` LEDs show the current volume marker in green.
- `pan`: each row is an instrument; x=0 is hard left and x=7 is hard right. The marker is two cells wide so center positions are visible as the middle pair.
- `pan` writes the audible pan target: for `Route=direct` instruments it sets `Mixer > Pan Pos`; for bus-routed (`fx_bus_n`) instruments it sets the bus pan (`Mixer > Buses[n] > Pan Pos`) plus the per-instrument pan for state preservation. The marker color reflects the route: white for direct, bus color (purple/cyan/green/amber for bus 1-4) for bus-routed instruments. Multiple instruments on the same bus show synchronized markers at the bus pan position.
- `pan` uses a **pressed-cell-plus-right-cell** mapping: pressing grid column X stores `panPos = X+1` (clamped to the right edge), and the LED marker lights cells `panPos-1` and `panPos`. Pressing column 3 (0-indexed) stores `panPos=4`, lighting display cells 4 and 5, which represents center.
- `fx`: grid cells trigger mapped momentary effects. Press starts the mapped effect and release stops it.
- FX cells are mapped from `L4: Touch > FX Page`: select an `FX Type`, edit its visible parameters, then select `Map to Grid` and press a grid cell. The effect type and current parameter values are stored on that cell. Mapping `none` clears a cell.
- FX assignments include a `Target` (default `master`). Targets are listed as `master` first, then FX buses, then instruments. Platform-core resolves grid semantics into audio commands; desktop forwards those commands without interpreting Touch/grid meaning; Rust applies the realtime DSP.
- Target insertion points: `instrument_n` is applied on the instrument's outgoing signal before routing/pan; `fx_bus_n` is applied on the bus outgoing signal after bus slot FX; `master` is applied after the final mix.
- FX concurrency is fixed by platform capability at 4. When all slots are active, additional assigned cells gray out and do not respond until a slot frees.
- Pressing a second cell with the same effect type replaces the existing active cell of that type and emits a release for the old cell before activating the new one.
- Stutter captures a short audio segment on press and loops it repeatedly; `Rate Hz` sets segment length (longer at lower rates) and `Depth` controls wet mix. An ease-in ramp (~2ms) and loop-wrap crossfade prevent clicks.
- Freeze captures the early sound burst into an infinite reverb tail on press (injection window ~120ms). The tail sustains while held with no new input after the window closes. On release, the tail fades out over `Release Ms` and the effect is then removed. `Mix` controls the wet/dry blend.
- Filter Sweep starts with the filter fully open (~20kHz, no audible effect) and sweeps toward the target lowpass cutoff over `Sweep In` on press. On release, it sweeps back to fully open over `Sweep Out` and removes the effect when complete. `Cutoff` sets the target position between 20kHz (0) and the lowest cutoff (100). `Res` controls resonance.
- FX LED colours are yellow for stutter, cyan for freeze, orange for filter_sweep, and magenta for pitch_shift. Assigned inactive cells are dim, active cells are bright, and limit-blocked cells are gray.
- Grid releases in Touch mode are consumed by the Touch layer and do not reach the active behavior engine.
- Aux encoder bindings continue to target whichever menu item they were bound to; Touch page switching does not alter bindings.

### System

```
System
├── Presets (group)
│   ├── Library (group)
│   │   ├── Save As (group)
│   │   │   ├── Name: (text, max 32 chars)  ← on exit/press: saves preset
│   │   │   └── Save: (action)
│   │   ├── Save Current: (action)    ← saves currently loaded preset (with confirm)
│   │   ├── Load (group)             ← dynamic: one action per preset
│   │   ├── Rename (group)           ← dynamic: one text+action per preset
│   │   ├── Delete (group)           ← dynamic: one action per preset
│   │   └── Refresh List: (action)
│   ├── Default (group)
│   │   ├── Save Default: (action)
│   │   ├── Load Default: (action)
│   │   └── Auto Save: [on | off]    ← auto-persists settled config after cooldown
│   └── Factory (group)
│       └── Load Fact. Default: (action)
├── Sound (group)                     ← merged: Audio + Sound controls
│   ├── Master Vol: [0..100] step 1  default 73
│   ├── Note Length: [30..2000] step 10 ms  default 120
│   ├── Velocity Scale: [0..200] step 5 %   default 100
│   ├── Velocity Curve: [linear | soft | hard]
│   └── Voice Stealing: [off | lenient | balanced | aggressive]  default balanced
├── MIDI (group)
│   ├── Enabled: [on | off]
│   ├── !Panic: (action)
│   ├── MIDI Out (group)             ← dynamic: one action per detected MIDI output port
│   ├── MIDI In (group)              ← dynamic: one action per detected MIDI input port
│   ├── Sync & Clock (group)
│   │   ├── Sync Mode: [internal | external]
│   │   ├── Clock Out: [on | off]
│   │   ├── Clock In: [on | off]
│   │   └── Respond Start/Stop: [on | off]
└── UI Settings (group)
    ├── Ghost Cells: [on | off]  default off  ← shows dim cells from inactive parts behind active part
    ├── Numeric Display: [bar | numbers | bar+numbers]  ← controls rendering of bar-style numeric params, default bar+numbers
    ├── Screen Sleep: [0..600] step 10 s    default 60 (0=off)
    ├── Display Brightness: [10..100] step 5  default 75 (bar display when Numeric Display is bar or bar+numbers)
    ├── Grid Brightness: [10..100] step 5     default 75 (bar display when Numeric Display is bar or bar+numbers)
    └── Button Brightness: [10..100] step 5   default 75 (bar display when Numeric Display is bar or bar+numbers)
```

## OLED Display

- 128×128 pixel, simulated in desktop app
- 20 characters × 8 lines of text (5×7 font, 16px line height)
- Top line: title bar (colored by section)
- Body lines 2-8: menu items with `@@` prefix on selected line, `*` prefix when editing
- Bottom-right corner: transport icon (`▶` / `⏸` / `■`)
- Transport flash: green (beat) or red (measure) border on play icon
- Yellow event dot: briefly shown when notes fire
- Top-right audio load indicator: hidden when idle, yellow when DSP load is moderate or recent voice stealing occurred, red when DSP load is heavy
- Toast text: displayed at bottom for feedback messages

Value editing semantics:

- Number/enum/bool rows enter edit mode on main press
- Bool behaves like a 2-option enum (`off`/`on`) and changes on encoder turn, not immediate row press
- Named target selectors (instrument slot, part index, mixer route) display their computed names via `formatDisplayValue()` (e.g. `I1: synth`, `P3: rain`, `fx_bus_2`)
- When `Numeric Display` is `bar` or `bar+numbers`, number items with `displayStyle: "bar"` or FX params render with a smooth geometric bar (filled rectangle) alongside the numeric value
- Bar display applies to FX params automatically; other number items opt in via `displayStyle: "bar"` on the menu node

Action row markers:

- `!` prefix means the row is an action item
- `[S]` suffix means the action is shared/routable across behaviors

## Grid LED Behavior (NeoKey per-key RGB)

Each cell in the 8×8 grid is mapped to an LED with color based on its `CellTriggerType`:

| Condition | Color |
|---|---|
| Cell off | Off (0, 0, 0) |
| `activate` | Bright white |
| `stable` | Green |
| `deactivate` | Dim white |
| `scanned` | Red (only if scan mode is "scanning") |

Brightness is scaled by the Grid Brightness setting.

Overrides:

- While Fn is held: leftmost column shows part selectors (gray) and active part (green).
- While sample assignment mode is active: grid shows assignment overlay (selected-slot colors, other-slot dim white, unassigned dark).
- While Touch Page is `mix`, `pan`, or `fx`: grid shows the Touch performance overlay instead of active behavior cells.
- When Ghost Cells is on, inactive parts' active cells render as very dim green behind the active part. Active part cells and sample assignment overlays take priority.

## Sectioned Scanning

- `Sections=1` preserves current scan behavior: `columns` scans one full column per step; `rows` scans one full row per step.
- `Sections=2`, `4`, or `8` split the perpendicular axis into that many lanes and scan each lane in sequence.
- For `rows` with `Sections=2`, each lane is 4 rows tall; the scan ray moves left-to-right across lane 1, then lane 2. Total steps: `gridWidth * sections`.
- For `columns` with `Sections=2`, each lane is 4 columns wide; the scan ray moves bottom-to-top/top-to-bottom by row across each lane. Total steps: `gridHeight * sections`.
- Stop/emergency reset scan index to origin.
- `Restart Section` on Pitch Steps makes pitch stepping local to the lane for the matching scan orientation: X restart applies to column sections; Y restart applies to row sections.

## Auto-Save

- Location: System > Presets > Default > Auto Save
- When enabled: config changes (via turnMenu, pressMenu, or turnAuxEncoder) emit deferred `store_save_default` effects; storage writes the latest pending `ConfigPayload` after a short cooldown instead of saving every intermediate encoder step
- Disabled by default
- Toggling Auto Save on triggers an immediate save when you exit that menu row
- Explicit Save Default is always immediate and cancels any pending deferred default save

## Aux Encoder Binding

- Each aux encoder has two independent slots:
  - turn slot: bound to value parameters (number/enum/bool)
  - press slot: bound to actions
- Shift + aux press on a bindable item binds/overwrites the slot:
  - while editing a value item: binds turn slot
  - while selecting an action item: binds press slot
- Shift + aux press when unbinding would occur opens confirmation with: `Both`, `Click`, `Turn`, `Cancel`
- Regular aux press triggers the press slot action (if any)
- Regular aux turn adjusts the turn slot value (if any)
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

Structured as:
```typescript
type ConfigPayload = {
  activeBehavior: string;
  runtimeConfig: RuntimeConfig;   // all menu-settable parameters
  mappingConfig: MappingConfig;   // activate/stable/deactivate/scanned targets + MIDI note params
};
```

On restore (`applyConfigPayload`), the payload is deep-merged with factory defaults via `sanitizePayload`, then:
- `activeBehavior` and `runtimeConfig` are applied
- `mappingConfig` is applied
- If behavior engine changed, behavior state is re-initialized via `behavior.init({})`
- All timing accumulators are reset to 0

## Brightness Behavior

- Display Brightness scales OLED display intensity in simulator.
- Grid Brightness scales matrix LED RGB intensity.
- Button Brightness scales NeoKey button LED intensity.

## Modulation Behavior

- Pitch modulation is additive across axes (`X Steps + Y Steps`).
- Axis pitch steps are signed (`-16..16`).
- Pitch note generation uses scale-degree stepping (not post-quantize).
- `Velocity` lane modulates outgoing `note_on` velocity.
- `Filter Cutoff` lane emits CC74 (mapped to lowpass cutoff).
- `Filter Resonance` lane emits CC71 (mapped to lowpass resonance).
- Lowpass is currently the active filter type.
- `Grid Offset` rotates axis indexing (offset=5 => cell 5 treated as first, then wraps).
- `Grid Offset` bounds are derived: `-(GRID_SIZE-1) .. +(GRID_SIZE-1)` → `-7..7`.

## Edit Marker

- Selected editable value line uses compact marker: `*Value`.
- In text edit mode: `*` prefix and cursor shown within the text.

## Behavior Engine Interface

```typescript
interface BehaviorEngine<TState, TConfig> {
  id: string;
  init: (config?: TConfig) => TState;
  onInput: (state: TState, input: DeviceInput) => TState;
  onTick: (state: TState, ctx: { bpm: number; emit: () => void }) => TState;
  renderModel: (state: TState) => BehaviorRenderModel;
  serialize: (state: TState) => unknown;
  deserialize: (data: unknown) => TState;
   interpretInputTransitions?: boolean;
   configMenu?: () => BehaviorConfigItem[];
 }
```

All behaviors use `CellTriggerType`: `"activate" | "stable" | "deactivate" | "scanned" | "none"`.

### Input Events

`DeviceInput` supports `grid_press` and `grid_release` events. Behaviors that do not handle `grid_release` (all except `keys`) simply ignore it. The `keys` behavior uses press→activate and release→deactivate semantics.

When a behavior sets `interpretInputTransitions: true`, the platform-core interprets grid changes from `onInput` through the same Sense/mapping pipeline used during tick, producing immediate musical events. The `keys` behavior uses this to provide immediate finger-drumming response.

## 4 Trigger Types

| Type | Source | When |
|---|---|---|
| `activate` | Algorithm | Cell becomes active (birth, shape hits cell, etc.) |
| `stable` | Algorithm | Cell stays active (alive, inside shape interior, etc.) |
| `deactivate` | Algorithm | Cell becomes inactive (death, shape leaves cell, etc.) |
| `scanned` | Scanning layer | Cell found active during scan (only in "scanning" mode) |

Scan mode "none" generates NO `scanned` triggers. Only "scanning" mode (column/row) generates `scanned` triggers.

## Maintenance Rule

Any control/menu/runtime behavior change must update this document in the same commit.
