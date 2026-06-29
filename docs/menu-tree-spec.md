# Menu Tree Spec

This file is part of the authoritative menu/control spec rooted at `menu-and-controls-spec.md`. Keep it in sync with native menu tree changes.

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
│   ├── Behavior: [none | life | sequencer | keys | looper | brain | ant | bounce | shapes | raindrops | dla | glider] ← controls which algorithm runs this part
│   ├── Step Rate: [1/16, 1/8, 1/4, 1/2, 1/1]   ← controls how often onTick() is called; hidden when Behavior is `none`
│   ├── ... per-behavior dynamic config from behavior's configMenu()
│   ├── Save Grid State: [on | off]              ← controls whether this part's current grid/runtime state is stored in preset/default saves
│   ├── Auto Label: [on | off]                   ← on: label auto-derives from behavior ID; off: label is manual text
│   └── Part Label: (text, max 32)               ← display label; editing sets Auto Label off
├── P2: ... (group)
└── P3: ... (group)                              ← up to partCount parts total
```

When Auto Label is on, the part label is derived from the active behavior ID (e.g. `life`, `brain`). Editing the Part Label text field switches Auto Label off.
Part selectors (Fn+column selection, L2 Sense Part selector) display the computed part label (e.g. `P1: life`, `P2: rain`).
When a part's behavior is `none`, the L1 part group shows Behavior, Auto Label, and Part Label only; Step Rate, dynamic behavior config rows, and Reset are hidden without deleting stored values.
Parameter target pickers mirror the main menu root order (`L1: Life`, `L2: Sense`, `L3: Voice`, `L4: Dance`, `System`). Within `L1: Life`, behavior `none` parts expose no Behavior targets, while real behavior parts expose `parts.N.algorithmStep` and `parts.N.l1.behaviorConfig.*` targets under their own part label.

Behavior-specific config items (from `configMenu()`):

| Behavior | Config Items | Type/Options |
|---|---|---|
| none | *(none)* | — |
| life | Spawn Count: [0..20] | number, step 1 (default 12) |
| life | Spawn Interval: [1..20] | number, step 1 (default 1) |
| life | !Spawn Random | action, shared route `trigger.life.spawn_now` |
| sequencer | *(none)* | — |
| keys | Quantize: [immediate, step] | enum |
| looper | !Punch In/Out | action |
| looper | Length: [1..64] | number, step 1 (default 16) |
| looper | !Clear Loop | action |
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
│   ├── Instrument 1..8 (group)                ← compact overview label e.g. `I1: synth direct`, `I2: samp fx_bus_1`, `I3: midi ch1`
│   │   ├── Type: [none | synth | sampler | midi]
│   │   ├── Note Mode: [oneshot | hold] default oneshot
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
│   │   ├── Auto Label: [on | off]        ← on: label auto-derives from Type as display text (`Synth`, `Sampler`, `MIDI`); off: label is manual text
│   │   ├── Name: (text, max 32)          ← display label; editing sets Auto Label off; charset includes uppercase, lowercase, digits, space, `_`, `-`
│   │   └── Slot Actions (group)
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
│   │   ├── Auto Label: [on | off]    ← on: label auto-derives from FX slot types as display text (`None`, `Delay+Duck`); off: label is manual text
│   │   └── Name: (text, max 32)      ← display label; editing sets Auto Label off; charset includes uppercase, lowercase, digits, space, `_`, `-`
│   └── ... (per bus)
└── Global FX (group)
    ├── Slot 1..N (group, N from platform capability `globalFxSlotCount`; current desktop/Pi Zero target = 2)
    │   ├── Type: [none | vinyl | eq | compressor | saturator | distortion] default none
    │   └── (effect params, visible per Type)
    └── ...
```

When an instrument Type is `none`, the slot keeps Type, Auto Label, and Name visible and hides Note Mode, engine-specific groups, Mixer, MIDI, and Slot Actions without deleting stored config.

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
- FX bus naming mode: `auto` builds from assigned slot types using display names (e.g. `Delay+Reverb`, or `None` when all slots are empty); manual names are preserved exactly. Legacy raw auto names are normalized only when `Auto Label` is on and the stored name is missing or equals the old raw auto-derived value.

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
- `looper` stores its recorded sequence in `savedState` as step-bucketed press/release events when `Save Grid State` is `on`. Live-held cells and currently sounding playback cells are not saved; loaded loops restart from step 1.
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
- The `Slot` and aux `Turn` target pickers use the same shared menu-mirrored parameter browser as `L4: Dance > X/Y Pad`; no separate parameter tree should diverge from that browser.
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
- `xy` target selection uses the menu-mirrored parameter browser to present all mappable parameters (same set used by aux encoder binding and Sense X/Y axis modulation). Selecting a target stores the parameter key and value metadata per part (`parts[N].xy`).
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
│   │   └── Load Factory: (action)
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
│   └── Voice Limit: [fixed12 | fixed16 | auto-soft | auto-balanced | auto-hard | none]  default auto-balanced
├── MIDI (group)
│   ├── Enabled: [on | off]
│   ├── !Panic: (action)
│   ├── MIDI Out (group)             ← dynamic: one action per detected MIDI output port
│   ├── MIDI In (group)              ← dynamic: one action per detected MIDI input port
│   ├── Sync & Clock (group)
│   │   ├── Sync Mode: [internal | external]
│   │   ├── Clock Out: [on | off]
│   │   ├── Clock In: [on | off]
│   │   └── Follow Start/Stop: [on | off]
├── UI (group)
│   ├── Ghost Cells: [on | off]  default off  ← shows dim cells from inactive parts behind active part
│   ├── Number Style: [bar | numbers | bar+numbers]  ← controls rendering of bar-style numeric params, default bar+numbers
│   ├── Screen Sleep: [0..600] step 10 s    default 60 (0=off)
│   ├── OLED Bright: [10..100] step 5     default 75 (bar display when Number Style is bar or bar+numbers)
│   ├── Grid Bright: [10..100] step 5     default 75 (bar display when Number Style is bar or bar+numbers)
│   └── Button Bright: [10..100] step 5   default 75 (bar display when Number Style is bar or bar+numbers)
├── Controls (group)                  ← read-only shortcut cheat sheet; rows have help but no actions/effects
│   ├── Help: Sh+Fn+Main
│   ├── Back: Back
│   ├── Play/Pause: Space
│   ├── Stop/Sync: Sh+Space
│   ├── Part: Fn+left col
│   ├── Dance: Fn+right col
│   ├── Aux Bind: Fn+Aux
│   ├── Sample: Sh+cell
│   └── Prob Map: grid
└── Shutdown: (action)                ← confirm, then show shutdown splash and exit/poweroff
```

Diagnostics is a pre-hardware Pi check, and the update actions are native placeholders for OTA flow control: `Check` is unconfirmed, while `Apply` and `Rollback` confirm before handing off to the Pi host adapter. Controls rows are native read-only menu rows: main press does not run an action, but context help explains each shortcut. `Stop/Sync: Sh+Space` follows the transport mode: internal sync emergency-stops and clears held notes, while external sync arms resync.
