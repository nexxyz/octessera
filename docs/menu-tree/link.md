# Link Menu Tree

Part of the authoritative menu/control spec; see [`../menu-tree-spec.md`](../menu-tree-spec.md) for
the index and notation. Link turns what a layer's world does into notes: which events reach which
instrument, when, at what pitch, and how the grid position bends other parameters.

## Tree

```
Link
├── BPM: [40..240] step 1  default 120
├── Swing A: [1..7] step 1  default 4
├── Swing B: [1..7] step 1  default 4
├── Aux Mappings (group)
│   ├── Aux 1 (group)
│   │   ├── Trn: <target | (none)> (picker)      ← parameter picker; no Range Min/Range Max rows
│   │   ├── Clk: <action | (none)> (picker)      ← click action picker
│   │   ├── S+Trn: <target | (none)> (picker)    ← shifted bank
│   │   └── S+Clk: <action | (none)> (picker)    ← shifted bank
│   ├── Aux 2 (group)
│   └── Aux 3 (group)
├── LFOs (group)                                  ← exactly eight global slots
│   ├── LFO1 (group)
│   │   ├── Enabled: [on | off]                   default off
│   │   ├── Target: <target | (none)> (picker)    ← numeric, live-safe targets only
│   │   ├── Period: [1/32T | 1/32 | 1/16T | 1/16 | 1/8T | 1/8 | 1/4T | 1/4 | 1/2T | 1/2 | 1/1T | 1/1 | 2/1T | 2/1 | 4/1T | 4/1 | 8/1T | 8/1]  default 1/1
│   │   └── Depth: [0..100] step 1              default 100
│   └── LFO2 .. LFO8 (group)
├── MIDI CC In (group)                            ← exactly eight slots, saved with the patch
│   ├── CC1 (group)
│   │   ├── CC: [0..127] step 1                   default 1 (CC2 listens to 2, .. CC8 to 8)
│   │   ├── Channel: [any | 1 .. 16]              default any
│   │   ├── Learn !                               ← next incoming CC sets CC and Channel
│   │   ├── Target: <target | (none)> (picker)    ← parameter picker with Range Min/Range Max
│   │   └── Invert: [on | off]                    default off; needs a Target
│   └── CC2 .. CC8 (group)
├── Paused Events: [on | off]                     default on
├── L1: <label> (group)                           ← one group per layer; entering it makes that layer active
│   ├── Scanning (group)
│   │   ├── Scan Mode: [none | scanning]          ← the rows below show only while scanning
│   │   ├── Scan Axis: [rows | columns]
│   │   ├── Scan Unit: [1/32T | 1/32 | 1/16T | 1/16 | 1/8T | 1/8 | 1/4T | 1/4 | 1/2T | 1/2 | 1/1T | 1/1]
│   │   ├── Scan Dir: [forward | reverse]
│   │   ├── Sections: [1 | 2 | 4 | 8]
│   │   ├── Instr.: [none | I1: <name> .. I8: <name>]
│   │   ├── Action: [none | note on | note off]
│   │   ├── Scan Delay: [0..16]
│   │   ├── Scan Retrig: [0..8]
│   │   ├── Empty Inst: [none | I1: <name> .. I8: <name>]
│   │   ├── Empty Act: [none | note on | note off]
│   │   ├── Empty Delay: [0..16]
│   │   └── Empty Retrig: [0..8]
│   ├── Events (group)
│   │   ├── Event Triggers: [on | off]            default on
│   │   ├── State Notes: [on | off]               default on; lets Hold events fire
│   │   ├── On Inst / On Trig / On Delay / On Retrig          ← activate
│   │   ├── Hold Inst / Hold Trig / Hold Delay / Hold Retrig  ← stable
│   │   └── Off Inst / Off Trig / Off Delay / Off Retrig      ← deactivate
│   │       (Inst: [none | I1: <name> .. I8: <name>]; Trig: [none | note on | note off]; Delay: [0..16]; Retrig: [0..8])
│   ├── Trigger Prob. (group)
│   │   ├── Mode: [0% | custom | 100%]           default 100%; stored as zero, custom, full
│   │   ├── Prob Low: [0..100] step 1             default 0
│   │   ├── Prob High: [0..100] step 1            default 100
│   │   ├── Prob Note Off: [on | off]             default off; off lets note-offs past probability and the Trigger Gate
│   │   └── Map Prob Grid (action)
│   ├── Note Mapping (group)
│   │   ├── Low Note: [0..127] step 1             default 24; shown as note name + MIDI number, e.g. C1 (24)
│   │   ├── High Note: [0..127] step 1            default 84
│   │   ├── 1st Note: [0..127] step 1             default 60
│   │   ├── Set: <compact> (picker, bindable enum) default chromatic; Main opens the foldered selector
│   │   │   ├── Scales (group)                    ← `..` first, then: Chromatic | Major | Natural Minor | Harmonic Minor | Dorian | Mixolydian
│   │   │   ├── Chord Tones (group)               ← `..` first, then: Major Triad | Minor Triad | Diminished Triad | Sus 2 | Sus 4 | Major 7 | Dominant 7 | Minor 7 | Major 9 | Dominant 9
│   │   │   ├── Pentatonic (group)                ← `..` first, then: Major Pentatonic | Minor Pentatonic | Suspended Penta | Hirajoshi-like | In Sen-like | Iwato-like
│   │   │   └── Symmetric (group)                 ← `..` first, then: Whole Tone | Octatonic H-W | Octatonic W-H
│   │   ├── Root: [C | C# | D | D# | E | F | F# | G | G# | A | A# | B]  default C
│   │   └── Out of Range: [clamp | wrap]          default wrap
│   ├── One-Shot Len: [10..2000] ms (time steps)  default 150; used by One-Shot instruments only
│   ├── X Axis (group)
│   │   ├── Slot 1: <target | (none)> (picker)    ← parameter picker with Range Min/Range Max for numeric bindings
│   │   ├── Slot 1 Invert: [on | off]
│   │   ├── Slot 2: <target | (none)> (picker)
│   │   ├── Slot 2 Invert: [on | off]
│   │   ├── Pitch Steps (group)
│   │   │   ├── Enabled: [on | off]               default on
│   │   │   ├── Steps: [-16..16] step 1           ← while enabled; default 1 (X), 3 (Y)
│   │   │   └── Restart Section: [on | off]          ← while enabled
│   │   ├── Velocity (group)                      ← default From 1, To 127
│   │   ├── Filter Cutoff (group)                 ← default From 20, To 127
│   │   └── Filter Res (group)                    ← default From 10, To 90
│   │       (each lane: Enabled [on | off] default off; while enabled: From [0..127], To [0..127], Grid Offs [-7..7], Curve [linear | curve])
│   ├── Y Axis (group)                            ← same structure; picker targets param:N:y:slot, config keys y.*
│   ├── Arp (group)
│   │   ├── Mode: [none | direct | up | down | bounce | outside in | rotating | random | octave spread | chord strike | strum]  default none; stored with `_`
│   │   ├── Source: [simultaneous | held]         default simultaneous
│   │   ├── Step: [1..16] step 1                  default 1
│   │   ├── Length: [10..2000]                 default 120
│   │   ├── Gate: [1..100] step 1               default 80
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

- Swing is the global ratio `Swing A : Swing B` of each beat's on-beat eighth to its offbeat
  eighth. Equal values (`4:4`, also `2:2`, `3:3`) are straight. `5:3` puts the offbeat at 5/8 of
  the beat, `4:5` at 4/9. Layer steps and scans run on this warped clock, and a step that falls
  between two 24 PPQN pulses is sent with the earlier pulse plus its exact offset, so the engine
  and MIDI sender place it on time rather than on the pulse grid. Under external sync the next
  pulse time is unknown, so a swung step lands on the nearest incoming MIDI clock pulse at or after
  its exact time. External MIDI clock out stays straight; the beat and bar boundaries never move.
- BPM and Swing are modulation targets (layer X/Y, Play XY, MIDI CC In, LFOs, Faders); see
  [Modulation](../menu-and-controls-spec.md#modulation).
- A swing change, from the menu, the Tempo page or a modulation source, takes effect at the next
  beat: the running beat finishes on the ratio it started with, so a change in the middle of a
  beat never doubles or drops a step.
- Musical timing selectors use the 24 PPQN vocabulary `1/32T`, `1/32`, `1/16T`, `1/16`, `1/8T`,
  `1/8`, `1/4T`, `1/4`, `1/2T`, `1/2`, `1/1T`, `1/1`. Straight `1/64` is left out because it would
  be 1.5 pulses at 24 PPQN; triplet values are exact. Invalid labels show the canonical `1/8`
  (12-pulse) default in selectors; delay metadata still normalizes invalid labels to the nearest
  note derived from `timeMs`.
- `Paused Events` (`inputEventsWhilePaused`) controls whether direct grid input can emit musical
  events while transport is stopped or paused. Behavior ticks stay stopped either way.

## Events and scanning

- `Event Triggers` gates On and Off events, `State Notes` gates Hold events. A Hold event fires at
  each behavior step for a tile that stays lit; play and game behaviors send none. Which tiles
  count as holders of a held note, and how a scan holds a pitch, is defined under
  [Trigger types](../menu-and-controls-spec.md#trigger-types).
- `Steps` on Pitch Steps counts notes of the set (scale degrees), not semitones; negative steps go
  down.
- Event mappings (`activate`, `stable`, `deactivate`, `scanned`, `scanned_empty`) each have Delay
  and Retrig. Delay counts that layer's Link ticks. Retrig adds extra repeats after the original,
  at delay+1, delay+2, and so on. Trigger probability is rolled once before scheduling; delayed
  repeats do not re-roll.
- Link collapses only same-tick NoteOn duplicates within one TileTriggerKind group and mapping
  context, after final instrument routing and before Delay/Retrig scheduling. It keeps the first
  event and duration and uses the highest velocity. Other event kinds, lanes, ticks, layers,
  mappings, and durations stay distinct.
- Scanning lanes are defined in
  [Sectioned scanning](../menu-and-controls-spec.md#sectioned-scanning).

## Note mapping

- `Set` is the bindable parent at `layers.N.link.pitch.scale`. Main enters its four folders on the
  one holding the active set; Main on a folder opens a `..`-first leaf list on the active set, and
  Back or Main on `..` goes up one level. The parent renders `Set: <compact> >` with the set's compact name, so the row fits the OLED in
  both fonts.
- A set constrains the concrete notes in `Low Note..High Note` that match the set and `Root`. Sets
  do not emit simultaneous chords or choose inversions. Hirajoshi-like, In Sen-like, and Iwato-like
  are common Western 12-TET approximations of scales with varying forms.
- Mapping picks the set note nearest `1st Note` (lower note wins ties) and applies X/Y pitch steps
  before clamp/wrap. `wrap` wraps within that concrete note list, so wrapped notes stay in the set.
  Play Transpose applies downstream.

## One-Shot Len

- Stored as `layers.N.link.oneShotLengthMs` (10..2000 ms, default 150) and saved with the patch.
  Every note the layer plays on a `oneshot` instrument lasts this long, MIDI Inst. slots
  included; a `hold` instrument ignores it and ends the note at its note-off. Arp notes keep the
  Arp's own Length, Sampler and Drum hits play their sample or voice, and MIDI Drum its own
  Duration.
- A bindable, Discrete modulation target (`Trn: One-Shot Len`); it is not offered to LFOs.

## X/Y axes

- `Slot 1` and `Slot 2` assign per-layer X/Y modulation targets through the shared parameter picker
  ([Parameter target pickers](../menu-and-controls-spec.md#parameter-target-pickers)). The same
  assignment is available from the grid with Shift held (see
  [Overlays and priority](../menu-and-controls-spec.md#overlays-and-priority)).
- Lane behavior (pitch steps, velocity, CC74/CC71, curves, Grid Offs) is defined under
  [Modulation](../menu-and-controls-spec.md#modulation).

## LFOs

- `Link > LFOs > LFO1..LFO8` is the global bank stored as `runtimeConfig.linkLfos`, exactly eight slots,
  independent of the active layer.
- Targets are additive, numeric, and live-safe; exclusive controls and every LFO configuration key
  are excluded. Instrument targets are Volume and Pan, the Synth oscillator levels, detunes and
  pulse widths, Gain, Cutoff and Res; FM Index, Gain, Cutoff and Res; Plucked Gain, Decay,
  Brightness, Cutoff and Res; Drum Gain, Cutoff and Res; and Sampler Gain, Cutoff and Res.
  Those three are the Sampler rows the engine changes as a single live value: Cutoff and Res
  glide on sounding hits, and Gain is read when a hit starts, so each hit takes the LFO's value
  at that moment. Sampler Tune is a whole-semitone setting, so XY, layer axes and CC move it but
  LFOs do not. The envelopes, the filter envelope, Env Amount, Key Tracking and the velocity
  levels reach the engine only through a slot reload, which is too heavy to run at LFO rate.
  `BPM`, `Swing A` and `Swing B` are LFO targets too: the LFO moves the tempo the clock runs at and
  the swing each next beat takes, while the rows and the save keep the set value
  ([Modulation](../menu-and-controls-spec.md#modulation)).
- `Depth` swings the target around its current value: at 100% the wave reaches half the
  target's range each way, clamped to the range. Claims are validated as described under
  [Modulation](../menu-and-controls-spec.md#modulation).
- Periods run from `1/32T` to `8/1` (eight bars, 768 pulses); `2/1T`, `4/1T` and `8/1T` are 128,
  256 and 512 pulses. Changing an LFO's period scales its phase to the new length, so the wave keeps
  its position in the cycle instead of jumping.
- LFOs shorter than `1/8` would only get a few clock pulses per wave (`1/32T` is two, so a sine
  sampled on them is always zero). With the internal clock they also move between pulses, on a
  quarter-pulse grid, whenever the runtime wakes between two pulses (about every 8 ms on the
  boards, so two or three extra steps per pulse at 120 BPM). `1/32T` and `1/32` then swing
  instead of standing still, though a fast wave stays coarse. Under external MIDI clock they move
  once per incoming pulse.
- An LFO's phase keeps running while it is off, so switching it back on (from the menu or the LFO
  play page) picks the wave up where a never-switched-off LFO would be, in phrase with the music.
  While off it contributes nothing.
- Phase and live contributions are transient and never serialized. Playback-runtime sums live LFO
  deltas once per affected endpoint, clamps the result to the target's canonical range, and emits
  transient audio commands without changing the persistent config.

## MIDI CC In

- `Link > MIDI CC In > CC1..CC8` is stored with the patch as `runtimeConfig.midiCcMappings`, exactly
  eight slots of `{ "cc": 0..127, "channel": 0..16, "target": <binding | null> }`. Channel `0` is
  `any`; the target binding carries `userMin`/`userMax` (Range Min/Max) and `invert`.
- Each slot is a modulation source after the LFOs. A Control Change whose number matches, on the
  slot's channel or any channel for `any`, sets the source to its share of the target's range
  (`1 - share` when inverted): `0` is the bottom, `64` exactly the middle, so a bipolar target
  reaches 0, and `127` the top. On an option or on/off target the CC keeps its option until it
  goes a quarter option past the middle to the next, so a knob resting on a boundary does not
  flicker. The value holds until the next CC; changing Range
  or Invert reshapes the held value at once, a new Target waits for the next CC. Claims are
  validated as described under [Modulation](../menu-and-controls-spec.md#modulation).
- CC only applies while `System > MIDI > MIDI Active` is on, from the port `MIDI In` (or USB MIDI)
  connects. A loaded patch starts with no CC input: targets keep their saved values until a CC
  arrives.
- `Learn` takes the number and channel of the next incoming CC (`CC1: learned CC 74 ch 1`). Back or
  10 s without a CC cancels it (`Learn canceled`); with MIDI Active off it only shows
  `Learn: MIDI is off`.
- CC 121 (Reset All Controllers) is never a source and Learn ignores it. It drops the held value of
  every slot listening on its channel (`any` slots hear every channel), so those targets return to
  their patch values like after Stop; other channels' slots keep theirs.
- Hosts pass Control Change (`0xB0`-`0xBF`) next to clock, Start, Continue and Stop; notes and other
  messages stay out. A drain handles its realtime bytes first, then applies only the latest value
  per channel and CC number.

## Arp

- Stored as `layers.N.link.arp` with `mode`, `source`, `stepIntervalSteps`, `noteLengthMs`,
  `gatePct`, and `octaveSpread`.
- Sources: `simultaneous` routed note-on batches, or `held` notes tracked by playback-runtime.
  Unsupported sources normalize to `simultaneous`. A batch is every note-on of one kind and
  instrument in one step, even with note-offs between them; the step's releases route before its
  note-ons, so a pitch released and struck in one step stays held. `held` replays the held set on a batch
  that starts a note; a batch that only releases notes replays nothing. `held` holds a `hold`
  instrument's notes from note-on to note-off, and a `oneshot` instrument's notes per tile while
  the tile is lit, from its On until it goes dark, whatever Off Trig or Prob Note Off do; a pitch
  two lit tiles share sounds once and stays held until both are dark. One-shot Hold and scan hits
  are not held. Stop, a layer trigger gate turned off, a behavior change and a patch load clear
  the held set.
- `rotating` starts each batch one note further along than the batch before.
- `none` keeps the normal Link path. Other modes emit finite note-ons lasting
  `noteLengthMs * gatePct / 100`, never shorter than 10 ms, and create no held notes; strum-like
  modes space notes by `Step` Link ticks. Matching note-offs update held membership and never cut off arp-owned finite notes.
- Clamps: `stepIntervalSteps` 1..16, `noteLengthMs` 10..2000, `gatePct` 1..100, `octaveSpread`
  0..3.

## Aux Mappings

- Each `Aux N` group holds four pickers, labelled with the current binding's menu name (for
  example `Trn: One-Shot Len`, `Clk: Map FX`): `Trn`, `Clk`, `S+Trn`, and `S+Clk` (the last two
  write the shifted bank).
- `Trn` pickers are the shared parameter picker without `Range Min`/`Range Max`.
- `Clk` pickers list `(none)`, `Current` when bound, a `Behavior` group with the active layer's
  behavior actions (when it has any), a `Sample Assign` group with one row per instrument, and an
  `Actions` group with `Map FX` and `Reset Behavior`. A behavior action is bound to the layer it
  was picked on and keeps running there after you select another layer.
- Binding rules, auto-map, and toasts are defined under
  [Aux Encoder Binding](../menu-and-controls-spec.md#aux-encoder-binding).
