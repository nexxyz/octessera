# Link Menu Tree

Part of the authoritative menu/control spec; see [`../menu-tree-spec.md`](../menu-tree-spec.md) for
the index and notation. Link turns what a layer's world does into notes: which events reach which
instrument, when, at what pitch, and how the grid position bends other parameters.

## Tree

```
Link
├── BPM: [40..240] step 1  default 120
├── Swing: [0..75] step 1 %  default 0
├── Aux Mappings (group)
│   ├── Aux 1 (group)
│   │   ├── Trn: <target | (none)> (group)       ← parameter picker; no Range Min/Range Max rows
│   │   ├── Clk: <mapped | (none)> (group)       ← click action picker
│   │   ├── S+Trn: <target | (none)> (group)     ← shifted bank
│   │   └── S+Clk: <mapped | (none)> (group)     ← shifted bank
│   ├── Aux 2 (group)
│   └── Aux 3 (group)
├── LFOs (group)                                  ← exactly eight global slots
│   ├── L1 (group)
│   │   ├── Enabled: [on | off]                   default off
│   │   ├── Target: <target | (none)> (group)     ← numeric, live-safe targets only
│   │   ├── Period: [1/32T | 1/32 | 1/16T | 1/16 | 1/8T | 1/8 | 1/4T | 1/4 | 1/2T | 1/2 | 1/1T | 1/1]  default 1/1
│   │   └── Depth %: [0..100] step 1              default 100
│   └── L2 .. L8 (group)
├── Paused Events: [on | off]                     default on
├── L1: <label> (group)                           ← one group per layer; entering it makes that layer active
│   ├── Scanning (group)
│   │   ├── Scan Mode: [none | scanning]
│   │   ├── Scan Axis: [rows | columns]           ← the rows below show only while scanning
│   │   ├── Scan Unit: [1/32T | 1/32 | 1/16T | 1/16 | 1/8T | 1/8 | 1/4T | 1/4 | 1/2T | 1/2 | 1/1T | 1/1]
│   │   ├── Scan Dir: [forward | reverse]
│   │   ├── Sections: [1 | 2 | 4 | 8]
│   │   ├── Instrument: [none | I1: <name> .. I8: <name>]
│   │   ├── Action: [none | note_on | note_off]
│   │   ├── Scan Delay: [0..16]
│   │   ├── Scan Retrig: [0..8]
│   │   ├── Empty Inst: [none | I1: <name> .. I8: <name>]
│   │   ├── Empty Act: [none | note_on | note_off]
│   │   ├── Empty Delay: [0..16]
│   │   └── Empty Retrig: [0..8]
│   ├── Events (group)
│   │   ├── Event Triggers: [on | off]            default on
│   │   ├── State Notes: [on | off]               default on
│   │   ├── On Inst / On Trig / On Delay / On Retrig          ← activate
│   │   ├── Hold Inst / Hold Trig / Hold Delay / Hold Retrig  ← stable
│   │   └── Off Inst / Off Trig / Off Delay / Off Retrig      ← deactivate
│   │       (Inst: [none | I1: <name> .. I8: <name>]; Trig: [none | note_on | note_off]; Delay: [0..16]; Retrig: [0..8])
│   ├── Trigger Prob. (group)
│   │   ├── Mode: [zero | custom | full]          default full
│   │   ├── Prob Low: [0..100] step 1             default 0
│   │   ├── Prob High: [0..100] step 1            default 100
│   │   └── Map Prob Grid (action)
│   ├── Note Mapping (group)
│   │   ├── Low Note: [0..127] step 1             default 24; shown as note name + MIDI number, e.g. C1 (24)
│   │   ├── High Note: [0..127] step 1            default 84
│   │   ├── 1st Note: [0..127] step 1             default 60
│   │   ├── Set: <compact> > (bindable enum)      default chromatic; Main opens the foldered selector
│   │   │   ├── Scales (group)                    ← `..` first, then: Chromatic | Major | Natural Minor | Harmonic Minor | Dorian | Mixolydian
│   │   │   ├── Chord Tones (group)               ← `..` first, then: Major Triad | Minor Triad | Diminished Triad | Sus 2 | Sus 4 | Major 7 | Dominant 7 | Minor 7 | Major 9 | Dominant 9
│   │   │   ├── Pentatonic (group)                ← `..` first, then: Major Pentatonic | Minor Pentatonic | Suspended Penta | Hirajoshi-like | In Sen-like | Iwato-like
│   │   │   └── Symmetric (group)                 ← `..` first, then: Whole Tone | Octatonic H-W | Octatonic W-H
│   │   ├── Root: [C | C# | D | D# | E | F | F# | G | G# | A | A# | B]  default C
│   │   └── Out of Range: [clamp | wrap]          default wrap
│   ├── X Axis (group)
│   │   ├── Slot 1: <target | (none)> (group)     ← parameter picker with Range Min/Range Max for numeric bindings
│   │   ├── Slot 1 Invert: [on | off]
│   │   ├── Slot 2: <target | (none)> (group)
│   │   ├── Slot 2 Invert: [on | off]
│   │   ├── Pitch Steps (group)
│   │   │   ├── Enabled: [on | off]               default on
│   │   │   ├── Steps: [-16..16] step 1           ← while enabled; default 1 (X), 3 (Y)
│   │   │   └── Restart Sect: [on | off]          ← while enabled
│   │   ├── Velocity (group)                      ← default From 1, To 127
│   │   ├── Filter Cutoff (group)                 ← default From 20, To 127
│   │   └── Filter Res (group)                    ← default From 10, To 90
│   │       (each lane: Enabled [on | off] default off; while enabled: From [0..127], To [0..127], Grid Offs [-7..7], Curve [linear | curve])
│   ├── Y Axis (group)                            ← same structure; picker targets param:N:y:slot, config keys y.*
│   ├── Arp (group)
│   │   ├── Mode: [none | direct | up | down | bounce | outside_in | rotating | random | octave_spread | chord_strike | strum]  default none
│   │   ├── Source: [simultaneous | held]         default simultaneous
│   │   ├── Step: [1..16] step 1                  default 1
│   │   ├── Length ms: [10..2000]                 default 120
│   │   ├── Gate %: [1..100] step 1               default 80
│   │   └── Octaves: [0..3] step 1                default 0
│   └── Seeded: [on | off]                        default off
├── L2: <label> (group)
└── ...                                           ← layerCount groups in total
```

The flat `Set` IDs used for binding and modulation are, in order: `chromatic`, `major`,
`natural_minor`, `harmonic_minor`, `dorian`, `mixolydian`, `major_triad`, `minor_triad`,
`diminished_triad`, `suspended_second`, `suspended_fourth`, `major_seventh`, `dominant_seventh`,
`minor_seventh`, `major_ninth`, `dominant_ninth`, `major_pentatonic`, `minor_pentatonic`,
`suspended_pentatonic`, `hirajoshi_like`, `in_sen_like`, `iwato_like`, `whole_tone`,
`octatonic_half_whole`, `octatonic_whole_half`.

## Transport and timing

- `Swing` is a global groove amount. `0%` is straight. Swing delays internal off-beat step/scan
  progression and catches up before the next beat; external MIDI clock out stays straight.
- Musical timing selectors use the 24 PPQN vocabulary `1/32T`, `1/32`, `1/16T`, `1/16`, `1/8T`,
  `1/8`, `1/4T`, `1/4`, `1/2T`, `1/2`, `1/1T`, `1/1`. Straight `1/64` is left out because it would
  be 1.5 pulses at 24 PPQN; triplet values are exact. Invalid labels show the canonical `1/8`
  (12-pulse) default in selectors; delay metadata still normalizes invalid labels to the nearest
  note derived from `timeMs`.
- `Paused Events` (`inputEventsWhilePaused`) controls whether direct grid input can emit musical
  events while transport is stopped or paused. Behavior ticks stay stopped either way.

## Events and scanning

- Event mappings (`activate`, `stable`, `deactivate`, `scanned`, `scanned_empty`) each have Delay
  and Retrig. Delay counts that layer's Link ticks. Retrig adds extra repeats after the original,
  at delay+1, delay+2, and so on. Trigger probability is rolled once before scheduling; delayed
  repeats do not re-roll.
- Link collapses only same-tick NoteOn duplicates within one CellTriggerKind group and mapping
  context, after final instrument routing and before Delay/Retrig scheduling. It keeps the first
  event and duration and uses the highest velocity. Other event kinds, lanes, ticks, layers,
  mappings, and durations stay distinct.
- Scanning lanes are defined in
  [Sectioned scanning](../menu-and-controls-spec.md#sectioned-scanning).

## Note mapping

- `Set` is the bindable parent at `layers.N.link.pitch.scale`. Main enters its four folders; Main on
  a folder opens a `..`-first leaf list, and Back or Main on `..` goes up one level. The parent
  renders `Set: <compact> >` within 19 columns.
- A set constrains the concrete notes in `Low Note..High Note` that match the set and `Root`. Sets
  do not emit simultaneous chords or choose inversions. Hirajoshi-like, In Sen-like, and Iwato-like
  are common Western 12-TET approximations of scales with varying forms.
- Mapping picks the set note nearest `1st Note` (lower note wins ties) and applies X/Y pitch steps
  before clamp/wrap. `wrap` wraps within that concrete note list, so wrapped notes stay in the set.
  Play Transpose applies downstream.

## X/Y axes

- `Slot 1` and `Slot 2` assign per-layer X/Y modulation targets through the shared parameter picker
  ([Parameter target pickers](../menu-and-controls-spec.md#parameter-target-pickers)). The same
  assignment is available from the grid with Shift held (see
  [Overlays and priority](../menu-and-controls-spec.md#overlays-and-priority)).
- Lane behavior (pitch steps, velocity, CC74/CC71, curves, Grid Offs) is defined under
  [Modulation](../menu-and-controls-spec.md#modulation).

## LFOs

- `Link > LFOs > L1..L8` is the global bank stored as `runtimeConfig.linkLfos`, exactly eight slots,
  independent of the active layer.
- Targets are additive, numeric, and live-safe; exclusive controls and every LFO configuration key
  are excluded. Claims are validated as described under
  [Modulation](../menu-and-controls-spec.md#modulation).
- Phase and live contributions are transient and never serialized. Playback-runtime sums live LFO
  deltas once per affected endpoint, clamps the result to the target's canonical range, and emits
  transient audio commands without changing the persistent config.

## Arp

- Stored as `layers.N.link.arp` with `mode`, `source`, `stepIntervalSteps`, `noteLengthMs`,
  `gatePct`, and `octaveSpread`.
- Sources: `simultaneous` routed note-on batches, or `held` notes tracked by playback-runtime.
  Unsupported sources normalize to `simultaneous`.
- `none` keeps the normal Link path. Other modes emit finite note-ons lasting
  `noteLengthMs * gatePct / 100` and create no held notes; strum-like modes space notes by `Step`
  Link ticks. Matching note-offs update held membership and never cut off arp-owned finite notes.
- Clamps: `stepIntervalSteps` 1..16, `noteLengthMs` 10..2000, `gatePct` 1..100, `octaveSpread`
  0..3.

## Aux Mappings

- Each `Aux N` group holds four pickers, labelled with the current binding: `Trn`, `Clk`, `S+Trn`,
  and `S+Clk` (the last two write the shifted bank).
- `Trn` pickers are the shared parameter picker without `Range Min`/`Range Max`.
- `Clk` pickers list `(none)`, `Current` when bound, a `Behavior` group with the active layer's
  behavior actions (when it has any), a `Sample Assign` group with one row per instrument, and an
  `Actions` group with `Map FX` and `Reset Behavior`.
- Binding rules, auto-map, and toasts are defined under
  [Aux Encoder Binding](../menu-and-controls-spec.md#aux-encoder-binding).
