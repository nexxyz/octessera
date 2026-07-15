# Shape Menu Tree

This file is part of the canonical split-out menu tree spec. See [`../menu-tree-spec.md`](../menu-tree-spec.md) for the canonical index.

### Shape

```
Shape
├── Instruments (group)
│   ├── Instrument 1..8 (group)                ← compact overview label e.g. `I1: synth direct`, `I2: samp fxb1`, `I3: midi ch1`
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
│   │   │   └── Pan Pos: [0..32] quantized (33-position stereo scale; 16=center; shown as non-editable bus pan note when Route=fx_bus_n)
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
│   │   │   ├── Type: [none | reverb | delay | tremolo | chorus | flanger | vibrato | auto_pan | filter_lfo | wah | vinyl | eq | compressor | duck | saturator | distortion | bitcrusher | glitch] default none
│   │   │   └── (effect params, visible per Type; Delay shows `Mix %`, `Spread %`, `Time Mode`, `Time Note`, then `Time ms`)
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

Routing semantics:

- Instrument `Volume` is a post-voice per-slot fader controlled by `Play > Mix`.
- Instrument `Route=direct` sends post-fader output to main mix using instrument `Pan Pos`.
- Instrument `Route=fx_bus_n` sends post-fader output to the selected FX bus (exclusive send); instrument `Pan Pos` is non-editable because bus output pan controls placement.
- Internal synth and sample instruments use the same route/pan/bus-FX mixer path; MIDI instruments emit external MIDI and are not processed by audio FX.
- Each bus runs `Slot 1`, then `Slot 2`, then `Slot 3` in order; with `none` selected this is passthrough.
- Global FX runs `Slot 1..N` in order on the stereo main mix after direct and bus outputs are summed, before global momentary FX and `Master Vol`.
- FX bus assignments above the recommended active bus warning budget of 6 active bus FX slots are accepted and saved, but the runtime shows a toast warning. Global stereo FX slots do not count toward the bus FX warning budget.
- Global FX is intentionally limited to `none | vinyl | eq | compressor | saturator | distortion` for current Pi Zero 2 W performance targets.
- Bus Delay timing stores `Time Mode` (`ms` or `note`), `Time Note`, and a materialized `Time ms`. In note mode, BPM changes re-materialize `Time ms` from the saved note. In ms mode, `Time ms` is manual and does not retime. Audio/runtime commands receive `timeMs` only; `Time Mode` and `Time Note` are patch metadata and are not bindable targets. `Spread %` widens only the final FX bus output; the bus input and FX slot chain stay mono.
- Selecting a slot `Type` initializes that effect's editable parameter defaults immediately; loaded presets/defaults with missing or invalid effect params are repaired to those defaults.
- Reverb `Decay` is stored as a feedback coefficient (`0..0.995`) but displayed as approximate tail time in seconds (for example `3.1s`) in menu rows and aux encoder toasts.
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
- Pi built-in sample favourites are `Samples` (`/home/pi/samples`) and `SD card` (`/home/pi/samples/sd-card/octessera/samples`, with `/home/pi/samples/sd-card` expected to be the OLED SD2 `OCTESSERA_SD` mount point). If the SD card is not mounted, selecting it shows a clear unavailable message. Desktop exposes a built-in `User data` sample favourite.
- Sample preview and assigned sample playback both follow instrument route, pan, volume, bus FX, and master output gain.

Layer runtime behavior:

- All 8 layers run in parallel while transport is running.
- Switching active layer never clears/reset any layer state automatically.
- Switching layer shows the selected layer's current state immediately.
- `Save Grid State` affects preset/default save payload persistence only.
- `looper` stores its recorded sequence in `savedState` as step-bucketed press/release events when `Save Grid State` is `on`. Live-held cells and currently sounding playback cells are not saved; loaded loops restart from step 1.
- `Step Rate`, behavior selection/config, Link mapping, trigger probabilities, instruments, mixer, system settings, selected Play page, Play FX assignments, X/Y bindings, and aux bindings are persistent and must round-trip through preset/default/autosave payloads.
- Active overlays, assignment modes, held modifiers, active momentary FX instances, live X/Y touch, help popups, and toast state are transient and are not restored from preset/default/autosave payloads.
