# Shape Menu Tree

Part of the authoritative menu/control spec; see [`../menu-tree-spec.md`](../menu-tree-spec.md) for
the index and notation. Shape is the sound side: eight instrument slots, four FX buses, and the
global FX on the main mix.

## Tree

```
Shape
├── Instruments (group)
│   ├── <overview label> (group)                 ← one per slot, e.g. `I1: synth direct`, `I2: FM fxb1`, `I3: Plucked direct`, `I4: Drum direct`, `I5: samp fxb1`, `I6: midi ch1`, `I7: none`
│   │   ├── Type: [none | synth | sampler | midi | fm | pluck | drum]   ← fm shows as FM, pluck as Plucked, drum as Drum
│   │   ├── Note Mode: [oneshot | hold]          ← default oneshot; hidden for none and Drum (Drum is always one-shot)
│   │   ├── Synth (group)                        ← type synth
│   │   │   ├── Preset (group) > Load (group)    ← per-slot synth presets, confirmed
│   │   │   ├── Osc 1 (group)                    ← Wave, Octave, Level, Detune, Pulse Width
│   │   │   ├── Osc 2 (group)                    ← same rows
│   │   │   ├── Filter (group)                   ← Type [lowpass | highpass | bandpass | notch], Cutoff, Res, Env Amount, Key Tracking
│   │   │   ├── Volume (group)                   ← Gain, Vel Sens
│   │   │   ├── Amp Env (group)                  ← Attack, Decay, Sustain, Release
│   │   │   └── Filter Env (group)               ← Attack, Decay, Sustain, Release
│   │   ├── FM (group)                           ← type fm
│   │   │   ├── Preset (group) > Load (group)    ← Init | Soft Keys | Bell
│   │   │   ├── Tone (group)                     ← Ratio, Fine cents, Index, Vel to Index, Mod Shape, Mod Mix
│   │   │   ├── Index Env (group)                ← Attack, Decay, Sustain, Release
│   │   │   ├── Filter (group)                   ← Type, Cutoff, Res, Env Amount, Key Tracking
│   │   │   ├── Volume (group)                   ← Gain, Vel Sens
│   │   │   ├── Amp Env (group)
│   │   │   └── Filter Env (group)
│   │   ├── Plucked (group)                      ← type pluck
│   │   │   ├── Preset (group) > Load (group)    ← Init | Nylon | Steel | Muted
│   │   │   ├── String (group)                   ← Decay, Brightness, Pick Pos, Pick Depth, Stiffness, Body Amt, Body Hz
│   │   │   ├── Filter (group)                   ← Type, Cutoff, Res, Env Amount, Key Tracking
│   │   │   ├── Volume (group)                   ← Gain, Vel Sens
│   │   │   ├── Amp Env (group)
│   │   │   └── Filter Env (group)
│   │   ├── Drum (group)                         ← type drum
│   │   │   ├── Kit (group) > Load (group)       ← Default | Tight | Heavy
│   │   │   ├── Voice: [V1: Kick .. V8: Rim]     ← names follow each voice's current Sound
│   │   │   ├── Edit (group)                     ← Sound, Tune, Decay, Tone, Attack, Sweep st, Sweep ms, Noise %, Level % of the selected voice
│   │   │   ├── Cell Tune: [-24..24] st          ← stamp value
│   │   │   ├── Cell Fine: [-100..100] cents     ← stamp value
│   │   │   ├── Cell Level: [Base | High | Medium | Low]  ← stamp value
│   │   │   ├── Assign (action)                  ← stamps voice + Cell values onto grid cells
│   │   │   ├── Preview (action)                 ← selected voice at its own Tune, no cell offset
│   │   │   ├── Vel Levels (group)               ← High 120, Medium 85, Low 45: [1..127]
│   │   │   ├── Follow Transp.: [on | off]       ← default off
│   │   │   ├── Follow Pitch: [on | off]         ← default off
│   │   │   ├── Filter (group)                   ← Type, Cutoff, Res, Env Amount, Key Tracking
│   │   │   └── Volume (group)                   ← Gain, Vel Sens
│   │   ├── Sampler (group)                      ← type sampler
│   │   │   ├── Kit (group) > Load (group)       ← SDB Kit | Synth Kit | Dist Kit
│   │   │   ├── Sample Slot: [1..8]
│   │   │   ├── <loaded sample> (action)         ← opens its folder in the browser; `(empty)` when unset
│   │   │   ├── S<n> Browse (group)              ← browses the sample tree (WAV only)
│   │   │   ├── Cell Tune: [-24..24] st          ← stamp value
│   │   │   ├── Cell Fine: [-100..100] cents     ← stamp value
│   │   │   ├── Cell Level: [Base | High | Medium | Low]  ← stamp value; only while Vel Levels is on
│   │   │   ├── Assign (action)                  ← stamps sample slot + Cell values onto grid cells
│   │   │   ├── Tune: [-24..24] st               ← slot-wide
│   │   │   ├── Follow Transp.: [on | off]       ← default off
│   │   │   ├── Follow Pitch: [on | off]         ← default off
│   │   │   ├── Gain: [0..100]
│   │   │   ├── Base Velocity: [1..127]          ← used by Base-level cells
│   │   │   ├── Vel Levels: [on | off]
│   │   │   ├── Vel Levels (group)               ← High / Medium / Low [1..127]; only while Vel Levels is on
│   │   │   ├── Filter (group)                   ← Type, Cutoff, Res, Env Amount, Key Tracking
│   │   │   ├── Vel Sens: [0..100]
│   │   │   ├── Amp Env (group)                  ← default A 0 / D 0 / S 100 / R 5 ms
│   │   │   └── Filter Env (group)               ← moves Cutoff by Env Amount
│   │   ├── MIDI (group)                         ← type midi
│   │   │   ├── Enabled: [on | off]
│   │   │   ├── Channel: [1..16]
│   │   │   ├── Velocity: [1..127]
│   │   │   └── Duration: [10..2000] ms
│   │   ├── Mixer (group)                        ← internal types only (synth, sampler, fm, pluck, drum)
│   │   │   ├── Route: [direct | fx_bus_1 | fx_bus_2 | fx_bus_3 | fx_bus_4]  default direct
│   │   │   ├── Volume: [0..100]                 default 100
│   │   │   └── Pan Pos: [0..32]                 ← 33-position scale, 16 = center; bus-routed slots show an empty `Pan Pos -- (bus)` row instead
│   │   ├── Auto Label: [on | off]               ← on: label follows Type (`Synth`, `FM`, `Plucked`, `Drum`, `Sampler`, `MIDI`)
│   │   ├── Name: (text, max 32)                 ← editing sets Auto Label off; uppercase, lowercase, digits, space, `_`, `-`
│   │   └── Slot Actions (group)                 ← hidden for none
│   │       ├── Clone (action)                   ← confirm, then copy to the next free slot
│   │       └── Reset (action)                   ← confirm, then factory defaults for this slot
├── FX Buses (group)
│   ├── B<n>: <bus name> (group)                 ← four buses
│   │   ├── Slot 1: <Effect> (group)             ← e.g. `Slot 1: Delay`
│   │   │   ├── Type: [none | tremolo | delay | vibrato | chorus | flanger | filter_lfo | wah | vinyl | eq | compressor | reverb | glitch | auto_pan | duck | saturator | distortion | bitcrusher]
│   │   │   └── ... params of that Type (see FX parameters)
│   │   ├── Slot 2: <Effect> (group)
│   │   ├── Slot 3: <Effect> (group)
│   │   ├── Volume: [0..100]
│   │   ├── Pan Pos: [0..32]                     ← 33-position scale, 16 = center
│   │   ├── Auto Label: [on | off]               ← on: name built from the slot types, e.g. `Delay+Duck`, or `None`
│   │   └── Name: (text, max 32)                 ← editing sets Auto Label off
└── Global FX (group)
    └── Slot 1..N: <Effect> (group)              ← N = globalFxSlotCount (2)
        ├── Type: [none | vinyl | eq | compressor | saturator | distortion]
        └── ... params of that Type
```

- Type `none` keeps Type, Auto Label, and Name and hides everything else without deleting stored
  config.
- Slot group labels name the loaded effect: `None`, `Delay`, `Duck`, `Reverb`, `Tremolo`,
  `Saturator`, `Distortion`, `Bitcrusher`, `Vibrato`, `Chorus`, `Flanger`, `Filter LFO`, `Wah`,
  `Auto Pan`, `Glitch`, `Compressor`, `EQ`, `Vinyl`.
- Bus naming: with Auto Label on, the name is built from the slot display names (`Delay+Reverb`,
  or `None` when every slot is empty); manual names are kept exactly. Legacy raw auto names are
  normalized only when Auto Label is on and the stored name is missing or equals the old raw
  auto-derived value.

## Instruments

### Loading presets and kits

- FM/Plucked/Synth `Preset > Load` and Sampler/Drum `Kit > Load` open a confirmation with Cancel
  selected (`Confirm Synth`, `Confirm FM`, `Confirm Plucked`, `Confirm Sampler`, `Confirm Drum`,
  each asking `Load I<n> <kind> <choice id>?`, for example `Load I2 FM preset soft_keys?`).
  Nothing changes until Confirm.
- A confirmed load changes only that slot and shows which slot, type, and entry loaded. The custom
  name and mixer routing stay. No load plays a preview.
- `System > Saves` remains the separate whole-patch workflow; there is no per-slot kit save manager.

### FM

- A sine carrier and a sine modulator. The top level fits seven rows below the OLED title.
- Tone, in order:
  - Ratio `0.5|1|2|3|4|5|6|8` (default `2`), displayed `1:2|1:1|2:1|3:1|4:1|5:1|6:1|8:1`: the
    modulator-to-carrier pitch relationship.
  - Fine cents `-100..100` (default `0`, shown signed with `c`): offsets the modulator interval,
    not carrier tuning.
  - Index `0..100` (default `50`), mapped to `0..4` radians: adds harmonics.
  - Vel to Index `0..100%` (default `0`): scales harmonic depth, not amplitude or Volume.
  - Mod Shape `0..100%` (default `0`): blends a second harmonic when Index is above 0.
  - Mod Mix `0..100%` (default `0`): adds audible modulator tone even at Index 0 and can reinforce
    the carrier at Ratio 1.
- Tone edits can reshape notes already ringing. Index Env shapes modulation depth independently of
  the amp and filter envelopes.
- Defaults: Index Env Attack 0 ms, Decay 250 ms, Sustain 20%, Release 120 ms; Volume Gain 80%,
  Vel Sens 100%; Amp Env 5/300/70/350; Filter Env 5/120/70/180. Filter uses Synth's control scales,
  saved under the slot's own `fm` block.

### Plucked

- A ringing string colored by Decay, Brightness, picking, stiffness, and body. The top level fits
  six rows below the OLED title.
- String, in order: Decay `100..5000 ms` (default `1500`), Brightness `0..100%` (`65`), Pick Pos
  `5..50%` (`25`), Pick Depth `0..100%` (`65`), Stiffness `0..100%` (`0`), Body Amt `0..100%`
  (`0`), Body Hz `100..2000 Hz` (`500`). The seven rows fit the OLED body.
- Decay and Brightness edits can reshape ringing notes. Pick Pos, Pick Depth, Stiffness, Body Amt,
  and Body Hz apply to the next pluck.
- Pick Depth changes excitation color, not Pick Pos or Brightness. Stiffness shapes wire-like
  inharmonic upper partials; it is not a literal material, weight, or tension model, so measure
  rendered pitch rather than assume it. Body Amt adds coloration outside the string feedback, and
  Body Hz moves that coloration (not the shared filter cutoff); Body Hz does nothing at zero Body
  Amt.
- Defaults: Volume Gain 80%, Vel Sens 100%; Amp Env 0/0/100/900; Filter Env 5/120/70/180. Filter
  uses Synth's scales, saved under the slot's `pluck` block.

### Drum

- Eight one-shot voices and no assigned cells to start with. The thirteen Drum rows scroll in the
  seven-row OLED body down to Volume without covering the footer.
- `Kit > Load` (Default, Tight, Heavy) changes the eight voice sounds/settings, never the stamped
  cells.
- Voice lists `V1: Kick` through `V8: Rim`, each name taken from the voice's current Sound.
- Edit acts on the selected voice: Sound (`kick`, `snare`, `closed_hat`, `open_hat`, `low_tom`,
  `high_tom`, `clap`, `rim`), Tune `-12..12` st, Decay `20..2000 ms`, Tone `0..100%`, Attack
  `0..50 ms`, Sweep st `0..36`, Sweep ms `0..200`, Noise % `0..100`, Level % `0..100`. The nine rows
  scroll in the OLED body.
- Changing Sound resets that voice's style controls; other voices and stamped cells stay. Data
  missing the sweep/noise/level controls uses per-Sound defaults (Sweep st/Sweep ms/Noise %/Level %):
  Kick 24/55/12/100, Snare 3/25/80/100, Closed and Open Hat 0/0/95/100, Low Tom 8/70/15/100, High
  Tom 6/50/15/100, Clap 0/0/95/100, Rim 0/0/35/100.
- Sweep and Noise shape the next hit, not one already ringing. Sweep ms 0 means no pitch excursion
  even with a positive Sweep st. Noise % blends noise against the pitched tone. Level % attenuates
  the selected voice, not the kit-wide Gain.
- Filter and Volume are Drum's own values on Synth's control scales (default Gain 80%, Vel Sens
  100%). There is no Amp Env or Filter Env page.
- Preview plays the selected voice at its own Tune with no cell offset, from the Main encoder,
  without Space or the sample browser.

### Sampler

- Row order: Kit, Sample Slot, the loaded sample, Browse, Cell Tune, Cell Fine, Cell Level (only
  while Vel Levels is on), Assign, Tune, Follow Transp., Follow Pitch, Gain, Base Velocity, the Vel
  Levels switch, the Vel Levels group (only while on), Filter, Vel Sens, Amp Env, Filter Env.
- SDB Kit, Synth Kit, and Dist Kit replace the slot's eight sample paths only. Cell assignments and
  levels, the selected Sample Slot, and the other sample settings stay.
- Tune, Gain, Base Velocity, the Vel Levels velocities, Filter, and the envelopes are slot-wide.
- Samples play neutral by default: the filter starts as a lowpass at the top of Cutoff with Res
  0, which skips the filter entirely. Drum and FM slots start the same way; Synth and Plucked start
  as an unresonant lowpass at 8 kHz. Cutoff's 0..255 spans 20 Hz to 20 kHz and Res runs 0..100 on
  every instrument; a lowpass at 255 or a highpass at 0 with Res 0 does no filter work.
- The browser's sample preview plays the file exactly as recorded, at its own pitch and level,
  straight into the master: none of the slot's sampler settings, mixer, bus FX or route apply.
- Filter Type picks lowpass, highpass, bandpass or notch. Env Amount (-100..100) lets Filter Env
  move Cutoff, and Key Tracking moves Cutoff with the played pitch (Tune plus the cell's tune),
  100 % being an octave per octave. Amp Env shapes each hit; a note-off starts its release. The
  default Amp Env (A 0, D 0, S 100 %, R 5 ms) plays a sample exactly as recorded, and with Env
  Amount and Key Tracking at 0 the filter stays fixed.
- A loaded sample that cannot be found or decoded shows `N/A-<filename>` (the saved path's
  basename) on the loaded-sample row and the matching browser row. The saved path stays the
  replacement action's source and is unchanged in patch/config payloads. Only that slot goes
  silent; the rest of the instrument loads. The host reports affected paths once per audio
  preparation, and the runtime opens a scrollable `Samples missing` popup listing each newly
  unavailable slot as `I<n> <path>`. Slots already shown as `N/A-` do not reopen it.

### Cell stamps (Sampler and Drum)

- The `Cell ...` rows are stamp values, not live sound settings. Assign copies Cell Tune
  (`-24..24` st), Cell Fine (`-100..100` cents), and Cell Level (Base, High, Medium, Low) onto each
  cell it stamps, together with the selected sample slot (Sampler) or voice (Drum). Sampler stamps
  carry a level only while Vel Levels is on.
- Each instrument saves its stamp values with the patch as `instruments[i].cellStamp`.
- Base-level cells play at the trigger velocity. High/Medium/Low cells play the Vel Levels
  velocities, still scaled by velocity shaping.
- Stamped values are offsets, so slot-wide Tune, Gain, Filter, velocity shaping, and their
  modulation bindings still act on every stamped cell.
- Follow Transp. (default off) lets Play Transpose shift this instrument's cells too (see
  [Transpose](play.md#transpose) for which layers transpose).
- Follow Pitch (default off) plays each cell at the incoming Link note relative to C4 (MIDI 60); the
  stamped Cell Tune/Fine then says how far to retune the sample so it sounds right at C4. Play Drums
  presses have no Link note and play the stamped pitch.
- The combined cell pitch is clamped to ±48 semitones.

### Binding targets

Instrument rows offered as modulation, XY, and Aux targets:

- FM: Index, Volume Gain, Filter Cutoff, Filter Res. Their saved labels carry an `FM` prefix and fit
  the OLED's selected `Current:` row.
- Plucked: Decay, Brightness, Pick Pos, Gain, Filter Cutoff, Filter Res. Decay and Brightness adjust
  ringing strings; Pick Pos shapes the next pluck.
- Drum: the slot's Gain, Filter Cutoff, and Res (live), plus each voice's Tune, Decay, Tone, and
  Attack, which change the next hits and never ringing ones.
- Sampler: every Sampler row except Follow Transp., Follow Pitch, Filter Type, Env Amount, Key
  Tracking, Amp Env, Filter Env, and the `Cell ...` rows. The audible targets are Tune, Gain, Base
  Velocity, Vel Sens, the Vel Levels velocities, and Filter Cutoff/Res.
- No instrument offers its `Cell ...` stamp rows, Ratio, Filter Type, Sound, or Voice as targets.
- FM Index, Plucked Decay, and a Drum voice's Tone emit typed controls without reinitializing the
  slot.

## Assignment modes

### Sample assignment

- Enter with `Shape > Instruments > <slot> > Sampler > Assign`. The OLED toasts
  `Assign S<n>: grid`; Back exits without changing mappings.
- One stamp per cell. A plain press stamps that cell, Shift + press its whole world-space row
  (bottom row `y=0`), and combined modifier + press its column, with the same values on every cell.
- If every target cell already holds exactly that stamp, the press clears them and toasts
  `Cleared`. Anything else is overwritten and the toast shows the stamp, for example
  `S1 +7 -25c High` (levels read `Base`, `High`, `Med`, `Low`).
- To change a cell, stamp it again; there is no per-cell editor. Assignments start empty and never
  auto-fill the grid.
- LED colors are listed under
  [Overlays and priority](../menu-and-controls-spec.md#overlays-and-priority).

### Drum assignment

- Drum `Assign` uses the same gestures, clear rule, and toast (prefix `V`, for example
  `V1 +7 -25c High`). Back exits without undoing work.
- Behavior-triggered Drum hits follow the normal Link/probability/timing path. Direct play from the
  grid is the [Play Drums page](play.md#drums).

### Sample browser

- The browser menu carries the slot context (`S1 Browse`). Body rows are browser entries: `..`,
  built-in/user favourites at the sample root, `[folder]` rows, file rows, or `(empty)`, then a blank
  separator row and a current-folder favourite action.
- Before directory entries arrive it shows `(loading...)` instead of `(empty)`. Long highlighted
  names are clipped to the row width and never overlap other display areas.
- Space previews the highlighted WAV through the selected instrument slot (folders and `..` do
  nothing). The favourite action toggles the current folder in `runtimeConfig.sampleFavouriteDirs`.
  Platform built-in favourites are added dynamically and cannot be removed from that config.
- Pi built-in favourites are `Samples` (`/home/pi/samples`) and `SD card`
  (`/home/pi/samples/sd-card/octessera/samples`, with `/home/pi/samples/sd-card` expected to be the
  OLED SD2 `OCTESSERA` mount point). An unmounted SD card shows a clear unavailable message.
  Desktop has a built-in `User data` favourite.
- Assigned sample playback follows the instrument route, pan, volume, bus FX, and master output gain.
  The browser preview skips all of those and joins the master directly, centred, before master FX.
- Runtime audio commands and full audio-config payloads use the same native normalization on desktop
  and Pi. Malformed instrument/FX payloads keep the last good audio state and raise a typed audio
  failure. Sample preview resolves and decodes through the host adapter before entering the selected
  realtime instrument path.

## FX parameters

| Type | Rows, in order |
|---|---|
| `reverb` | Decay, Damp, Mix % |
| `delay` | Mix %, Spread %, Time Mode [ms \| note], Time Note, Time ms, Feedback |
| `tremolo` | Rate Hz, Depth % |
| `vibrato`, `chorus`, `flanger` | Mix %, Rate Hz, Depth ms, Base ms, Feedback |
| `filter_lfo`, `wah` | Rate Hz, Center, Depth %, Q |
| `auto_pan` | Rate Hz, Depth % |
| `vinyl` | Saturation %, Crackle %, Warp Depth %, Mix % |
| `eq` | Low Gain dB, Mid Gain dB, High Gain dB, Mid Freq Hz, Mid Q, Mix % |
| `compressor` | Thresh dB, Ratio, Attack, Release, Makeup dB, Mix % |
| `duck` | Source, Source Tap, Threshold, Amount %, Attack, Release |
| `saturator` | Drive, Mix % |
| `distortion` | Drive, Clip, Mix % |
| `bitcrusher` | Bits, Rate Div, Mix % |
| `glitch` | Chance %, Slice ms, Mix % |

- Selecting a slot Type initializes that effect's parameter defaults immediately. Loaded presets or
  defaults with missing or invalid params are repaired to those defaults.
- Reverb `Decay` is stored as a feedback coefficient (`0..0.995`) and displayed as approximate tail
  time in seconds (for example `3.1s`) in menu rows and Aux toasts.
- Delay: editing `Time Note` switches to note mode and materializes `timeMs` from the current BPM;
  later BPM edits retime note-mode delays. Editing `Time ms` switches to ms mode, which stays manual.
  Runtime/audio commands carry `timeMs` only; `timeMode` and `timeNote` persist as patch metadata
  and are not modulation, Aux, or XY targets. `Spread %` (`0..100`) widens only the final bus
  output; with Mix 0 and Spread 100 there is no widening.
- Duck: `Source` lists `I1..I8`, then the other buses `B<n>` (never the duck's own bus). `Source Tap`
  stores `pre|post` and displays `Pre|Post`; a missing value in a legacy patch behaves as `pre`.
  `Threshold` is `0..1` (shown as `0..100`), `Amount %` `0..100`, `Attack` `1..500 ms`, `Release`
  `1..5000 ms`. These menu, saved-config, and realtime ranges are the same; accepted values go to the
  ducking renderer without a narrower range. Tap points are under
  [Routing semantics](#routing-semantics).
- When bus config is missing, the menu shows the shipped defaults (`Slot 1: Delay`, `Slot 2: Duck`,
  `Slot 3: None`) rather than the first option. Old configs without `slot3` load it as `none`; saved
  configs always include `slot3`.

## Routing semantics

- Instrument `Volume` is a post-voice per-slot fader, also driven by `Play > Mix`.
- `Route=direct` sends post-fader output to the main mix using the instrument's `Pan Pos`.
- `Route=fx_bus_n` sends post-fader output to that bus as an exclusive send. The instrument's own
  pan is not editable there because the bus output pan places it.
- Synth, FM, Plucked, Drum, and Sampler use the same route/pan/bus-FX mixer path. MIDI instruments
  emit external MIDI and never pass through audio FX.
- Bus input and the Slot 1→2→3 chain are mono; a `none` slot passes through. Bus `Volume`, `Pan Pos`,
  and delay `Spread %` apply only at the final bus output stage before summing into the main mix.
- `auto_pan` moves the bus stereo output position after the slot chain.
- Global FX runs `Slot 1..N` in order on the stereo main mix after direct and bus outputs are summed,
  before global momentary FX and `Master Vol`. Global FX stays two stereo slots and is limited to
  `none | vinyl | eq | compressor | saturator | distortion` for Pi Zero 2 W performance.
- Duck `Source Tap`:
  - Instrument source: `Pre` is the raw source before its fader; `Post` applies its fader, before
    instrument-target momentary FX and pan.
  - Bus source: `Pre` is the bus input before its fader and slot chain; `Post` applies that bus
    fader to the same pre-chain input. A bus input already includes routed instruments' faders and
    instrument-target momentary FX.
  - Both exclude the source bus's slot FX, bus-target momentary FX, spread/pan, global FX, and
    master volume.
