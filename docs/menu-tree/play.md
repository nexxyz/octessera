# Play Menu Tree

Part of the authoritative menu/control spec; see [`../menu-tree-spec.md`](../menu-tree-spec.md) for
the index and notation. Play turns the grid into a performance surface. Each page swaps the grid's
job: mixer faders, pan, routing and live FX bypass, tempo and swing, the global LFOs, momentary FX, trigger gates, a transpose keyboard, or
an XY pad.

## Tree

```
Play
├── Mix                                  ← page-select row
├── Pan                                  ← page-select row
├── Route                                ← page-select row
├── Tempo                                ← page-select row
├── LFO                                  ← page-select row
├── FX (group)
│   ├── FX Type: [none | stutter | freeze | flt_sweep | pitch]   ← stored as filter_sweep, pitch_shift
│   ├── Target: [master | fx buses | instruments]
│   ├── ... params of the FX Type (see Play FX)
│   ├── Map to Grid (action)
│   └── Aux Map (group)                  ← the Play FX params/actions that auto-map to aux, with their aux markers
├── XY (group)
│   ├── X Axis: <target | (none)> (picker)   ← parameter picker
│   ├── Y Axis: <target | (none)> (picker)
│   ├── Glide: [0..5000] ms (time steps)     ← default 80; 0 shows Off; stored as smoothingMs
│   ├── Invert X: [on | off]
│   ├── Invert Y: [on | off]
│   └── Release: [hold | center]             ← stored as sample-hold, reset-center
├── Transpose                            ← page-select row
├── Rate                                 ← page-select row
└── Trigger Gate                         ← page-select row
```

The menu lists the technical bank, then the musical bank.

## Pages and navigation

- The pages sit in two banks on the right grid column, bottom row first:

  | Row | Fn: technical | Shift+Fn: musical |
  |---|---|---|
  | 0 | `mix` | `fx` |
  | 1 | `pan` | `xy` |
  | 2 | `route` | `transpose` |
  | 3 | `tempo` | `rate` |
  | 4 | `lfo` | `trigger-gate` |

  Other rows are free and do nothing. Fn or Shift+Fn + right column always selects and activates
  that bank's page, moves the menu to its Play row, and toasts `Play: <page>`
  (`Play: trig` for Trigger Gate).
- In the Play menu, `Mix`, `Pan`, `Route`, `Tempo`, `LFO`, `Transpose`, `Rate`, and `Trigger Gate` are page-select
  rows: Main press
  selects and activates the page without entering an empty submenu. `FX` and `XY` are
  enterable groups and activate their page too.
- Entering `Play` activates the saved page. Back, Fn + left column (layer select), and entering
  Build, Link, Shape, or System leave the Play overlay without changing the saved page. Layer
  selection does not move the menu.
- Grid releases on a Play page are consumed and never reach the active behavior.
- Aux bindings keep their targets; switching pages does not alter them.
- Every page dims the behavior tiles to a quarter and draws its markers on top. Fn
  navigation LEDs and overlay priority are defined in
  [Overlays and priority](../menu-and-controls-spec.md#overlays-and-priority).

## Mix

- Each column is an instrument. Row `y=0` toggles that slot's mute; rows `1..7` set its
  `Mixer > Volume` to `round(y / 7 × 100)`: row 1 is 14%, row 7 is 100%. Play Mix cannot set
  0%; the menu, Aux or modulation can. Type `none` slots ignore presses. Mix and Pan presses go
  through the same edit as turning the menu row, so the row follows and an LFO or XY on the value
  wobbles around the new setting.
- Mute is live only: it is never saved and never marks the patch dirty. Boot, patch/preset loads
  and Clear All start unmuted. Toasts read `I3 muted` / `I3 unmuted`.
- A muted internal instrument keeps its stored volume; the engine receives volume 0 from every
  volume path (Play Mix, menu, Aux, LFO/XY modulation, slot changes, full config syncs) until it
  is unmuted, which re-sends the effective volume. Volume rows still change the stored volume
  while muted. Held internal notes and one-shots keep running silently, so an unmute mid-note
  brings their tails back, like a mixer channel mute.
- A muted MIDI slot sends no new note-ons on its MIDI channel; muting sends exact note-offs for its
  held external notes, and unmuting does not revive them. Note-offs still pass, so a key released
  while muted may send a harmless extra note-off. MIDI slots sharing a channel mute together.
- LEDs: row 0 is green when active and red when muted. The volume marker sits at its rounded row
  clamped to `1..7` (low volumes show on row 1): green, or dimmed green while muted. `none` slots
  show dim gray on row 0 and at the marker.
- Holding Shift switches the page to buses and master until Shift is released. Columns `0..3` are
  B1..B4 in the Pan page's bus colors (magenta, blue, green, yellow): rows `1..7` set the saved bus
  `Volume` on the instrument scale, and row 0 toggles a live-only bus mute (green active, red
  muted; `B2 muted` / `B2 unmuted`) with the same rules as instrument mutes: never saved, cleared on
  boot, patch loads and Clear All, and the engine gets volume 0 for a muted bus from every bus
  volume path while the stored volume keeps changing. A muted bus's marker dims. Column 5 is the
  master (white): rows `1..7` set `System > Master Vol` through its menu edit; it has no mute, so
  its row 0 is dark. Columns 4, 6 and 7 are dark. Shift + press here never reaches the behavior or
  the param-mod lanes.

## Pan

- Each row is an instrument: `x=0` is hard left, `x=7` hard right. The stored scale is 33 positions
  (`0..32`, center `16`), shared with the menu and the audio engine.
- The eight columns map onto seven two-tile marker positions: column 0 stores `0` and lights 0+1;
  1 stores `5` (1+2); 2 stores `11` (2+3); 3 and 4 both store center `16` (3+4); 5 stores `21`
  (4+5); 6 stores `27` (5+6); 7 stores `32` (6+7).
- Pan writes the audible target. Direct instruments set `Mixer > Pan Pos`. Bus-routed instruments
  set the bus `Pan Pos` and also store their own pan to preserve it. Several instruments on one bus
  show synchronized markers at the bus pan.
- Marker colors follow the route: white for direct, magenta/blue/green/yellow for buses 1-4. `none`
  slots show a dimmed marker.

## Route

- Columns are instruments I1..I8 unless noted.
- Rows `0..4` set `Mixer > Route`: direct (white), B1 (magenta), B2 (blue), B3 (green), B4
  (yellow), the Pan page's bus colors. It is the menu route edit without moving the menu: the
  saved route changes, the patch is dirty, and the engine gets the new routing. Toast `I2 B1` or
  `I2 direct`. The current route is bright and the others dim; MIDI and `none` slots are dark and
  ignore presses.
- Row 5 is FX bypass, live only: columns `0..3` bypass the B1..B4 slot chains, columns `4..5` the
  Global FX slots 1 and 2; columns `6..7` are dark. Lit is effects on, dim is bypassed (bus colors,
  white for Global FX). A bypassed chain reaches the engine with every slot `none`; the saved
  slots are untouched. Toast `B1 FX bypassed` / `B1 FX on`, `G1 FX bypassed` / `G1 FX on`.
- Row 6 is empty.
- Row 7 is the duck key, live only: pressing an instrument makes every bus duck listen to it
  (`Duck key: I2`); its own `Source Tap` stays. Another column moves the key; pressing the lit
  tile again returns every duck to its saved Source (`Duck key off`). The key is red, other
  audio instruments dim red; MIDI and `none` slots are dark. A key whose slot has since become
  MIDI or `none` stays red, and pressing it clears the key.
- Bypass and duck key are never saved, never mark the patch dirty, and clear on boot, patch or
  preset loads, and Clear All. One gate on the engine command path applies them to every bus
  slot, Global FX slot and full configuration the engine receives, so menu, Aux and LFO edits made
  meanwhile land in the saved config and reach the engine as soon as the override is undone.
  Toggling one resends the bus and Global FX slots, which restarts their effect tails.

## Tempo

- Rows top to bottom:

  | Row | Columns 0..7 |
  |---|---|
  | 7 | Tap, then 50 55 60 65 70 75 80 BPM |
  | 6 | 85 .. 120 BPM in steps of 5 |
  | 5 | 125 .. 160 |
  | 4 | 165 .. 200 |
  | 3 | 205 .. 240 |
  | 2 | nudge -4 -3 -2 -1 +1 +2 +3 +4 BPM |
  | 1 | Swing A: 1 2 3 4 4 5 6 7 |
  | 0 | Swing B: 1 2 3 4 4 5 6 7 |

- A tempo tile or nudge sets `Link > BPM` through the same path as the menu edit, at once and
  phase-continuous: the clock keeps its position and runs on at the new rate. BPM stays in
  `40..240`. Toast: `Tempo 120`.
- Tap: the BPM is the average beat of the last four taps, set from the second tap on and clamped
  to `40..240` (`Tap 117`); the first tap only toasts `Tap`. A gap over 2 s starts over.
- Swing rows set `Link > Swing A` and `Swing B` with Pan-style two-tile markers: value `n` lights
  columns `n-1` and `n`, so `4` lights the middle pair and both middle columns set `4`. Toast
  `Swing 4:5`. Equal values are straight.
- Under external sync the tempo, nudge and Tap tiles are dim and a press toasts `External sync`;
  Swing still works.
- LEDs: the exact BPM tile green, otherwise the nearest tile yellow; other tempo tiles dim gray;
  nudges dim blue (down) and dim magenta (up); swing markers white on dim gray. While playing, the
  Tap tile pulses with the Play key's beat flash: bright green on the downbeat, half green on
  other beats, dim green between them and while stopped.

## LFO

- Rows are the global LFOs `Link > LFOs > L1..L8`, L1 on the bottom row. Every press goes through
  the menu edit of that LFO, so it is saved with the patch.
- Column 0 switches `Enabled` (`L3 on` / `L3 off`). An LFO without a target cannot be switched on:
  its tile is dark and a press toasts `L3: no target`.
- Columns `1..7` set `Period` to `1/8`, `1/4`, `1/2`, `1/1`, `2/1`, `4/1`, `8/1`. Pressing the lit
  period again toggles its triplet (`1/4` and `1/4T`). Toast `L3 period 1/4T`. The phase is scaled
  to the new period, so the wave keeps its position.
- Shift + columns `1..7` set `Depth %` to 15, 29, 43, 57, 71, 86, 100 (`L3 depth 43%`).
- LEDs: column 0 is green while enabled, brightening and darkening with the LFO's current value
  (updated with the playing display), dim gray while off, dark without a target. Period columns:
  the current straight period green, a triplet blue, the others dim green. While Shift is held the
  columns show depth instead: magenta up to the step that holds the current depth, dim magenta
  above.

## Play FX

- Choose `FX Type`, edit its rows, then `Map to Grid` and press a tile. The toast is
  `Map FX: <type>`, and Back leaves assignment without changing stored tiles. The tile stores the
  type, `Target`, and current parameter values. Mapping `none` clears a tile.
- `Target` defaults to `master`. Options list `master`, then the FX buses, then the instruments.
  Insertion points: `instrument_n` on the instrument's outgoing signal before routing/pan; `fx_bus_n`
  on the bus output after its slot FX; `master` after the final mix. Platform-core resolves grid
  semantics into audio commands; desktop forwards them without interpreting Play meaning; Rust runs
  the DSP.
- On the `fx` page, pressing a mapped tile starts its effect and releasing stops it. At most two
  momentary FX run at once (Play FX concurrency capability), and only one per type. A press that
  would exceed either limit is ignored with `Momentary FX limit reached (2)` or
  `Momentary FX type active (<type>)`.
- LEDs: stutter yellow, freeze blue, filter_sweep green, pitch_shift magenta. Assigned idle tiles are
  bright, active tiles add white, and tiles blocked by a limit are dimmed.
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

- A live override of each layer's trigger mode. It does not edit the saved per-tile probability map.
- Rows are layers in Fn-navigation orientation: bottom row is layer 1.
- Columns `0..2` set that row's layer to `0%` (magenta), `custom` (yellow), or `100%` (green). The
  selected mode is bright, the other two dim.
- Columns `3..4` are an unassigned gap. Bottom-row columns `5..7` are always-bright all-layer
  actions: every layer to `0%`, `custom`, or `100%`.
- Filtering per layer: `zero` blocks all triggers, `full` passes all, `custom` uses the stored
  per-tile probability map with that layer's `Prob Low` and `Prob High`.
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
  apply after mapping and before routing. Sampler and Drum tiles shift only when their
  `Follow Transp.` is on.
- LEDs: left column green (selected and enabled), dim blue (selected, disabled), dim gray
  (unselected), dark (ineligible). Keys: the active offset green, center C white, other offsets dim
  blue.
- Held transposed notes drain with exact routed note-offs when transpose routing is retargeted,
  disabled, stopped, or reset.

## Rate

- Rows are layers in Fn-navigation orientation: bottom row is layer 1.
- Columns `0..7` set that layer's `Build > Step Rate` to `1/32`, `1/16T`, `1/16`, `1/8T`, `1/8`,
  `1/4T`, `1/4`, `1/2`, through the same path as the menu edit, so it is saved with the patch. The
  layer keeps its step phase and the new rate applies from its next step. Toast `L3 rate 1/8`.
- Scanning layers step at their `Link > Lx > Scanning > Scan Unit`, not at Step Rate, so Step Rate
  tiles on a scanning layer change the saved Step Rate without changing when it steps.
- Shift view: while Shift is held the same columns show and set each layer's Scan Unit instead,
  through the same menu edit path, so it is saved and autosaved with the patch. Toast
  `L3 scan 1/8`. Layers whose Scan Mode is `none` are dim; a press on one toasts `L3: not scanning`
  and changes nothing (the page never turns scanning on). Releasing Shift returns to Step Rate.
- Pressing the lit tile of a layer, in either view, arms a resync for it (`L3 resync`) instead of
  changing a value: at the next bar line its step accumulator is set so a step lands on the bar
  and its scan restarts at its first position on that step, so its steps and scan follow the bar
  grid again. Stop and Clear All drop a resync that has not landed yet.
- LEDs: the current value bright, the others dim; straight values green (Step Rate) or white
  (Scan Unit), triplets blue in both views. A layer on a value outside these eight (`1/32T`, `1/2T`,
  `1/1T`, `1/1`) shows no bright tile. `none` layers are dark and ignore presses.

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
- `Glide` (saved as `smoothingMs`) is `Off` at 0, otherwise 1–5000 ms on the curved time steps
  from [Numeric steps](../menu-and-controls-spec.md#numeric-steps). Raw markers follow the press immediately, while
  each mapped normalized axis glides independently from its current value without overshoot, before
  target range/step conversion. Elapsed time is monotonic. `sample-hold` lets an active glide finish
  after release; `reset-center` moves the marker to center at once and glides mapped axes to 0.5.
- Shift + press latches that position: after release the marker and the axes stay there even with
  `reset-center` (with `sample-hold` it behaves like the normal hold), and the glide runs as usual.
  A plain press clears the latch and behaves as above. The latch is live only: never saved, never
  marks the patch dirty, and clears on boot, patch loads and Clear All. When a patch load clears
  it under `reset-center`, the marker and the axes return to the center.
- LEDs: the touched tile is bright white while pressed, and stays bright white while latched.
  After release, `sample-hold` leaves a dim gray marker at the held value and `reset-center` a
  dimmer gray marker at center.

## Persistence

- Saved with presets and the default patch: the selected Play page, BPM and swing, the FX page
  config and tile
  assignments, instrument volumes and pans, per-layer trigger mode, `Prob Low`/`Prob High`, the
  probability map, the global X/Y bindings, X/Y glide, invert flags, and release behavior.
- Not saved: the active Play overlay on load/startup, the XY latch, Mix instrument and bus mutes, Route FX bypass
  and duck key, Tap history, Transpose
  selections/enabled state/offsets, the
  live X/Y touch (`playXyTouch`), active momentary FX, assign modes, held modifiers, and other temporary
  overlays.
