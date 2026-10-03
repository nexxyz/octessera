# Shape Menu Tree

This file is part of the canonical split-out menu tree spec. See [`../menu-tree-spec.md`](../menu-tree-spec.md) for the canonical index.

### Shape

```
Shape
├── Instruments (group)
│   ├── Instrument 1..8 (group)                ← compact overview label e.g. `I1: synth direct`, `I2: FM direct`, `I3: Plucked direct`, `I4: Drum direct`, `I5: samp fxb1`, `I6: midi ch1`
│   │   ├── Type: [none | synth | sampler | midi | fm | pluck | drum]   ← fm displays as FM; pluck as Plucked; drum as Drum
│   │   ├── Note Mode: [oneshot | hold] default oneshot; hidden for Drum (always oneshot)
│   │   ├── Synth (group, visible when type=synth)
│   │   │   ├── Preset > Load (group)      ← per-slot synth preset load with confirm
│   │   │   ├── Osc 1 (group)              ← Wave, Octave, Level, Detune, Pulse Width
│   │   │   ├── Osc 2 (group)              ← same sub-items
│   │   │   ├── Filter (group)             ← Type, Cutoff, Res, Env Amt, Key Track
│   │   │   ├── Volume (group)             ← Gain, Vel Sens
│   │   │   ├── Amp Env (group)            ← ADSR loudness contour
│   │   │   └── Filter Env (group)         ← ADSR filter contour
│   │   ├── FM (group, visible when type=fm)
│   │   │   ├── Preset > Load (group)      ← Init | Soft Keys | Bell; confirmed FM-only load
│   │   │   ├── Tone (group)               ← Ratio: [0.5 | 1 | 2 | 3 | 4 | 5 | 6 | 8] default 2, displayed as 1:2 | 1:1 | 2:1 ... 8:1; Fine cents -100..100 default 0; Index 0..100 default 50; Vel to Index, Mod Shape, Mod Mix 0..100 default 0
│   │   │   ├── Index Env (group)          ← Attack 0 ms, Decay 250 ms, Sustain 20%, Release 120 ms by default
│   │   │   ├── Filter (group)             ← same Type, Cutoff, Res, Env Amount, Key Tracking controls as Synth
│   │   │   ├── Volume (group)             ← Gain 80%, Vel Sens 100% by default
│   │   │   ├── Amp Env (group)            ← Attack 5 ms, Decay 300 ms, Sustain 70%, Release 350 ms by default
│   │   │   └── Filter Env (group)         ← same ADSR controls and defaults as Synth
│   │   ├── Drum (group, visible when type=drum)
│   │   │   ├── Kit > Load (group)         ← Default | Tight | Heavy; confirmed eight-voice load
│   │   │   ├── Voice: [V1: Kick ... V8: Rim]  ← current Sound supplies each name; 8 kit voices
│   │   │   ├── Edit (group)               ← Sound [kick | snare | closed_hat | open_hat | low_tom | high_tom | clap | rim], Tune -12..12 st, Decay 20..2000 ms, Tone 0..100%, Attack 0..50 ms, Sweep st 0..36, Sweep ms 0..200, Noise % 0..100, Level % 0..100
│   │   │   ├── !Assign                     ← per-cell set/replace/toggle with selected voice
│   │   │   ├── !Cell Tune                  ← assigned cell offset -24..24 st; Back returns/exits
│   │   │   ├── !Preview                    ← base voice pitch, no cell offset
│   │   │   ├── Filter (group)             ← own Type, Cutoff, Res, Env Amount, Key Tracking settings
│   │   │   └── Volume (group)             ← own Gain 80%, Vel Sens 100%; no visible Drum Amp/Filter Env pages
│   │   ├── Plucked (group, visible when type=pluck)
│   │   │   ├── Preset > Load (group)      ← Init | Nylon | Steel | Muted; confirmed Plucked-only load
│   │   │   ├── String (group)             ← Decay 100..5000 ms default 1500; Brightness 0..100% default 65; Pick Pos 5..50% default 25; Pick Depth 0..100% default 65; Stiffness 0..100% default 0; Body Amt 0..100% default 0; Body Hz 100..2000 Hz default 500
│   │   │   ├── Filter (group)             ← same Type, Cutoff, Res, Env Amount, Key Tracking controls as Synth
│   │   │   ├── Volume (group)             ← Gain 80%, Vel Sens 100% by default
│   │   │   ├── Amp Env (group)            ← Attack 0 ms, Decay 0 ms, Sustain 100%, Release 900 ms by default
│   │   │   └── Filter Env (group)         ← same ADSR controls and defaults as Synth
│   │   ├── Sampler (group, visible when type=sampler)
│   │   │   ├── Kit > Load (group)         ← SDB Kit | Synth Kit | Dist Kit; confirmed eight-path load
│   │   │   ├── Sample Slot: [1..8]
│   │   │   ├── Loaded sample (action)     ← opens its folder in the browser; shows (empty) when unset
│   │   │   ├── S* Browse (group)          ← browses `samples/` tree (wav only)
│   │   │   ├── Assign (action)            ← enters grid assignment mode for selected sample slot
│   │   │   ├── Tune: [-24..24] semitones
│   │   │   ├── Gain: [0..100]
│   │   │   ├── Base Velocity: [1..127]    ← used when Vel Levels=off
│   │   │   ├── Vel Levels (group)         ← High / Medium / Low: [1..127] (visible when Vel Levels=on)
│   │   │   ├── Vel Levels: [on | off]
│   │   │   ├── Filter (group)             ← sample filter controls
│   │   │   ├── Vel Sens: [0..100]
│   │   │   ├── Amp Env (group)            ← saved sample amp envelope
│   │   │   └── Filter Env (group)         ← saved sample filter envelope
│   │   ├── Note Settings (group, visible when type=midi)
│   │   │   ├── Velocity: [1..127]
│   │   │   └── Duration: [10..2000] ms
│   │   ├── Mixer (group)
│   │   │   ├── Route: [direct | fx_bus_1..fx_bus_N] default direct (N from platform capabilities)
│   │   │   ├── Volume: [0..100] default 100
│   │   │   └── Pan Pos: [0..32] quantized (33-position stereo scale; 16=center; shown as non-editable bus pan note when Route=fx_bus_n)
│   │   ├── MIDI (group)
│   │   │   ├── Enabled: [on | off]       default off
│   │   │   └── Channel: [1..16]
│   │   ├── Auto Label: [on | off]        ← on: label auto-derives from Type as display text (`Synth`, `FM`, `Plucked`, `Drum`, `Sampler`, `MIDI`); off: label is manual text
│   │   ├── Name: (text, max 32)          ← display label; editing sets Auto Label off; charset includes uppercase, lowercase, digits, space, `_`, `-`
│   │   └── Slot Actions (group)
│   │       ├── !Clone (action)           ← duplicates instrument config to next free slot, with confirmation
│   │       └── !Reset (action)           ← resets instrument to factory defaults, with confirmation
├── FX Buses (group)
│   ├── Bus 1..4 (group)
│   │   ├── Slot 1: Effect (group)
│   │   │   ├── Type: [none | reverb | delay | tremolo | chorus | flanger | vibrato | auto_pan | filter_lfo | wah | vinyl | eq | compressor | duck | saturator | distortion | bitcrusher | glitch] default none
│   │   │   └── (effect params, visible per Type; Duck shows `Source`, then `Source Tap: [pre | post]` as `Pre | Post`; Delay shows `Mix %`, `Spread %`, `Time Mode`, `Time Note`, then `Time ms`)
│   │   ├── Slot 2: Effect (group)
│   │   │   ├── Type: [same options] default none
│   │   │   └── (effect params, visible per Type)
│   │   ├── Slot 3: Effect (group)
│   │   │   ├── Type: [same options] default none
│   │   │   └── (effect params, visible per Type)
│   │   ├── Volume: [0..100] default 100
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

FM uses a sine carrier and modulator. Tone has six rows in order: Ratio, Fine cents, Index, Vel to Index, Mod Shape, Mod Mix; all fit in the seven-row OLED body. Ratio sets the modulator-to-carrier pitch relationship; Fine cents adjusts the modulator interval, not carrier tuning. Index adds harmonics, with 50 mapping to 2 radians of modulation depth (0..100 maps to 0..4 radians). Vel to Index scales harmonic depth rather than directly scaling amplitude or Volume. Tone edits can reshape notes already ringing. Mod Shape blends a second harmonic when Index is above 0; Mod Mix adds audible modulator tone even at Index 0 and can reinforce the carrier at Ratio 1. Index Env shapes modulation depth independently from the Amp and Filter envelopes. Preset, Tone, Index Env, Filter, Volume, Amp Env, and Filter Env retain their seven-row top-level page; open a group to edit its controls with the usual encoder and Back.

Plucked makes a ringing string you can color with Decay, Brightness, Pick Pos, Pick Depth, Stiffness, and body coloration. String has seven rows in order: Decay, Brightness, Pick Pos, Pick Depth, Stiffness, Body Amt, Body Hz; they fit the existing seven-row OLED body. Decay and Brightness edits can reshape ringing notes. Pick Pos and Pick Depth shape the next pluck: Pick Depth changes excitation color rather than picking position or string Brightness. Stiffness shapes wire-like inharmonic upper partials, not literal string material, weight, or tension; rendered pitch should be measured rather than assumed. Body Amt adds coloration outside the string feedback, while Body Hz moves that coloration when enabled and is dormant at zero Body Amt. Preset, String, Filter, Volume, Amp Env, and Filter Env top-level rows remain unchanged; edit and Back work just as they do for Synth and FM.

Drum starts with eight voices and no assigned grid cells. Its eight top rows scroll in the seven-row OLED body so Volume remains reachable. Sound changes only the selected voice's style controls; the Voice label follows its current Sound. Edit keeps Sound, Tune, Decay, Tone, and Attack first, then adds Sweep st (0..36 semitones), Sweep ms (0..200 ms), Noise % (0..100), and Level % (0..100); its nine rows scroll in the existing seven-row OLED body. Missing legacy values use Sound defaults: Kick 24/55/12/100, Snare 3/25/80/100, Closed/Open Hat 0/0/95/100, Low/High Tom 8/70/15/100 and 6/50/15/100, Clap 0/0/95/100, Rim 0/0/35/100. These controls shape the next hit, not one already ringing. Sweep ms at 0 means no pitch excursion even with positive Sweep st; Noise mixes noise against pitched tone; Level attenuates this voice, not kit-wide Gain. Assign sets/replaces or toggles off cells with the selected voice: plain press edits one cell, Shift+press the whole world-space row (bottom is `y=0`), and the combined modifier a column. Cell Tune edits an assigned cell's own ±24-semitone offset; an empty cell says `No drum here`. Back returns to cell selection, then exits without undoing assignments. Preview plays the selected voice at its own Tune, without a cell offset. Kit Load changes eight voice settings but keeps cell references and tune offsets; it never auto-fills the grid or plays a preview.

FM/Plucked Preset Load and Sampler/Drum Kit Load ask for confirmation with Cancel selected. Confirm changes only that slot; the custom name and mixer stay as they were. Sampler kits replace eight sample paths, not cell assignments, per-cell levels, or the current Sample Slot. `System > Saves` remains the separate whole-patch workflow.

### Routing semantics

- Instrument `Volume` is a post-voice per-slot fader controlled by `Play > Mix`.
- Instrument `Route=direct` sends post-fader output to main mix using instrument `Pan Pos`.
- Instrument `Route=fx_bus_n` sends post-fader output to the selected FX bus (exclusive send); instrument `Pan Pos` is non-editable because bus output pan controls placement.
- Internal synth, FM, Plucked, Drum, and sample instruments use the same route/pan/bus-FX mixer path; MIDI instruments emit external MIDI and are not processed by audio FX.
- Each bus runs `Slot 1`, then `Slot 2`, then `Slot 3` in order; with `none` selected this is passthrough.
- Global FX runs `Slot 1..N` in order on the stereo main mix after direct and bus outputs are summed, before global momentary FX and `Master Vol`.
- FX bus assignments above the recommended active bus warning budget of 12 active bus FX slots are accepted and saved, but the runtime shows a toast warning. Global stereo FX slots do not count toward the bus FX warning budget.
- Global FX is intentionally limited to `none | vinyl | eq | compressor | saturator | distortion` for current Pi Zero 2 W performance targets.
- Bus Delay timing stores `Time Mode` (`ms` or `note`), `Time Note`, and a materialized `Time ms`. In note mode, BPM changes re-materialize `Time ms` from the saved note. In ms mode, `Time ms` is manual and does not retime. Audio/runtime commands receive `timeMs` only; `Time Mode` and `Time Note` are patch metadata and are not bindable targets. `Spread %` widens only the final FX bus output; the bus input and FX slot chain stay mono.
- Duck `Source Tap` stores `pre` or `post` per FX slot and displays `Pre` or `Post`. For an Instrument source, `Pre` is the raw source before its fader; `Post` applies its fader before instrument-target momentary FX and pan. For a Bus source, `Pre` is the bus input before its fader and Slot 1→2→3 chain; `Post` applies that bus fader to the same pre-chain input. A bus input already includes routed instruments' faders and instrument-target momentary FX. Both modes exclude source-bus slot FX, bus-target momentary FX, spread/pan, global FX, and master volume. Missing values in legacy patches behave as `Pre`.
- Selecting a slot `Type` initializes that effect's editable parameter defaults immediately; loaded presets/defaults with missing or invalid effect params are repaired to those defaults.
- Reverb `Decay` is stored as a feedback coefficient (`0..0.995`) but displayed as approximate tail time in seconds (for example `3.1s`) in menu items and aux encoder toasts.
- Bus output is scaled by bus `Volume`, panned by bus `Pan Pos`, and summed to main mix.
- `duck` source options are stable and capability-sized: `I1..I{instrumentCount}` and `B1..B{busCount}`.
- `auto-pan` modulates the bus stereo output position after the slot chain.
- FX bus slot and global slot group labels include the loaded effect display name, e.g. `Slot 1: Delay`, `Slot 2: Duck`, `Slot 3: None`, or `Slot 1: None`.
- FX bus naming mode: `auto` builds from assigned slot types using display names (e.g. `Delay+Reverb`, or `None` when all slots are empty); manual names are preserved exactly. Legacy raw auto names are normalized only when `Auto Label` is on and the stored name is missing or equals the old raw auto-derived value.

Sample assignment mode semantics:

- Enter via `Shape > Instruments > Instrument N > Sampler > Assign`
- Back exits assignment mode
- Entering assignment mode shows a concise OLED toast (for example `Assign S1: grid`); Back continues to exit without changing mappings.
- One sample assignment per cell (new assignment replaces the existing cell assignment)
- With Velocity Levels ON, selected-slot cell presses cycle: `Off -> High(magenta) -> Medium(yellow) -> Low(green) -> Off`
- With Velocity Levels OFF, selected-slot cell presses toggle: `Off <-> Assigned(white)`
- Cells assigned to other sample slots are shown as dim gray during assignment editing
- Shift + cell applies the same toggle/step to the whole row
- Combined modifier + cell applies the same toggle/step to the whole column
- The sample browser menu is labeled with selected slot context (for example `S1 Browse`) and preserves the body rows as browser entries: `..`, built-in/user favourites at the sample root, `[folder]`, file rows, or `(empty)`, followed by a blank separator row and a current-folder favourite action.
- Before directory entries arrive, the browser shows `(loading...)` instead of `(empty)`. Long highlighted names are clipped to the OLED row width rather than overlapping adjacent display areas.
- In `S1 Browse`/sample browser menus, Space previews the highlighted wav file through the selected instrument slot (folders and `..` are no-op); the favourite action toggles the current folder's entry in `runtimeConfig.sampleFavouriteDirs`. Platform built-in favourites are added dynamically and cannot be removed from that config.
- Pi built-in sample favourites are `Samples` (`/home/pi/samples`) and `SD card` (`/home/pi/samples/sd-card/octessera/samples`, with `/home/pi/samples/sd-card` expected to be the OLED SD2 `OCTESSERA` mount point). If the SD card is not mounted, selecting it shows a clear unavailable message. Desktop exposes a built-in `User data` sample favourite.
- Sample preview and assigned sample playback both follow instrument route, pan, volume, bus FX, and master output gain.

Layer runtime behavior:

- All 8 layers run in parallel while transport is running.
- Switching active layer never clears/reset any layer state automatically.
- Switching layer shows the selected layer's current state immediately.
- `Save Grid State` affects preset/default save payload persistence only.
- `looper` stores its recorded sequence in `savedState` as step-bucketed press/release events when `Save Grid State` is `on`. Live-held cells and currently sounding playback cells are not saved; loaded loops restart from step 1.
- `Step Rate`, behavior selection/config, Link mapping, trigger probabilities, instruments, mixer, selected Play page, Play FX assignments, X/Y bindings, and musical aux bindings are patch-persistent and must round-trip through preset/default/autosave payloads. Device/system settings and device/system aux bindings stay local when loading presets.
- Active overlays, assignment modes, held modifiers, active momentary FX instances, live X/Y touch, help popups, and toast state are transient and are not restored from preset/default/autosave payloads.
