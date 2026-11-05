# Sound and Link: from tiles to sound

A behavior only moves tiles around. Three more stops turn that motion into
music:

1. **Link** decides which tile changes count as notes, which instrument they
   go to, what pitch they get, and what else the grid position steers.
2. **Shape** decides what those notes sound like: the instrument, its
   settings, and where its audio goes.
3. **FX buses and Global FX** color the sound on its way out.

Every row in these menus has its own context help on the instrument (hold
*Shift* + *Fn* and click *Main*). This page is the map; the help is the street
view.

![Tile-to-audio flowchart](print/tile-to-audio-flow.svg)

## Link: what counts as a note

Each layer has its own Link page (`Link > L1` to `L8`). A layer can make notes
in two ways, and you can use both at once.

### Events: tiles changing state

Under **Events**, three moments in a tile's life can each trigger something:

| Event | When it fires | Usual job |
|---|---|---|
| **On** | A tile turns on | `note_on` |
| **Hold** | A tile stays on | Often `none`; `note_on` for droning, insistent layers |
| **Off** | A tile turns off | `note_off`, so held notes stop |

Each event has an **Inst** (instrument slot 1 to 8), a **Trig** (`none`,
`note_on` or `note_off`), a **Delay** (0 to 16 layer ticks) and a **Retrig**
(0 to 8 extra repeats, one tick apart). Delay and Retrig are a cheap way to
turn one blip into a little echo figure without touching an effect.

**Event Triggers** switches the whole event path off for the layer without
losing its settings. **State Notes** lets the layer also play from its current
state on every transport tick.

### Scanning: a playhead over the grid

Under **Scanning**, set **Scan Mode** to `scanning` and the layer gets a
cursor that sweeps rows or columns (**Scan Axis**, **Scan Dir**) at the
**Scan Unit** rate, from `1/32T` up to `1/1`. When the cursor finds a lit tile
it plays **Instrument** with **Action**; crossing a dark tile can fire the
separate **Empty Inst** / **Empty Act** pair, handy for release notes or a
ghostly counter-rhythm. **Sections** (1, 2, 4 or 8) splits the sweep into
shorter lanes, so one grid becomes several short loops.

Scanning suits slow, sculptural worlds like `dla`, `coral` and `vines`: their
shape becomes a sequence instead of a trickle of birth events.

### Note Mapping: which pitches

**Note Mapping** keeps a layer in key:

- **Set** picks the note collection: Scales (Chromatic, Major, Natural Minor,
  Harmonic Minor, Dorian, Mixolydian), Chord Tones (triads, sus, sevenths,
  ninths), Pentatonic (including Hirajoshi-, In Sen- and Iwato-like sets) and
  Symmetric (Whole Tone, two Octatonics).
- **Root** sets the key.
- **1st Note** is where pitch counting begins.
- **Low Note** and **High Note** fence the range, and **Out of Range** decides
  what happens at the fence: `clamp` stops at the edge, `wrap` folds back
  round.

Chord Tones sets are lovely on busy layers. Everything that sounds belongs to
one chord, so even chaos sounds like it meant it.

### X and Y: position steers the sound

**X Axis** and **Y Axis** turn where a tile sits into how it sounds. Each lane
has its own **Enabled** switch.

- **Pitch Steps** moves the pitch by **Steps** (-16 to 16) per tile along that
  axis. **Restart Sect** turns scan sections into little grids of their own:
  each lane counts from its own edge, so they all play in the same register.
  It works on the axis the lanes split (Y for column sections, X for row
  sections).
- **Velocity**, **Filter Cutoff** and **Filter Res** map position onto a
  **From** to **To** range, with a **Curve** and a small **Grid Offs** nudge.
- **Slot 1** and **Slot 2** can drive any mappable parameter (an FX mix, a
  synth's detune, a bus volume), each with its own **Invert** row.

A `boids` flock drifting right can open a filter while a `wave` on another
layer makes the delay bloom.

### Trigger probability: not every time

**Trigger Prob. > Mode** decides which triggers get through:

- `full` lets them all through and `zero` blocks the layer.
- `custom` uses a per-tile map you paint with **Map Prob Grid**. Each press
  cycles a tile through zero (dark), low (magenta), high (yellow) and full
  (green); *Shift* paints a row and *Shift* + *Fn* a column.
- **Prob Low** and **Prob High** set what "low" and "high" mean, in percent.

Play's **Trigger Gate** page overrides the mode live without changing what's
saved.

### Arp: notes in a line instead of a lump

When several notes arrive together, **Arp > Mode** can spread them out: `up`,
`down`, `bounce`, `outside_in`, `rotating`, `random`, `octave_spread` or
`strum`. `direct` and `chord_strike` keep them together; `none` leaves Link
alone.

- **Source** picks notes arriving in one batch (`simultaneous`) or notes the
  layer is holding (`held`).
- **Step**, **Length ms**, **Gate %** and **Octaves** shape the run.

### LFOs, tempo and swing

These live at the top of **Link**, not under a layer.

- **LFOs** has eight global sine LFOs. Switch one **Enabled**, pick a
  **Target** (the list only offers parameters that are safe to wobble while
  playing), a **Period** in note lengths (up to eight bars) and a **Depth %**. An LFO adds its
  wobble on top of anything else steering the same parameter. A switched-off LFO keeps counting
  along with the music, so when you switch it back on it comes in right where it would have been.
- **BPM** sets the tempo (40 to 240).
- **Swing A** and **Swing B** set the swing ratio: how long each beat's first
  eighth lasts against its second. `4` and `4` is straight, `5:3` is a lazy
  shuffle, `3:5` rushes the offbeat. Steps land exactly on the ratio, even
  between clock pulses. Following an external MIDI clock, a swung step waits
  for the first clock pulse at or after its spot instead, since nobody tells
  us when the next pulse will come. External MIDI clock out stays straight.
  The Tempo Play page has both rows on the grid.
- Each layer's **Seeded** row makes trigger probability and random arp orders
  repeatable; see [Seeds](behaviors-and-play.md#seeds-surprises-you-can-keep).

### MIDI CC In: your knobs, steering the worlds

**Link > MIDI CC In** has eight slots, **CC1** to **CC8**, that let the knobs
and faders on a MIDI controller move parameters, much like the XY pad does.

- Switch on `System > MIDI > MIDI Active` and connect the controller under
  **MIDI In** (or plug a computer into USB MIDI). Clock and sync settings
  don't matter here.
- In a slot, press **Learn** and wiggle the knob you want. The slot takes its
  CC number and channel (`CC1: learned CC 74 ch 1`). Back, or ten seconds of
  silence, cancels. You can also set **CC** and **Channel** by hand; `any`
  listens on every channel.
- Pick a **Target** from the usual list and narrow it with **Range Min** and
  **Range Max**. The knob's full turn sweeps that range, and **Invert** turns it
  around.
- The parameter stays where the knob left it. The mappings are saved with the
  patch, but a freshly loaded patch keeps its saved values until you touch a
  knob, so nothing jumps on load.
- Switch-like parameters (on/off, a choice from a list) take one driver at a
  time. If an XY slot or another CC already has one, you get `Mapping rejected:
  target already claimed`.

## Shape: what it sounds like

`Shape > Instruments` holds eight instrument slots. Link's **Inst** numbers
point at them.

| Type | What it is | Where to start |
|---|---|---|
| `synth` | Two oscillators into a filter, with amp and filter envelopes | **Osc 1** > Wave and Octave, then **Filter** > Cutoff and **Filter Env** |
| `FM` | A sine carrier and modulator: bells, keys, glassy tones | **Tone** > Ratio and Index, then **Index Env** > Decay |
| `Plucked` | A modelled ringing string | **String** > Decay, Brightness and Pick Pos |
| `Drum` | An eight-voice kit; tiles are stamped with voices | **Kit** > Load, set the **Tile** rows, then **Assign** |
| `sampler` | Eight WAV slots; tiles are stamped with samples | **Browse** for a sample, set the **Tile** rows, then **Assign** |
| `MIDI Inst.` | Sends notes to external gear instead of making sound | **MIDI** > Enabled, then Channel, Velocity and Duration |
| `MIDI Drum` | Turns stamped tiles into MIDI drum notes for external gear | **MIDI Drum** > Enabled, set **Note** and **Tile Vel**, then **Assign** |

- **Presets and kits.** Synth, FM and Plucked have **Preset > Load**; Drum and
  Sampler have **Kit > Load**. Loading asks first and only changes that slot.
- **Note Mode.** `oneshot` gives each trigger a finite note. `hold` keeps the
  note sounding until a matching `note_off`, which is what Off events and the
  scan's empty action are for. Drum hits are always one-shot.
- **Envelopes.** Attack, Decay, Sustain and Release do what they say, and
  Decay runs after the attack whatever the attack length. A sampler's Amp Env
  starts out flat (no attack, full sustain, a 5 ms release) and its filter
  starts fully open, so a sample plays exactly as recorded until you shape it. Give it a slow attack for swells, or a
  short decay and low sustain to chop a loop into stabs. Its Filter Env only
  moves the filter once **Env Amount** is off zero.
- **MIDI.** A `MIDI Inst.` or `MIDI Drum` slot sends nothing until its
  **Enabled** is on (and `System > MIDI > MIDI Active` is on). MIDI never
  passes through the internal effects.
- **Internal notes on MIDI out.** Out of the box only the MIDI slots talk to
  MIDI out. Set `System > MIDI > Send MIDI` to `all inst.` and the synth,
  FM, Plucked, Drum and Sampler slots send their notes too, each on its slot's
  channel (slot 1 on channel 1, and so on), while still playing here. A Drum
  hit sends the General MIDI note of its voice's Sound (kick 36, snare 38,
  closed hat 42 and so on), and Sampler slots 1 to 8 send notes 36 to 43, each
  as a short 100 ms note. Handy for doubling a part on outboard
  gear. Flip it back to `MIDI inst.` and any note still held gets its
  note-off, so nothing hangs on the other end. Muting a slot in Play Mix
  silences its MIDI copy too, the same tidy way.

### Drum and Sampler tiles are stamps

Drum and Sampler tiles play what's stamped onto them, not Link's pitch.

1. Pick the voice (Drum **Voice**) or sample (**Sample Slot**).
2. Set **Tile Tune** (-24 to 24 semitones), **Tile Fine** (-100 to 100 cents)
   and **Tile Level** (Base, High, Medium or Low).
3. Click **Assign** and press tiles. Each press copies those values onto the
   tile, like a rubber stamp. *Shift* stamps the whole row, *Shift* + *Fn* the
   whole column. **Back** finishes.

Want a chromatic row? Stamp each tile with the next Tile Tune. Want a ghost
note? Stamp it Low. On a Sampler, **Tile Level** only appears once
**Vel Levels** is on. High, Medium and Low tiles play at the Vel Levels
velocities; Base tiles play at the normal velocity. To change a tile, stamp it
again. Pressing a tile that already matches exactly clears it.

Everything else (Tune, Gain, Filter, the Vel Levels values) stays slot-wide,
and anything you modulate there still reaches every stamped tile. Loading a
kit swaps the sounds but keeps your stamps.

- **Follow Transp.** lets Play's Transpose page shift these tiles too. Off by
  default.
- **Follow Pitch** makes the tiles play Link's note instead of a fixed pitch,
  measured from C4. Tile Tune and Tile Fine then answer "how far do I nudge
  this sample so it sounds like a C4?". Stamp a bass hit with `-5` and it
  lands on the G below when Link plays C4, then follows Link's melody from
  there. Drums can do it too, for tuned toms that wander with the tune.

### MIDI Drum tiles are stamps too

A `MIDI Drum` slot works like a Drum kit whose sounds live in another box.
Set **Note** (it shows the note name and General MIDI drum name, so
`38 D2 Snare` is easy to spot) and **Tile Vel**, click **Assign** and press
tiles, with the same *Shift* row and *Shift* + *Fn* column tricks. **Back** finishes.

When Link strikes a stamped tile, the slot sends that note and velocity on
its **Channel** (10, the usual drum channel, unless you change it) and lets
go after **Duration**. Tiles you didn't stamp stay quiet, which makes it easy
to leave holes in a pattern. Mute, the trigger gate and trigger probability
work on it like on any other slot. It never makes a sound of its own, so if
nothing happens, check that the drum machine on the other end is listening
on the same channel.

### Mixer

Each internal instrument's **Mixer** has **Route** (`direct` to the main mix,
or `fx_bus_1` to `fx_bus_4`), **Volume** and **Pan Pos**. A slot routed to a
bus takes its pan from the bus. Play Mix and Play Pan edit the same values.

## FX buses: shared effect chains

`Shape > FX Buses` has four buses. Each runs **Slot 1 → Slot 2 → Slot 3** in
order, then its own **Volume** and **Pan Pos**, into the main mix. Send
instruments there with their **Route**.

| Type | Does | Type | Does |
|---|---|---|---|
| `delay` | Echoes, in ms or synced to a note length | `reverb` | Space and tails |
| `chorus` | Doubling shimmer | `flanger` | Swooshing comb filter |
| `vibrato` | Pitch wobble | `tremolo` | Volume pulsing |
| `auto_pan` | Moves the bus around the stereo field | `filter_lfo` | Self-moving filter |
| `wah` | Vocal filter sweep | `glitch` | Stutters |
| `vinyl` | Lo-fi warmth, crackle and warp | `bitcrusher` | Grit |
| `eq` | Three-band tone shaping (±12 dB) | `compressor` | Evens out levels |
| `saturator` | Soft clipping and warmth | `distortion` | Hard clipping |
| `duck` | Turns this bus down when another instrument or bus is loud | `none` | Passes sound through |

**Duck** is the pumping trick. Put a pad on Bus 1 with `duck` sourced from
your kick instrument, and the pad breathes in time.

Choosing a Type loads sensible starting values, so it's safe to try things.
All twelve slots can be busy at once, but see the
[performance table](README.md#practical-performance) before you ask a
Raspberry to run them all.

## Global FX: the finishing touch

`Shape > Global FX` has two slots on the whole mix, after everything is
summed: `vinyl`, `eq`, `compressor`, `saturator` or `distortion`. A touch of
`saturator` or `compressor` glues the layers together. `distortion` here
affects everything, so go gently.

## Turning knobs

Time, frequency and decay rows take finer steps where small changes matter,
so a short attack or a long reverb tail is easy to set. The details are in the
[controls cheat sheet](controls-cheat-sheet.md#regular-controls).

## A patch to try

1. **Layer 1:** `sequencer` → Inst 1 set to `Drum`. Stamp a kick, snare and
   hat onto a few tiles.
2. **Layer 2:** `boids` → Inst 2 set to `synth`, Route `fx_bus_1`. Note
   Mapping Set `Minor Pentatonic`; X Axis **Filter Cutoff** enabled.
3. **Bus 1:** Slot 1 `delay` synced to a note, Slot 2 `duck` sourced from
   `I1`.
4. **Link > LFOs > L1:** Enabled, target Bus 1's delay mix, Period `1/1`,
   Depth around 30%.
5. Press **Play**, then open the **XY** Play page and steer it by hand.

The drums anchor the beat, the flock wanders and opens the filter, the delay
ducks out of the kick's way, and the LFO keeps the echoes from sitting still.
