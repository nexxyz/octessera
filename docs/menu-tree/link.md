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
│   │   ├── Scan Axis: [rows | columns]           ← columns: a column moving left to right; rows: a row moving bottom to top
│   │   ├── Scan Unit: [1/32T | 1/32 | 1/16T | 1/16 | 1/8T | 1/8 | 1/4T | 1/4 | 1/2T | 1/2 | 1/1T | 1/1]
│   │   ├── Scan Dir: [forward | reverse]
│   │   ├── Sections: [1 | 2 | 4 | 8]
│   │   └── Lanes: [poly | mono]                  default poly
│   ├── Events (group)
│   │   ├── Note Preview: [on | off]              default on; a behavior pick sets it from the behavior's play style
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

- The layer's own steps, direct or scanned, always play through the On, Hold and Off rows; no
  switch gates them, only the rows decide. A Hold event fires at each behavior step for a tile
  that stays lit; play and game behaviors send none. How note-ons press and note-offs release an
  instrument's keys is defined under
  [Trigger types](../menu-and-controls-spec.md#trigger-types).
- `Note Preview` (`layers.N.link.notePreview`, saved with the patch, default on): on, a tile you
  press or release fires its On or Off at once through the rows, scanning or not; off, a press
  only changes the grid, and on a direct layer its next step plays what the press changed, so a
  press that turns a sounding tile off still ends its note. Keys and the Looper are played by
  hand: their presses play at once whatever Note Preview says, unless the layer scans.
- Picking a behavior, from the menu or a binding, sets Scan Mode and Note Preview from the
  behavior's play style: a grid sequencer (Sequencer, Euclid, Polyrhythm, Breaks, Clave, Groove,
  Contour, Cadence) scans with Scan Axis `columns`, the line moving left to right, Note Preview
  on; Keys, Looper and None, and the games Pong and Blockout, play directly with Note Preview off;
  every other behavior plays directly with Note Preview on. Scan Dir starts forward. A layer saved
  without Note Preview loads with what its behavior's pick would set. Scan Mode, Scan Dir and
  Note Preview stay as edited until the next pick; a patch load, the Factory Patch included,
  keeps the saved values. The pick drains the layer like any change below.
- A scanning layer plays the same On, Hold and Off rows, from its scan line instead of its grid's
  changes. Each tile the line reaches is compared with the tile it touched one step before in the
  same lane, the row a column scan runs along or the column a row scan runs along: dark to lit
  fires On, lit to lit Hold, lit to dark Off, dark to dark nothing. The lane loops across the lap
  wrap, and with Sections > 1 each section's lane loops on its own. Hold fires on a scan for
  every behavior. With only On mapped a run of lit tiles sounds once; mapping Hold to `note on`
  as well sounds every lit step. On a Sequencer page turn the new page's lit tiles after lit ones
  fire On instead of Hold, so its first lit column is a downbeat, and the old page's Offs still
  fire.
- `Lanes` (`layers.N.link.scanLanes`, saved with the patch, default `poly`). In `poly` every
  scanned tile plays its own mapped pitch: an Off releases the key of the tile it fires on, so
  with Pitch Steps along the lane notes stack and may keep sounding, as intended. In `mono` each
  lane remembers the pitch and tile of the last note it played on each instrument, its only
  memory: a note-on from the lane first sends a note-off for that note, so the new note replaces
  it, and a note-off from the lane ends that note and its sample pad, not the pitch of the tile
  it fires on. A Hold instrument usually wants `mono`, because a poly Off carries the dark tile's
  pitch, which on a row scan with default Y Steps is not the held note's. A row of `none` does nothing either way. Direct layers are unchanged.
- A change to where the layer's presses and releases go, from the menu or a CC, Aux or Fader
  binding alike, drains the instruments the layer's note-on rows played before the change: a
  row's Inst or Trig, Note Preview, Scan Mode on or off, Scan Axis, Sections, Lanes, the Arp
  turned on or off (Mode leaving or entering `none`) and the trigger gate closing. A drain
  releases every key on those instruments, drops the layer's queued presses (their releases still
  land), lets go of the Arp's keys, and clears the mono lanes and the line's memory, so the next
  step compares with dark tiles. Scan Dir turns the line round where it is and keeps every note;
  Scan Unit only changes the tempo. An Arp Mode or Source change also lets go of the keys the Arp
  holds; its other rows keep them.
- `Steps` on Pitch Steps counts notes of the set (scale degrees), not semitones; negative steps go
  down.
- Event mappings (`activate`, `stable`, `deactivate`) each have Delay and Retrig. Delay counts
  that layer's Link ticks, which on a scanning layer are its scan steps (`Scan Unit`): an On with
  Delay 2 and Retrig 1 sounds 2 and 3 scan steps after the line reaches the tile. Retrig adds
  extra repeats after the original, at delay+1, delay+2, and so on. Trigger probability is rolled
  once before scheduling; delayed repeats do not re-roll.
- Within one layer step every release of a key plays before any press of it, whatever layer,
  source or Delay produced them, and the layers' events sound in layer order, whichever layer is
  selected: a tile going dark never silences a note another tile starts on the same step. After
  Delay/Retrig scheduling, the note-ons that sound on one step play one hit per instrument and
  key, or per sound (MIDI channel and note on MIDI out): the loudest wins whole, with its
  velocity, length and tile, and the first of equals in layer order wins a tie. A second note-off
  of the key finds nothing left to end. A held press drops its layer's queued releases of that
  key, so an Off Delay never cuts the tile's next note. A tile's Off on the step its delayed On
  falls due drops that On, so the note never starts; another tile's On of the same key still
  sounds. On a scan, whose Off comes from the lane's next tile, an Off from the same lane drops
  it. So on a scan an On Delay longer than the Off Delay shortens each lit run by the
  difference, and on a Hold instrument a tile lit for one step then never sounds: give the Off
  row the same Delay.
  A Retrig repeats a note-off too, which ends only a key pressed again in between, so on
  an Off row it usually has no effect.
- An Off acts only on its own Off Inst: set Off Inst to the instrument Hold Inst or On Inst plays
  to end those notes.
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
  Play Transpose applies downstream, to the sound only: the key is the note as mapped.
- A mapping change that moves the layer's pitches releases the keys held on the instruments its
  note-on rows play, at its next step or event; see
  [Trigger types](../menu-and-controls-spec.md#trigger-types).

## One-Shot Len

- Stored as `layers.N.link.oneShotLengthMs` (10..2000 ms, default 150) and saved with the patch.
  Every note the layer plays on a `oneshot` instrument lasts this long, MIDI Inst. slots
  included, and its key lets go by itself then; with the instrument's `Note Off` at `cut` a
  `note off` may end it earlier. A `hold`
  instrument ignores it and ends the note at its note-off. Arp notes keep the
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
- The Arp sits between the layer and its instruments like a hardware arpeggiator: while its mode
  is not `none`, the layer's note-ons and note-offs press and release keys on the Arp's own list
  instead of the instruments. The list holds only the keys this layer has down; the Arp never sees
  its own notes, other layers, Keys, Notes In or the Looper. A key stays down until its note-off,
  on a `oneshot` instrument too, so with Off set to `none` it stays on the list.
- Sources: `simultaneous` arpeggiates the keys pressed together, `held` every key down.
  Unsupported sources normalize to `simultaneous`. A batch is every note-on of one kind and
  instrument in one step, even with note-offs between them; the step's releases route before its
  note-ons, so a key released and pressed in one step stays down. `held` replays the list, oldest
  key first, on a batch that presses a key; a batch that only releases keys replays nothing. Two
  tiles on one pitch are one key, so the first note-off lets it up. `held` plays through the
  current Play Transpose: hold C, E and G, transpose +2 and press A, and it plays D, F#, A and B;
  two keys that sound alike play once, at the louder velocity.
  A new batch drops the notes the previous run still had waiting. Stop, a layer trigger gate turned
  off (from any page or menu), a behavior change, a scan setting change, a mapping change that
  moves the layer's pitches and a patch load let go of the list and stop the pattern.
- `rotating` starts each batch one note further along than the batch before.
- `none` keeps the normal Link path. Other modes press the layer's target instruments' keys with
  finite notes lasting `noteLengthMs * gatePct / 100`, never shorter than 10 ms, as ordinary
  presses (a `hold` instrument ends a still-sounding one before striking it again); strum-like
  modes space notes by `Step` Link ticks. Drum and Sampler tiles and MIDI Drum notes keep their
  own length.
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
