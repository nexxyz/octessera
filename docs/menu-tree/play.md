# Play Menu Tree

Part of the authoritative menu/control spec; see [`../menu-tree-spec.md`](../menu-tree-spec.md) for
the index and notation. Play turns the grid into a performance surface. Each page swaps the grid's
job: mixer faders, pan, momentary FX, trigger gates, a transpose keyboard, an XY pad, or drum pads.

## Tree

```
Play
├── Mix                                  ← page-select row
├── Pan                                  ← page-select row
├── FX (group)
│   ├── FX Type: [none | stutter | freeze | filter_sweep | pitch_shift]
│   ├── Target: [master | fx buses | instruments]
│   ├── ... params of the FX Type (see Play FX)
│   ├── Map to Grid (action)
│   └── Aux Map (group)                  ← the Play FX params/actions that auto-map to aux, labelled `<label>: <key>`
├── Trigger Gate                         ← page-select row
├── Transpose                            ← page-select row
├── XY (group)
│   ├── X Axis: <target | (none)> (group)    ← parameter picker
│   ├── Y Axis: <target | (none)> (group)
│   ├── Smoothing: [0..500] step 10 ms       ← default 80; 0 shows Off
│   ├── Invert X: [on | off]
│   ├── Invert Y: [on | off]
│   └── Release: [sample-hold | reset-center]
└── Drums (group)                        ← labelled `No Drum slot`, with no rows, when no Drum slot exists
    └── Slot: [I<n>: Drum ...]           ← existing Drum slots; session-local
```

## Pages and navigation

- Pages by Fn + right column row: 0 `mix`, 1 `pan`, 2 `fx`, 3 `trigger-gate`, 4 `transpose`,
  5 `xy`, 6 `drums`; row 7 does nothing. Fn + right column always selects and activates that page,
  moves the menu to its Play row, and toasts `Play: <page>`.
- In the Play menu, `Mix`, `Pan`, `Trigger Gate`, and `Transpose` are page-select rows: Main press
  selects and activates the page without entering an empty submenu. `FX`, `XY`, and `Drums` are
  enterable groups and activate their page too.
- Entering `Play` activates the saved page. Back, Fn + left column (layer select), and entering
  Build, Link, Shape, or System leave the Play overlay without changing the saved page. Layer
  selection does not move the menu.
- Grid releases on a Play page are consumed and never reach the active behavior.
- Aux bindings keep their targets; switching pages does not alter them.
- Every page except Drums dims the behavior cells to a quarter and draws its markers on top. Fn
  navigation LEDs and overlay priority are defined in
  [Overlays and priority](../menu-and-controls-spec.md#overlays-and-priority).

## Mix

- Each column is an instrument. Pressing row `y` sets that slot's `Mixer > Volume` to
  `round(y / 7 × 100)`: `y=0` mutes, `y=7` is 100%. Type `none` slots ignore presses.
- LEDs: the current volume marker is green; `none` slots show a dim gray marker.

## Pan

- Each row is an instrument: `x=0` is hard left, `x=7` hard right. The stored scale is 33 positions
  (`0..32`, center `16`), shared with the menu and the audio engine.
- The eight columns map onto seven two-cell marker positions: column 0 stores `0` and lights 0+1;
  1 stores `5` (1+2); 2 stores `11` (2+3); 3 and 4 both store center `16` (3+4); 5 stores `21`
  (4+5); 6 stores `27` (5+6); 7 stores `32` (6+7).
- Pan writes the audible target. Direct instruments set `Mixer > Pan Pos`. Bus-routed instruments
  set the bus `Pan Pos` and also store their own pan to preserve it. Several instruments on one bus
  show synchronized markers at the bus pan.
- Marker colors follow the route: white for direct, magenta/blue/green/yellow for buses 1-4. `none`
  slots show a dimmed marker.

## Play FX

- Choose `FX Type`, edit its rows, then `Map to Grid` and press a cell. The toast is
  `Map FX: <type>`, and Back leaves assignment without changing stored cells. The cell stores the
  type, `Target`, and current parameter values. Mapping `none` clears a cell.
- `Target` defaults to `master`. Options list `master`, then the FX buses, then the instruments.
  Insertion points: `instrument_n` on the instrument's outgoing signal before routing/pan; `fx_bus_n`
  on the bus output after its slot FX; `master` after the final mix. Platform-core resolves grid
  semantics into audio commands; desktop forwards them without interpreting Play meaning; Rust runs
  the DSP.
- On the `fx` page, pressing a mapped cell starts its effect and releasing stops it. At most two
  momentary FX run at once (Play FX concurrency capability), and only one per type. A press that
  would exceed either limit is ignored with `Momentary FX limit reached (2)` or
  `Momentary FX type active (<type>)`.
- LEDs: stutter yellow, freeze blue, filter_sweep green, pitch_shift magenta. Assigned idle cells are
  bright, active cells add white, and cells blocked by a limit are dimmed.
- `Aux Map` lists the Play FX params and `Map to Grid` that auto-map to aux. Rows are editable but
  do not change the mapping target; OLED prefixes use the live `1-` turn and `1!` press markers.

| FX Type | Rows | Behavior |
|---|---|---|
| `stutter` | Rate Hz `1..32` (default 8), Depth `0..100` (default 100) | Captures a short segment on press and loops it. Rate sets segment length (longer at lower rates); Depth sets wet mix. A ~2 ms ease-in and a loop-wrap crossfade prevent clicks. |
| `freeze` | Release Ms `10..5000` (default 500), Mix `0..100` (default 100) | Captures the early sound burst (about a 120 ms injection window) into an infinite reverb tail that sustains while held with no new input. On release the tail fades over Release Ms, then the effect is removed. |
| `filter_sweep` | Cutoff `0..100` (default 50), Res `0..100` (default 0), Sweep In `10..3000 ms` (default 120), Sweep Out `10..3000 ms` (default 180) | Starts fully open (about 20 kHz, inaudible) and sweeps to the target lowpass cutoff over Sweep In; on release sweeps back open over Sweep Out and is removed. Cutoff 0 targets about 120 Hz (darkest), 100 about 8 kHz. |
| `pitch_shift` | Semitones `-24..24`, Cents `-100..100`, Slide In `10..3000 ms` (default 120), Slide Out `10..3000 ms` (default 180), Mix `0..100` (default 100) | Pitch glides linearly in octaves. The fixed `PITCH_FILL_FRAMES=1088` live-input fill and the fixed 10 ms wet/dry anti-click envelopes are independent of the slides. |

The `ms` rows use the curved time steps from
[Numeric steps](../menu-and-controls-spec.md#numeric-steps).

## Trigger Gate

- A live override of each layer's trigger mode. It does not edit the saved per-cell probability map.
- Rows are layers in Fn-navigation orientation: bottom row is layer 1.
- Columns `0..2` set that row's layer to `0%` (magenta), `custom` (yellow), or `100%` (green). The
  selected mode is bright, the other two dim.
- Columns `3..4` are an unassigned gap. Bottom-row columns `5..7` are always-bright all-layer
  actions: every layer to `0%`, `custom`, or `100%`.
- Filtering per layer: `zero` blocks all triggers, `full` passes all, `custom` uses the stored
  per-cell probability map with that layer's `Prob Low` and `Prob High`.
- `Shift+Fn` + left-column layer toggles that layer between `0%` and its previous mode without
  rewriting the map or changing the active layer. Note release on disable is described in the
  [Cheat Sheet](../menu-and-controls-spec.md#cheat-sheet).

### Probability map

- Stored per layer under `Link > Lx > Trigger Prob.`. `Map Prob Grid` edits the saved four-state map
  for that layer: each press cycles `zero -> low -> high -> full -> zero`; Shift + press applies to a
  row and Shift+Fn + press to a column.
- LEDs: black `0%`, magenta `low`, yellow `high`, green `100%`.

## Transpose

- The left column toggles which eligible layers transpose; Shift + left column enables or disables
  transpose for all eligible layers (`Transpose all on` / `Transpose all off`).
- Columns 1..7 form a three-octave piano: white keys on rows 1/3/5, black keys on rows 2/4/6, octaves
  -1/0/+1. Center C at `x=1, y=3` is no transpose. A press sets the offset for every selected,
  enabled layer and toasts `Transpose <±n>`.
- A layer is eligible when any of its Link mappings sends `note_on` or `note_off` to a Synth, FM, or
  Plucked slot, an enabled MIDI slot, or a Sampler or Drum slot with `Follow Transp.` on. Offsets
  apply after mapping and before routing. Sampler and Drum cells shift only when their
  `Follow Transp.` is on.
- LEDs: left column green (selected and enabled), dim blue (selected, disabled), dim gray
  (unselected), dark (ineligible). Keys: the active offset green, center C white, other offsets dim
  blue.
- Held transposed notes drain with exact routed note-offs when transpose routing is retargeted,
  disabled, stopped, or reset.

## XY

- The full grid is a continuous two-axis surface. A press normalizes its X,Y over `0..1` (full
  width and height, no margin) and drives the global targets in `Play > XY > X Axis`/`Y Axis`.
- Targets come from the shared parameter picker
  ([Parameter target pickers](../menu-and-controls-spec.md#parameter-target-pickers)) and are stored
  under `runtimeConfig.xy`. XY is a global Play source, separate from per-layer Link axis slots.
- `Release: sample-hold` keeps the last values after lifting; `reset-center` returns X and Y to 0.5.
- `Invert X`/`Invert Y` use `value = 1 - norm`, so left (X) or bottom (Y) becomes max. Inversion
  retargets only the affected mapped axis through the smoothing glide; the visual marker is never
  inverted.
- `Smoothing` is `Off` at 0, otherwise 10–500 ms. Raw markers follow the press immediately, while
  each mapped normalized axis glides independently from its current value without overshoot, before
  target range/step conversion. Elapsed time is monotonic. `sample-hold` lets an active glide finish
  after release; `reset-center` moves the marker to center at once and glides mapped axes to 0.5.
- LEDs: the touched cell is bright white while pressed. After release, `sample-hold` leaves a dim
  gray marker at the held value and `reset-center` a dimmer gray marker at center.

## Drums

- `Slot` selects an existing Drum instrument (`I<n>: Drum`), starting with the first Drum slot for
  the session. With no Drum slot the seventh Play row reads `No Drum slot` and has no selectable
  guidance row.
- On this page a press on an assigned cell plays exactly one hit from the selected slot, even when
  stopped; releases are consumed. Empty cells, or a slot no longer set to Drum, stay silent. These
  hits bypass behavior, Link timing, and probability, and do not trigger the behavior engine.
- LEDs: the grid goes black and the selected slot's assigned cells show dim white.
- Outside this page, mapped Drum hits follow the normal behavior/Link path. Drum assignment is in
  [shape.md](shape.md#drum-assignment).

## Persistence

- Saved with presets and the default patch: the selected Play page, the FX page config and cell
  assignments, instrument volumes and pans, per-layer trigger mode, `Prob Low`/`Prob High`, the
  probability map, the global X/Y bindings, X/Y smoothing, invert flags, and release behavior.
- Not saved: the active Play overlay on load/startup, Transpose selections/enabled state/offsets, the
  live X/Y touch (`playXyTouch`), active momentary FX, assign modes, held modifiers, other temporary
  overlays, and the Play Drums slot choice.
