# Play Menu Tree

This file is part of the canonical split-out menu tree spec. See [`../menu-tree-spec.md`](../menu-tree-spec.md) for the canonical index.

### Play

```
Play
├── Mix
├── Pan
├── FX
│   ├── FX Type, Target, visible params for selected FX Type, Map to Grid
│   └── Aux Map: editable auto-mapped Play FX params/actions, with 1-/1! OLED markers
├── Trigger Gate
├── Transpose
└── XY
    └── X Axis, Y Axis, Invert X, Invert Y, Release
```

Play layer behavior:

- Hold Fn for navigation columns: the leftmost grid column selects the active layer using grid Y directly (`y=0` = layer 0), and the rightmost grid column selects and activates Play pages by row: row 0 = mix, row 1 = pan, row 2 = fx, row 3 = trigger-gate, row 4 = transpose, row 5 = xy. Lower rows are unused. Hold Shift+Fn for the layer column only; pressing a left-column layer toggles that layer's trigger gate without changing the active layer, and the right column is reserved.
- In the Play menu, `Mix`, `Pan`, `Trigger Gate`, and `Transpose` are selectable pages, not submenus. Pressing the main encoder on one selects and activates that Play page without entering an empty child page. `FX` and `XY` remain enterable because they have configuration rows.
- Fn + leftmost grid selection exits the current Play overlay without changing the saved Play Page selection. Menu position is not changed by layer selection.
- When Fn is held, the left grid column shows layer-selection options and the right grid column shows Play page options. The active layer and saved Play page are highlighted; layers whose behavior is not `none` have a dim indicator; `none` layers stay dark. When Shift+Fn is held, only the left layer column is shown. All other cells are dark to make the active shortcut lane unambiguous.
- `mix`: each column is an instrument; y=0 mutes, y=7 sets 100%, intermediate rows quantize per-slot `Mixer > Volume`.
- `mix` LEDs show the current volume marker in green; inactive instruments use dim gray.
- `pan`: each row is an instrument; x=0 is hard left and x=7 is hard right. The marker is two cells wide so center positions are visible as the middle pair. Stored pan is a 33-position stereo scale (`0..32`, center `16`) shared with the menu and audio engine.
- `pan` writes the audible pan target: for `Route=direct` instruments it sets `Mixer > Pan Pos`; for bus-routed (`fx_bus_n`) instruments it sets the bus pan (`Mixer > Buses[n] > Pan Pos`) plus the per-instrument pan for state preservation. The marker color reflects the route: white for direct, and magenta, cyan, green, or yellow for buses 1-4. Multiple instruments on the same bus show synchronized markers at the bus pan position.
- `pan` maps the 8 grid columns onto 7 two-cell marker positions: column 0 stores `0` and lights 0+1; column 1 stores `5` and lights 1+2; column 2 stores `11` and lights 2+3; columns 3 and 4 both store center `16` and light 3+4; column 5 stores `21` and lights 4+5; column 6 stores `27` and lights 5+6; column 7 stores `32` and lights 6+7.
- `fx`: grid cells trigger mapped momentary effects. Press starts the mapped effect and release stops it. At most two momentary FX may be active at once, and only one momentary FX of each type may be active. If the active momentary FX limit is reached or another mapping of the same type is already active, the press is ignored and a toast warns the user.
- `trigger-gate`: this Play page performs live trigger mode overrides for each layer; it does not edit the saved per-cell probability map.
- `transpose`: left column toggles which eligible layers are affected; Shift + left column enables/disables transpose for all eligible layers. Columns 1..7 form a piano layout: white rows 1/3/5, black rows 2/4/6, octaves -1/0/+1, with center C at x=1,y=3 as no-op. Offsets apply transiently to synth and enabled MIDI note events after mapping and before routing; sampler assignment notes are not transposed.
- Stored per-layer trigger probability data lives in `Link > L* > Trigger Prob.`.
- `Map Prob Grid` edits the saved four-state probability map for the selected layer. Cell cycle is `zero -> low -> high -> full -> zero`; `Shift+grid` applies to a row; `Shift+Fn+grid` applies to a column.
- Probability-map editor LEDs: black = `0%`, magenta = `low`, yellow = `high`, green = `100%`.
- `Link > Aux Mappings` exposes root-level menu-based assignment for aux encoder turn and click bindings.
- `Link > Paused Events` controls whether direct grid input can emit musical events while the transport is stopped/paused. Algorithm tick/evolution remains stopped either way.
- `Link > L* > X Axis` and `Y Axis` expose explicit per-layer assignment for X/Y param-mod slots.
- The `Slot` and aux `Turn` target pickers use the same shared menu-mirrored parameter browser as `Play > XY`; no separate parameter tree should diverge from that browser.
- When an X/Y axis already has a numeric binding, its picker shows `Range Min` and `Range Max`. These rows limit the user modulation range without changing the target's real min/max metadata. Missing sides fall back to the target range; equal min/max produces a fixed value.
- Aux `Click` uses a dedicated action browser for click-bindable actions.
- Existing hardware shortcuts remain valid: Shift+grid still assigns X/Y param-mod slots and Fn+aux press provides the alternate aux-binding action for the currently highlighted menu parameter or `!` press action.
- Trigger-gate Play layout uses rows as layers with the same orientation as Fn layer navigation: bottom row = layer 0, top row = highest layer.
- Play columns `0..2` set that row's layer mode: `0%` (magenta), `custom` (yellow), `100%` (green). Selected mode is bright; the other two are dim.
- Play columns `3..4` are an unassigned black gap.
- Bottom-row columns `5..7` are always-bright all-layers actions: set all layers to `0%`, `custom`, or `100%`.
- Trigger filtering resolves per-layer mode as follows: `zero` blocks all triggers, `full` passes all triggers, `custom` uses the stored per-cell probability map with that layer's `Low Prob` and `High Prob` thresholds.
- `Shift+Fn+left-column layer` toggles that layer between `0%` and its previously active trigger mode without rewriting the stored probability map or changing the active layer.
- FX cells are mapped from `Play > FX`: select an `FX Type`, edit its visible parameters, then select `Map to Grid` and press a grid cell. The effect type, target, and current parameter values are stored on that cell. Mapping `none` clears a cell.
- `Play > FX > Aux Map` lists the current Play FX parameters/actions that are auto-mapped to aux controls. Rows are editable but do not change the mapping target. OLED row prefixes use the same `1-` turn and `1!` press markers as the live auto-map indicators.
- Entering FX grid assignment shows a concise `Map FX: ...` toast; Back exits assignment without changing stored cells.
- FX assignments include a `Target` (default `master`). Targets are listed as `master` first, then FX buses, then instruments. Platform-core resolves grid semantics into audio commands; desktop forwards those commands without interpreting Play/grid meaning; Rust applies the realtime DSP.
- Target insertion points: `instrument_n` is applied on the instrument's outgoing signal before routing/pan; `fx_bus_n` is applied on the bus outgoing signal after bus slot FX; `master` is applied after the final mix.
- FX concurrency is fixed by platform capability at 2. When both slots are active, all other assigned FX cells gray out and do not respond until a slot frees. When one slot is active, other mappings of the same FX type gray out and do not respond until that type frees.
- Pressing a second cell with the same effect type replaces the existing active cell of that type and emits a release for the old cell before activating the new one.
- Stutter captures a short audio segment on press and loops it repeatedly; `Rate Hz` sets segment length (longer at lower rates) and `Depth` controls wet mix. An ease-in ramp (~2ms) and loop-wrap crossfade prevent clicks.
- Freeze captures the early sound burst into an infinite reverb tail on press (injection window ~120ms). The tail sustains while held with no new input after the window closes. On release, the tail fades out over `Release Ms` and the effect is then removed. `Mix` controls the wet/dry blend.
- Filter Sweep starts with the filter fully open (~20kHz, no audible effect) and sweeps toward the target lowpass cutoff over `Sweep In` on press. On release, it sweeps back to fully open over `Sweep Out` and removes the effect when complete. `Cutoff` sets the target position between 20kHz (0) and the lowest cutoff (100). `Res` controls resonance.
- FX LED colours are yellow for stutter, cyan for freeze, green for filter_sweep, and magenta for pitch_shift. Assigned inactive cells are bright, active cells add white, and limit-blocked cells are dimmed.
- Grid releases in Play mode are consumed by the Play layer and do not reach the active behavior engine.
- Aux encoder bindings continue to target whichever menu item they were bound to; Play page switching does not alter bindings.
- `xy`: the full 8×8 grid acts as a continuous two-axis modulation surface. Pressing a grid cell normalizes its X,Y coordinates over 0–1 (full width/height, no margin). The normalized position modulates the global targets assigned in `Play > XY > X/Y Axis`. While pressed, the current touch cell is bright white; after release, `sample-hold` leaves a dim gray marker at the held value and `reset-center` returns the dim marker to center.
- `xy` target selection uses the menu-mirrored parameter browser to present all mappable parameters (same set used by aux encoder binding and Link X/Y axis modulation). Selecting a target stores the global parameter key and value metadata under `runtimeConfig.xy`.
- `xy` modulation is a global Play source. Its persistent X/Y assignments are separate from per-layer Link axis assignments; transient XY values are not serialized.
- `xy` grid LEDs: bright white on the touched cell while finger is down; dim gray on sample-hold (when `Release = sample-hold` and finger is lifted); rest of grid is dark.
- `Release: sample-hold` keeps the last modulation values active after lifting the finger. `Release: reset-center` returns X and Y to 0.5 (center) on release.
- `Invert X` / `Invert Y` flip the respective axis: `value = 1 - norm` when enabled, so left becomes max and right becomes min (X axis), or bottom becomes max and top becomes min (Y axis).
- Saved with presets/defaults: selected Play Page, FX page config and assignments, instrument mix volumes, pan positions, per-layer trigger probability mode, low/high thresholds, trigger probability map cell state, global X/Y bindings, X/Y invert flags, and X/Y release behavior.
- Not saved: transient performance state such as the currently active Play overlay on load/startup, Play Transpose selections/enabled state/offsets, the live X/Y touch position (`sparksXyTouch`), active momentary FX instances, assign modes, held modifiers, and other temporary overlays.
