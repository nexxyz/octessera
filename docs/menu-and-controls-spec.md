# Menu and Controls Spec (Authoritative)

This is the single source of truth for menu structure, control mappings, and parameter behavior.

Context-help copy source: `resources/menu-help-texts.tsv` (required header row).
Platform capability source: `resources/platform-capabilities.json`; generated TypeScript and Rust constants must stay in sync with it.

## Cheat Sheet

| Combo | Function | Notes |
|---|---|---|
| Shift + Space | Emergency Stop | Internal sync: panic + stop/reset.
| Shift + Space (external sync) | Resync arm | External sync: does not emergency-stop transport.
| Shift + Back | Clear active layer | Re-initializes current active layer behavior state.
| Shift + Fn | Combined modifier | Acts as its own logical button; Fn and Shift are inactive while both physical buttons are held.
| Combined modifier + Main press | Context help | Opens help for highlighted menu entry.
| Fn + leftmost grid column | Select active part (1..8) | Mirrors `L1: Life > Part`.
| Fn held + leftmost column LEDs | Layer indicators | Gray = available layers, green = current active layer.
| Fn + rightmost grid column | Toggle Dance | Opens `L4: Dance` and enables Dance page if currently off; exits Dance if already active.
| Sample assign + Shift + cell | Row assign step | Applies current selected-cell assign step to the whole row.
| Sample assign + combined modifier + cell | Column assign step | Applies current selected-cell assign step to the whole column.
| Fn + Aux press | Bind aux mapping | Binds the focused bindable value as that aux Turn target, or focused action as its `!` press target.

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
| Fn + Aux encoder press | Fn + (simulated) | Bind current value as Turn target or current action as `!` press target |
| Shift + Fn | Shift+Ctrl | Combined modifier; acts as its own logical button and disables Fn/Shift functions while both are held |
| Combined modifier + Main press | Shift+Ctrl+Enter | Context help for highlighted entry |
| Fn + leftmost grid column | Ctrl + leftmost grid column | Select active part (1..8); hold Fn to see part indicators |
| Fn + rightmost grid column | Ctrl + rightmost grid column | Select/activate L4 Dance page; hold Fn to see page indicators |
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

## Menu Tree (Full)

### Root Menu

```
Root (group)
├── L1: Life (group)
├── L2: Sense (group)
├── L3: Voice (group)
├── L4: Dance (group)
├── [spacer] (visual separator)
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
| life | !Spawn Random | action, shared route `trigger.life.spawn_now` |
| sequencer | *(none)* | — |
| keys | Quantize: [immediate, step] | enum |
| brain | Fire Threshold: [1..6] | number, step 1 |
| brain | !Seed Random | action, shared route `trigger.life.spawn_now` |
| ant | Max Ants: [1..10] | number, step 1 |
| ant | !Spawn Ant | action, shared route `trigger.life.spawn_now` |
| bounce | Max Balls: [1..20] | number, step 1 |
| bounce | !Add Ball | action, shared route `trigger.life.spawn_now` |
| shapes | Shape: [ring, heart, star, plus, x] | enum |
| shapes | Expansion Speed: [1..5] | number, step 1 |
| shapes | Auto Spawn Int: [0=off, 10, 20, 50] | enum |
| shapes | !Spawn Pulse | action, shared route `trigger.life.spawn_now` |
| raindrops | !Drop Now | action, shared route `trigger.life.spawn_now` |
| dla | !Seed Cluster | action, shared route `trigger.life.spawn_now` |
| glider | Glider Spawn Int: [0=off, 1, 2, 4, 8, 16] | enum |
| glider | !Spawn Glider | action, shared route `trigger.life.spawn_now` |

### L2: Sense

```
L2: Sense
├── Aux Mappings (group)
│   ├── Aux 1 (group)
│   │   ├── Turn (group)
│   │   │   ├── (none) (action)
│   │   │   └── parameter tree...            ← same shared browser as Dance X/Y target selection
│   │   └── Click (group)
│   │       ├── (none) (action)
│   │       └── action tree...               ← behavior actions, sample assign, selected FX map-to-grid
│   ├── Aux 2 (group)
│   └── Aux 3 (group)
├── Events when paused: [on | off]         default on; when on, grid input may still emit events while transport is stopped/paused
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
│   ├── Trigger Prob. (group)
│   │   ├── Mode: [zero | custom | full]
│   │   ├── Low Prob: [0..100] step 1
│   │   ├── High Prob: [0..100] step 1
│   │   └── Map Prob Grid (action)
│   ├── Mappings (group)
│   │   ├── X Axis (group)
│   │   │   ├── Slot 1 (group)
│   │   │   │   ├── (none) (action)
│   │   │   │   └── parameter tree...                ← same shared browser as Dance X/Y target selection
│   │   │   ├── Slot 1 Invert: [on | off]
│   │   │   ├── Slot 2 (group)
│   │   │   │   ├── (none) (action)
│   │   │   │   └── parameter tree...
│   │   │   └── Slot 2 Invert: [on | off]
│   │   ├── Y Axis (group)
│   │   │   └── (same sub-structure as X Axis)
│   ├── Note Mapping (group)
│   │   ├── Low Note: [0..127] step 1          ← lower bound, displayed as note name + MIDI number, e.g. C2 (36)
│   │   ├── High Note: [0..127] step 1         ← upper bound, displayed as note name + MIDI number, e.g. D5 (74)
│   │   ├── Start Note: [0..127] step 1        ← nearest scale start index, displayed as note name + MIDI number, e.g. C4 (60)
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
│   │   │   ├── Grid Offs: [-7..7] step 1
│   │   │   └── Curve: [linear | curve]
│   │   ├── Filter Cutoff (group)
│   │   │   ├── Enabled: [on | off]
│   │   │   ├── From: [0..127] step 1
│   │   │   ├── To: [0..127] step 1
│   │   │   ├── Grid Offs: [-7..7] step 1
│   │   │   └── Curve: [linear | curve]
│   │   └── Filter Resonance (group)
│   │       ├── Enabled: [on | off]
│   │       ├── From: [0..127] step 1
│   │       ├── To: [0..127] step 1
│   │       ├── Grid Offs: [-7..7] step 1
│   │       └── Curve: [linear | curve]
│   └── Y Axis (group)
│       └── (same sub-structure as X Axis, keys use y.* prefix, defaults: Pitch Steps steps=3; Restart Section affects row sections)
├── P2: ... (group)
└── P3: ... (group)
```

### L3: Voice

```
L3: Voice
├── Instruments (group)
│   ├── Instrument 1..8 (group)                ← compact overview label e.g. `I1: synth direct`, `I2: samp B1`, `I3: midi ch1`
│   │   ├── Type: [none | synth | sampler | midi]
│   │   ├── Note Behavior: [oneshot | hold] default oneshot
│   │   ├── Synth (group, visible when type=synth)
│   │   │   ├── Preset > Load (group)      ← per-slot synth preset load with confirm
│   │   │   ├── Osc 1 (group)              ← Wave, Octave, Level, Detune, Pulse Width
│   │   │   ├── Osc 2 (group)              ← same sub-items
│   │   │   ├── Filter (group)             ← Type, Cutoff, Res, Env Amt, Key Track
│   │   │   ├── Volume (group)             ← Gain, Vel Sens
│   │   │   ├── Amp Env (group)            ← ADSR loudness contour
│   │   │   └── Filter Env (group)         ← ADSR filter contour
│   │   ├── Sampler (group, visible when type=sampler)
│   │   │   ├── Sample Slot: [1..8]
│   │   │   ├── S* Browse (group)          ← browses `samples/` tree (wav only)
│   │   │   ├── Assign (action)            ← enters grid assignment mode for selected sample slot
│   │   │   ├── Vel Levels: [on | off]
│   │   │   ├── Level High / Medium / Low: [1..127] (visible when Vel Levels=on)
│   │   │   ├── Base Velocity: [1..127]    ← used when Vel Levels=off
│   │   │   ├── Tune Semis: [-24..24]
│   │   │   ├── Filter (group)             ← sample filter + filter envelope (before Volume)
│   │   │   └── Volume (group)             ← sample amp + amp envelope
│   │   ├── Note Settings (group, visible when type=midi)
│   │   │   ├── Velocity: [1..127]
│   │   │   └── Duration: [10..2000] ms
│   │   ├── Mixer (group)
│   │   │   ├── Route: [direct | fx_bus_1..fx_bus_N] default direct (N from platform capabilities)
│   │   │   ├── Volume: [0..100] default 100
│   │   │   └── Pan Pos: [0..32] quantized (33-position stereo scale; 16=center)
│   │   ├── MIDI (group)
│   │   │   ├── Enabled: [on | off]       default off
│   │   │   └── Channel: [1..16]
│   │   ├── Auto Name: [on | off]         ← on: name auto-derives from Type as display text (`Synth`, `Sampler`, `MIDI`); off: name is manual text
│   │   ├── Name: (text, max 32)          ← display name; editing sets Auto Name off; charset includes uppercase, lowercase, digits, space, `_`, `-`
│   │   └── Actions (group)
│   │       ├── !Clone (action)           ← duplicates instrument config to next free slot, with confirmation
│   │       └── !Reset (action)           ← resets instrument to factory defaults, with confirmation
├── FX Buses (group)
│   ├── Bus 1..4 (group)
│   │   ├── Slot 1: Effect (group)
│   │   │   ├── Type: [none | reverb | delay | tremolo | chorus | flanger | vibrato | auto_pan | filter_lfo | wah | eq | compressor | duck | saturator | distortion | bitcrusher | glitch] default none
│   │   │   └── (effect params, visible per Type)
│   │   ├── Slot 2: Effect (group)
│   │   │   ├── Type: [same options] default none
│   │   │   └── (effect params, visible per Type)
│   │   ├── Pan Pos: [0..32] quantized (33-position stereo scale; 16=center)
│   │   ├── Auto Name: [on | off]     ← on: name auto-derives from FX slot types as display text (`None`, `Delay+Duck`); off: name is manual text
│   │   └── Name: (text, max 32)      ← display name; editing sets Auto Name off; charset includes uppercase, lowercase, digits, space, `_`, `-`
│   └── ... (per bus)
└── Global FX (group)
    ├── Slot 1..N (group, N from platform capability `globalFxSlotCount`; current desktop/Pi Zero target = 2)
    │   ├── Type: [none | vinyl | eq | compressor | saturator | distortion] default none
    │   └── (effect params, visible per Type)
    └── ...
```

Routing semantics:

- Instrument `Volume` is a post-voice per-slot fader controlled by `L4: Dance > mix`.
- Instrument `Route=direct` sends post-fader output to main mix using instrument `Pan Pos`.
- Instrument `Route=fx_bus_n` sends post-fader output to the selected FX bus (exclusive send).
- Internal synth and sample instruments use the same route/pan/bus-FX mixer path; MIDI instruments emit external MIDI and are not processed by audio FX.
- Each bus runs `Slot 1` then `Slot 2` in order; with `none` selected this is passthrough.
- Global FX runs `Slot 1..N` in order on the stereo main mix after direct and bus outputs are summed, before global momentary FX and `Master Vol`.
- FX bus assignments above the recommended active bus warning budget of 6 active bus FX slots are accepted and saved, but the runtime shows a toast warning. Global stereo FX slots do not count toward the bus FX warning budget.
- Global FX is intentionally limited to `none | vinyl | eq | compressor | saturator | distortion` for current Pi Zero 2 W performance targets.
- Selecting a slot `Type` initializes that effect's editable parameter defaults immediately; loaded presets/defaults with missing or invalid effect params are repaired to those defaults.
- Reverb `Decay` is stored as a feedback coefficient (`0..0.995`) but displayed as approximate tail time in seconds (for example `3.1s`) in menu rows and aux encoder toasts.
- Bus output is then panned by bus `Pan Pos` and summed to main mix.
- `duck` source options are stable and capability-sized: `I1..I{instrumentCount}` and `B1..B{busCount}`.
- `auto-pan` modulates the bus stereo output position after the slot chain.
- FX bus slot and global slot group labels include the loaded effect display name, e.g. `Slot 1: Delay`, `Slot 2: Duck`, or `Slot 1: None`.
- FX bus naming mode: `auto` builds from assigned slot types using display names (e.g. `Delay+Reverb`, or `None` when all slots are empty); manual names are preserved exactly. Legacy raw auto names are normalized only when `Auto Name` is on and the stored name is missing or equals the old raw auto-derived value.

Sample assignment mode semantics:

- Enter via `L3: Voice > Instruments > Instrument N > Sampler > Assign`
- Back exits assignment mode
- Entering assignment mode shows a concise OLED toast (for example `Assign S1: grid`); Back continues to exit without changing mappings.
- One sample assignment per cell (new assignment replaces the existing cell assignment)
- With Velocity Levels ON, selected-slot cell presses cycle: `Off -> High(red) -> Medium(yellow) -> Low(green) -> Off`
- With Velocity Levels OFF, selected-slot cell presses toggle: `Off <-> Assigned(white)`
- Cells assigned to other sample slots are shown as dim white during assignment editing
- Shift + cell applies the same toggle/step to the whole row
- Combined modifier + cell applies the same toggle/step to the whole column
- The sample browser menu is labeled with selected slot context (for example `S1 Browse`) and preserves the body rows as browser entries: `..`, built-in/user favourites at the sample root, `[folder]`, file rows, or `(empty)`, followed by a blank separator row and a current-folder favourite action.
- Before directory entries arrive, the browser shows `(loading...)` instead of `(empty)`. Long highlighted names are clipped to the OLED row width rather than overlapping adjacent display areas.
- In `S1 Browse`/sample browser menus, Space previews the highlighted wav file through the selected instrument slot (folders and `..` are no-op); the favourite action toggles the current folder's entry in `runtimeConfig.sampleFavouriteDirs`. Platform built-in favourites are added dynamically and cannot be removed from that config.
- Pi built-in sample favourites are `Samples` (`/home/pi/samples`) and `SD card` (`/home/pi/samples/sd-card`, expected to be an SD-card mount point). If the SD card is not mounted, selecting it shows a clear unavailable message. Desktop exposes a built-in `User data` sample favourite.
- Sample preview and assigned sample playback both follow instrument route, pan, volume, bus FX, and master output gain.

Part runtime behavior:

- All 8 parts run in parallel while transport is running.
- Switching active part never clears/reset any part state automatically.
- Switching part shows the selected part's current state immediately.
- `Save Grid State` affects preset/default save payload persistence only.
- `Step Rate`, behavior selection/config, Sense mapping, trigger probabilities, instruments, mixer, system settings, selected Dance page, Dance FX assignments, X/Y bindings, and aux bindings are persistent and must round-trip through preset/default/autosave payloads.
- Active overlays, assignment modes, held modifiers, active momentary FX instances, live X/Y touch, help popups, and toast state are transient and are not restored from preset/default/autosave payloads.

### L4: Dance

```
L4: Dance
├── Dance Page: [none | mix | pan | fx | trigger-gate | xy]
├── BPM: [40..240] step 1  default 120
├── selected page controls only, flattened here:
│   ├── fx: FX Type, Target, visible params for selected FX Type, Map to Grid
│   └── xy: X Axis, Y Axis, Invert X, Invert Y, Release
```

Dance layer behavior:

- Hold Fn to reveal navigation columns: the leftmost grid column selects the active part using grid Y directly (`y=0` = part 0), and the rightmost grid column selects and activates Dance pages by row: row 0 = mix, row 1 = pan, row 2 = fx, row 3 = trigger-gate, row 4 = xy. Lower rows are unused.
- Fn + leftmost grid selection exits the current Dance overlay without changing the saved Dance Page selection. Menu position is not changed by part selection.
- When Fn is held, the left grid column shows part-selection options and the right grid column shows Dance page options. The active part and saved Dance page are highlighted; parts whose behavior is not `none` have a dim indicator; `none` parts stay dark. All other cells (columns 1 through 6) are dimmed to 25% brightness to make the navigation columns unambiguous.
- `mix`: each column is an instrument; y=0 mutes, y=7 sets 100%, intermediate rows quantize per-slot `Mixer > Volume`.
- `mix` LEDs show the current volume marker in green.
- `pan`: each row is an instrument; x=0 is hard left and x=7 is hard right. The marker is two cells wide so center positions are visible as the middle pair. Stored pan is a 33-position stereo scale (`0..32`, center `16`) shared with the menu and audio engine.
- `pan` writes the audible pan target: for `Route=direct` instruments it sets `Mixer > Pan Pos`; for bus-routed (`fx_bus_n`) instruments it sets the bus pan (`Mixer > Buses[n] > Pan Pos`) plus the per-instrument pan for state preservation. The marker color reflects the route: white for direct, bus color (purple/cyan/green/amber for bus 1-4) for bus-routed instruments. Multiple instruments on the same bus show synchronized markers at the bus pan position.
- `pan` maps the 8 grid columns onto 7 two-cell marker positions: column 0 stores `0` and lights 0+1; column 1 stores `5` and lights 1+2; column 2 stores `11` and lights 2+3; columns 3 and 4 both store center `16` and light 3+4; column 5 stores `21` and lights 4+5; column 6 stores `27` and lights 5+6; column 7 stores `32` and lights 6+7.
- `fx`: grid cells trigger mapped momentary effects. Press starts the mapped effect and release stops it. At most two momentary FX may be active at once, and only one momentary FX of each type may be active. If the active momentary FX limit is reached or another mapping of the same type is already active, the press is ignored and a toast warns the user.
- `trigger-gate`: this Dance page performs live trigger mode overrides for each part; it does not edit the saved per-cell probability map.
- Stored per-part trigger probability data lives in `L2: Sense > Pn > Trigger Prob.`.
- `Map Prob Grid` edits the saved four-state probability map for the selected part. Cell cycle is `zero -> low -> high -> full -> zero`; `Shift+grid` applies to a row; `Shift+Fn+grid` applies to a column.
- Probability-map editor LEDs: black = `0%`, red = `low`, yellow = `high`, green = `100%`.
- `L2: Sense > Aux Mappings` exposes root-level menu-based assignment for aux encoder turn and click bindings plus an `Auto Map` toggle.
- `L2: Sense > Events when paused` controls whether direct grid input can emit musical events while the transport is stopped/paused. Algorithm tick/evolution remains stopped either way.
- `L2: Sense > Pn > Mappings` exposes explicit per-part assignment for X/Y param-mod slots.
- The `Slot` and aux `Turn` target pickers use the same shared parameter browser as `L4: Dance > X/Y Pad`; no separate parameter tree should diverge from that browser.
- Aux `Click` uses a dedicated action browser for click-bindable actions.
- Existing hardware shortcuts remain valid: Shift+grid still assigns X/Y param-mod slots and Fn+aux press binds the currently highlighted menu parameter as a Turn target or action as a `!` press target.
- Trigger-gate Dance layout uses rows as parts with the same orientation as Fn part navigation: bottom row = part 0, top row = highest part.
- Dance columns `0..2` set that row's part mode: `0%` (red), `custom` (yellow), `100%` (green). Selected mode is bright; the other two are dim.
- Dance columns `3..4` are an unassigned dark gap.
- Bottom-row columns `5..7` are always-bright all-parts actions: set all parts to `0%`, `custom`, or `100%`.
- Trigger filtering resolves per-part mode as follows: `zero` blocks all triggers, `full` passes all triggers, `custom` uses the stored per-cell probability map with that part's `Low Prob` and `High Prob` thresholds.
- `Fn+Play` toggles the active part between `0%` and its previously active trigger mode without rewriting the stored probability map. On desktop this is `Fn+Space`.
- FX cells are mapped from the flattened `L4: Dance` FX page controls: select an `FX Type`, edit its visible parameters, then select `Map to Grid` and press a grid cell. The effect type, target, and current parameter values are stored on that cell. Mapping `none` clears a cell.
- Entering FX grid assignment shows a concise `Map FX: ...` toast; Back exits assignment without changing stored cells.
- FX assignments include a `Target` (default `master`). Targets are listed as `master` first, then FX buses, then instruments. Platform-core resolves grid semantics into audio commands; desktop forwards those commands without interpreting Dance/grid meaning; Rust applies the realtime DSP.
- Target insertion points: `instrument_n` is applied on the instrument's outgoing signal before routing/pan; `fx_bus_n` is applied on the bus outgoing signal after bus slot FX; `master` is applied after the final mix.
- FX concurrency is fixed by platform capability at 2. When both slots are active, all other assigned FX cells gray out and do not respond until a slot frees. When one slot is active, other mappings of the same FX type gray out and do not respond until that type frees.
- Pressing a second cell with the same effect type replaces the existing active cell of that type and emits a release for the old cell before activating the new one.
- Stutter captures a short audio segment on press and loops it repeatedly; `Rate Hz` sets segment length (longer at lower rates) and `Depth` controls wet mix. An ease-in ramp (~2ms) and loop-wrap crossfade prevent clicks.
- Freeze captures the early sound burst into an infinite reverb tail on press (injection window ~120ms). The tail sustains while held with no new input after the window closes. On release, the tail fades out over `Release Ms` and the effect is then removed. `Mix` controls the wet/dry blend.
- Filter Sweep starts with the filter fully open (~20kHz, no audible effect) and sweeps toward the target lowpass cutoff over `Sweep In` on press. On release, it sweeps back to fully open over `Sweep Out` and removes the effect when complete. `Cutoff` sets the target position between 20kHz (0) and the lowest cutoff (100). `Res` controls resonance.
- FX LED colours are yellow for stutter, cyan for freeze, orange for filter_sweep, and magenta for pitch_shift. Assigned inactive cells are dim, active cells are bright, and limit-blocked cells are gray.
- Grid releases in Dance mode are consumed by the Dance layer and do not reach the active behavior engine.
- Aux encoder bindings continue to target whichever menu item they were bound to; Dance page switching does not alter bindings.
- `xy`: the full 8×8 grid acts as a continuous two-axis modulation surface. Pressing a grid cell normalizes its X,Y coordinates over 0–1 (full width/height, no margin). The normalized position modulates the per-part targets assigned in `L4: Dance > X/Y Pad > X/Y Axis`. While pressed, the current touch cell is bright white; after release, `sample-hold` leaves a dim gray marker at the held value and `reset-center` returns the dim marker to center.
- `xy` target selection walks the menu tree to present all mappable parameters (same set used by aux encoder binding and Sense X/Y axis modulation). Selecting a target stores the parameter key and value metadata per part (`parts[N].xy`).
- `xy` modulation beats all other modulation sources: it is applied last in `applyModulationResult()`, after `applyParamModulation()`, overwriting the same runtime config keys.
- `xy` grid LEDs: bright white on the touched cell while finger is down; dim gray on sample-hold (when `Release = sample-hold` and finger is lifted); rest of grid is dark.
- `Release: sample-hold` keeps the last modulation values active after lifting the finger. `Release: reset-center` returns X and Y to 0.5 (center) on release.
- `Invert X` / `Invert Y` flip the respective axis: `value = 1 - norm` when enabled, so left becomes max and right becomes min (X axis), or bottom becomes max and top becomes min (Y axis).
- Saved with presets/defaults: selected Dance Page, FX page config and assignments, instrument mix volumes, pan positions, per-part trigger probability mode, low/high thresholds, trigger probability map cell state, X/Y bindings, X/Y invert flags, and X/Y release behavior.
- Not saved: transient performance state such as the currently active Dance overlay on load/startup, the live X/Y touch position (`xyTouch`), active momentary FX instances, assign modes, held modifiers, and other temporary overlays.

### System

```
System
├── Saves (group)
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
│   ├── Factory (group)
│   │   └── Load Fact. Default: (action)
├── Diagnostics (group)
│   └── Hardware Test: (action)       ← confirms, then runs pre-hardware Pi checks
├── Updates (group)
│   ├── Check: (action)               ← read-only update status request
│   ├── Apply: (action)               ← confirms before applying the staged update
│   └── Rollback: (action)            ← confirms before switching back to the previous release
├── Sound (group)                     ← merged: Audio + Sound controls
│   ├── Master Vol: [0..100] step 1  default 73
│   ├── Note Length: [30..2000] step 10 ms  default 120
│   ├── Velocity Scale: [0..200] step 5 %   default 100
│   ├── Velocity Curve: [linear | soft | hard]
│   └── Voice Stealing: [fixed12 | fixed16 | auto-soft | auto-balanced | auto-hard | none]  default auto-balanced
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
├── UI (group)
│   ├── Ghost Cells: [on | off]  default off  ← shows dim cells from inactive parts behind active part
│   ├── Numeric Display: [bar | numbers | bar+numbers]  ← controls rendering of bar-style numeric params, default bar+numbers
│   ├── Screen Sleep: [0..600] step 10 s    default 60 (0=off)
│   ├── Display Brightness: [10..100] step 5  default 75 (bar display when Numeric Display is bar or bar+numbers)
│   ├── Grid Brightness: [10..100] step 5     default 75 (bar display when Numeric Display is bar or bar+numbers)
│   └── Button Brightness: [10..100] step 5   default 75 (bar display when Numeric Display is bar or bar+numbers)
└── Shutdown: (action)                ← confirm, then show shutdown splash and exit/poweroff
```

Diagnostics is a pre-hardware Pi check, and the update actions are native placeholders for OTA flow control: `Check` is unconfirmed, while `Apply` and `Rollback` confirm before handing off to the Pi host adapter.

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
- Transport flash: green (beat) or red (measure) border on play icon
- Event dot: briefly shown when notes fire, hidden while a footer toast is active; turns red when recent voice stealing occurred
- Top-right audio load indicator: hidden when idle, yellow when DSP load is moderate or recent voice stealing occurred, red when DSP load is heavy
- Toast text: displayed at bottom for feedback messages

Value editing semantics:

- Number/enum/bool rows enter edit mode on main press
- Browsing selected values are shown on the selected label row; edit mode uses a separate value-focused row for clarity.
- Bool behaves like a 2-option enum (`off`/`on`) and changes on encoder turn, not immediate row press
- Named target selectors (instrument slot, part index, mixer route) display their computed names via `formatDisplayValue()` (e.g. `I1: synth`, `P3: rain`, `fx_bus_2`)
- When `Numeric Display` is `bar` or `bar+numbers`, bounded sound/control/behavior number items render with a smooth geometric bar (filled rectangle) alongside the numeric value
- Bar display applies automatically to FX params, synth/sample shaping controls, mixer volume/pan, touch FX controls, system sound/UI controls, L2 axis controls, and behavior controls such as spawn interval/count, threshold, lifespan, and radius
- Selector-like numeric rows stay plain text, including MIDI channels, instrument/sample slots, part selectors, and MIDI note ranges
- Bar value text uses compact units where useful: `%`, `ms`/`s`, `Hz`, `bpm`, `dB`, semitones/cents, and pan as `L15`/`C`/`R15`; ambiguous internal `0..1` ranges display as `0..100`

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

Brightness is scaled by the Grid Brightness setting.

Overrides:

- While Fn is held: leftmost column shows part selectors (gray) and active part (green).
- While sample assignment mode is active: grid shows assignment overlay (selected-slot colors, other-slot dim white, unassigned dark).
- While any Dance Page (`mix`, `pan`, `fx`, `trigger-gate`, `xy`) is active: grid shows the Dance performance overlay instead of active behavior cells.
- When Ghost Cells is on, inactive parts' active cells render as very dim green behind the active part. Active part cells and sample assignment overlays take priority.
- Active context changes use OLED toast/status feedback, for example `Part: P3 rain` or `Dance: fx`; these toasts do not change LED overlay priority. Modal help/confirm displays keep display priority over context feedback.
- Holding Shift, Fn, or Shift+Fn for more than one second without another mapped action shows a concise hint toast (`Shift: map/edit`, `Fn: parts/pages`, or `Help: Sh+Fn+Enter`). Startup uses the same chord wording: `Help: Sh+Fn+Enter`. Existing toasts, help/confirm dialogs, assignment overlays, and consumed mappings suppress the hint.

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
- When enabled: native menu edits and aux-bound value changes emit deferred `store_save_default` effects; fast audio-facing edits update state/audio immediately and coalesce `ConfigPayload` generation for about 150ms so storage writes the latest settled value instead of saving every intermediate encoder step
- Disabled by default
- Toggling Auto Save on triggers an immediate save when you exit that menu row
- Explicit Save Default is always immediate and cancels any pending deferred default save

## Aux Encoder Binding

- Each aux encoder has two independent custom slots:
  - turn slot: bound to value parameters (number/enum/bool)
  - press slot: bound to actions
- Fn + aux press on a bindable item binds/overwrites the relevant custom slot:
  - while editing a value item: binds Turn slot
  - while selecting an action item: binds `!` press slot
- In the Fn-held aux overlay, plain labels are turn targets and `!Label` entries are press actions; `/` means both slots are present for that encoder.
- Regular aux press triggers the press slot action (if any)
- Regular aux turn adjusts the turn slot value (if any)
- When `Auto Map` is enabled, context-sensitive auto mappings take precedence over custom aux mappings for the active menu context.
- In supported contexts, focused menu rows show auto-map indicators like `1-Cutoff` and `1!Assign`.
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

- Display Brightness scales OLED display intensity in host display adapters.
- Grid Brightness scales matrix LED RGB intensity.
- Button Brightness scales NeoKey button LED intensity.

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

`DeviceInput` supports `grid_press` and `grid_release` events. Behaviors that do not handle `grid_release` (all except `keys`) simply ignore it. The `keys` behavior uses press→activate and release→deactivate semantics.

When a behavior enables immediate input-transition interpretation, `platform-core` interprets grid changes from input through the same Sense/mapping pipeline used during tick, producing immediate musical events. The `keys` behavior uses this to provide immediate finger-drumming response.

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
