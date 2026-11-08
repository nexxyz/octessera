# Behaviors and Play pages

Each of the eight layers runs one behavior. A behavior is a small rule-based
world: you seed it, nudge it, sometimes interrupt it, and listen to what it
decides to become.

Run several at once. One layer might be a steady pulse, another a strange
little colony, another your own fingers. Things get interesting when they
disagree politely.

The grid only shows the layer you're on. To keep an eye on the others, switch on
**System > Display > Ghost Tiles**: their lit tiles then glow faintly green
underneath, and anything the current layer draws sits on top.

How a behavior's tiles turn into notes is covered in
[Sound and Link](sound-and-link.md).

## Start with a small patch

The shipped patch is a good place to start. Layer 1 runs a self-sustaining
`life` pattern into a dark saw-and-pulse synth, Layer 2 a `sequencer` beat
into the Sampler (kick, clap and open hat), Layer 3 a `looper` into a Kalimba,
Layer 4 another `sequencer` into the Swell Bass FM voice, and Layer 5 a
`twinkle` sky into FM Glass. Layer 5 starts asleep at a 0% Trigger Gate:
*Shift* + *Fn* and its left-column tile wakes it. The Faders page is set up to play with it, and
the other pages all have something on them to press.

Layer 6 is the quiet one. `raindrops` falls into the Default drum kit, but only
the bottom row has drums stamped on it, so you hear a drop when it lands and
not on the way down. Trigger probability skips about half of those landings,
which keeps it to a stray tom, rim or ghost snare every bar or so. Turn up
Splash Radius in Build if you want the splashes to play too, or shorten the
Spawn Interval for a proper downpour.

These five behaviors make a good first tour:

| Behavior | First thing to try |
|---|---|
| `life` | Seed a few cells and listen to birth, survival and death move through the grid. Glider Interval, Spawn Step and Spawn Glider add gliders. |
| `sequencer` | Set a manual rhythm, then add probability if it feels too square. |
| `keys` | Press tiles to play; release to stop. Finger drumming, basically. |
| `raindrops` | Start with a sparse tile and let drops and ripples bloom outward. |
| `bounce` | Seed moving particles and listen to their repeating collisions. |

## Full behavior catalog

Names are shown as they appear on the OLED. Behaviors are grouped by category
in **Build**.

| Behavior | Category | What it's good for |
|---|---|---|
| `none` | Human | Inactive layer, for when you want space or are setting up. |
| `keys` | Human | Press a tile to play, release it to stop. Immediate and human. |
| `sequencer` | Human | A manual grid sequence for anchoring the more generative layers. It can hold up to 4x4 pages of 8x8: pick the page in view with Page X and Page Y, or let Advance turn the pages for you every few steps (stepping at 1/8, Advance 8 turns once a bar; a scanning layer steps at its Scan Unit). |
| `looper` | Human | Records and replays grid presses and releases. Use Punch In/Out to overdub. |
| `weave` | Human | Two interlaced melodic strands crossing the grid. Quick counterpoint-ish motion. |
| `polyrhythm` | Rhythm | Lanes with different loop lengths shifting against each other. |
| `breaks` | Rhythm | A rearranging breakbeat grid: kicks, cuts, gaps and glitchy fragments. |
| `fills` | Rhythm | A base groove that opens up into denser fills near phrase ends. |
| `clave` | Rhythm | A compact clave-style cell, rotated and shared across lanes. |
| `groove` | Rhythm | A coherent kick/snare/hat-style grid for fast beat foundations. |
| `euclid` | Rhythm | Evenly spread hits across uneven lane lengths. Simple controls, surprisingly useful loops. |
| `ostinato` | Musical | A repeating melodic figure that shifts without losing its anchor. |
| `motif` | Musical | A short melodic shape that mutates, folds and returns. |
| `canon` | Musical | Delayed copies of a line, stacked into layered motion. |
| `chords` | Musical | Moving triad-like tiles for harmonic blocks and chord strikes. |
| `contour` | Musical | Melody-like tiles following rise, fall, arch and valley shapes. |
| `cadence` | Musical | Harmonic movement with repeated tension and resolution points. |
| `phrase` | Musical | A phrase-shaped pattern with opening, development, rest and return. |
| `life` | Cellular | Conway-style cells that are born, survive and die, plus optional glider injection. |
| `brain` | Cellular | Brian's Brain states. Leaves trails and pulses rather than plain living and dying. |
| `cyclic` | Cellular | Multi-state wave fronts chasing each other round the grid. |
| `forest fire` | Cellular | Trees grow, catch from burning neighbors and occasionally get zapped by lightning. Presses plant and ignite. |
| `predator prey` | Cellular | Grass feeds herbivores, herbivores feed predators, and starvation keeps the ecosystem moving. |
| `ant` | Cellular | Langton-style: a tiny agent walks the grid and flips cells as it goes. |
| `twinkle` | Cellular | Stars appear one at a time, linger and fade. Cluster Bias decides whether they bunch up or scatter. |
| `bounce` | Motion | Particles bouncing through the grid. Good for kinetic, repeating patterns. |
| `bubbles` | Motion | Bubbles rise from the bottom, merge when they touch and vanish past the top. |
| `gravity` | Motion | Sand grains fall, slide and settle, and flip direction when you invert gravity. |
| `boids` | Motion | A small flock steering by separation, alignment and cohesion. It chirps when agents enter new tiles. |
| `orbit` | Motion | Particles circling a moving attractor, flickering as they cross tiles. |
| `lava lamp` | Motion | Soft blobs drift, merge and split. |
| `sand ripples` | Motion | Wind pushes grains into migrating dune crests. Gust it, shift the wind, listen to the ridges move. |
| `shapes` | Geometry | Geometric areas and edges as musical material, with a visible skeleton. |
| `fractal explorer` | Geometry | A drifting Mandelbrot/Julia explorer that turns changes in detail into accents. |
| `maze growth` | Geometry | Maze corridors carve, walkers wander them, and old passages sometimes crumble back to wall. |
| `ink` | Fields | Pigment blooms, diffuses and fades. Press for a splash, then listen as it thins out. |
| `ising` | Fields | Magnetic domains flipping under temperature, noise and field pressure. |
| `kuramoto` | Fields | Coupled phases drifting toward sync. Notes come at wrap flashes, not from a permanently lit grid. |
| `lightning` | Fields | Branching leaders crawl toward a target edge, flash when they connect, then decay and restart. |
| `raindrops` | Fields | Drops and ripples. Great for sparse starts that bloom into motion. |
| `reaction diffusion` | Fields | Two chemicals chasing each other into spots and edges. Seed it and let it brew. |
| `rivers` | Fields | Rain falls, water finds downhill paths, and the terrain slowly erodes. |
| `wave` | Fields | A vibrating membrane: impulses travel, reflect at the edges and fade. |
| `cracks` | Growth | Crack tips crawl through stressed glass until the pane shatters and clears. |
| `coral` | Growth | Competing colonies grow along exposed edges, leave skeletons and break off in chunks. |
| `crystal growth` | Growth | Icy clusters spreading by cross, diagonal or snowflake symmetry. Press to seed or refresh. |
| `dla` | Growth | Diffusion-limited aggregation. Slow-growing clusters; more sculpture than step sequencer. |
| `physarum` | Growth | Slime agents sniff trails and food, leaving evaporating paths. |
| `vines` | Growth | Tendrils climb toward light, branch, leaf out and prune themselves back. |
| `pong` | Games | Two CPU paddles rally a ball, and every step of it is a note. Lower CPU Skill for shorter rallies; press a tile to nudge the nearer paddle and maybe cost it the point. |
| `blockout` | Games | A CPU paddle knocks the ball into a rainbow wall. Every brick it breaks flashes and plays a note on its way out, cleared rows drop new ones on top, and a press fills a gap in the wall. |

## Trigger types

| Trigger | Meaning |
|---|---|
| `activate` | A tile turns on. |
| `stable` | A tile stays on. |
| `deactivate` | A tile turns off. |
| `scanned` | The scan cursor finds a lit tile (scanning layers only). |
| `scanned_empty` | The scan cursor crosses a dark tile (scanning layers only). |

Link turns these into notes. See [Sound and Link](sound-and-link.md#link-what-counts-as-a-note).

## Play pages

Play pages are performance overlays. They borrow the grid for a moment so you
can play the whole instrument instead of editing one layer.

The pages come in two banks on the right-hand grid column. Hold *Fn* for the
technical ones (from the bottom up: Mix, Pan, Route, Tempo, LFO, Faders) or *Shift* + *Fn* for the
musical ones (FX, XY, Transpose, Rate, Trigger Gate), then press a tile
in that column. Unlit tiles are free. *Back*, or *Fn* plus a layer in the left
column, leaves the page.

| Page | What the grid becomes | Use it for |
|---|---|---|
| Mix | One column per instrument. The bottom tile mutes and unmutes it (green is live, red is muted); the rows above set the volume, top row 100%. | Setting each instrument's volume, and dropping parts in and out. Hold *Shift* for the four FX buses (volume and mute, in their Pan colors) and the master volume in the sixth column. Mutes are never saved. |
| Pan | One row per instrument; left edge is hard left, right edge hard right. | Placing instruments in the stereo field. Bus-routed instruments move their bus pan. |
| Route | One column per instrument. The bottom five rows pick direct or FX bus 1-4; row 6 switches bus and Global FX chains off and on; the top row picks the instrument every duck listens to and saves it as each duck's Source. | Sending a part through an effect bus, cutting the effects for a dry break, or pumping everything off a different kick. Routes and duck sources stick with the patch; bypass is never saved, so press the lit tile again to undo it. |
| Tempo | The top five rows are tempo tiles from 50 to 240 BPM (the top-left one is Tap), then a row of -4..+4 nudges, then the two swing rows. | Grabbing a tempo, tapping one in, or dialing in swing: Swing A against Swing B, `4:4` straight. The Tap tile blinks with the beat. |
| LFO | One row per global LFO, bottom row is LFO1. The first tile switches it and glows with its wave; the rest pick a period from 1/8 to eight bars. Press a lit period again for its triplet; hold *Shift* for depth. | Slowing a wobble down to a long swell, or snapping it to the beat, without menu diving. |
| Faders | Eight vertical sliders, one per column; press a row to set that column's parameter there. Hold *Shift* for a second set of eight. Columns from the right can become action tiles instead (see below). | Riding 16 parameters of your choice, like a little fader box. Each bar shows where its parameter is right now, whoever moved it last. |
| FX | Tiles you've mapped with momentary effects. | Press to start an effect, release to stop it. |
| Trigger Gate | One row per layer, bottom row is Layer 1. The first three columns pick `0%`, `custom` or `100%`. | Blocking, allowing or probability-gating each layer live. The bottom row's right three tiles set every layer at once. |
| Transpose | Left column picks which layers follow; the rest is a three-octave piano. | Shifting Synth, FM, Plucked and enabled MIDI notes. Sampler and Drum tiles only follow when their **Follow Transp.** is on. Not saved with the patch. |
| Rate | One row per layer, bottom row is Layer 1; the columns are step rates from 1/32 to 1/2, straight green and triplets blue. Hold *Shift* for each scanning layer's Scan Unit (straight in white). | Speeding a layer up or slowing it down mid-groove. Press a layer's lit tile to pull it back onto the bar at the next bar line. |
| XY | A two-axis touch surface. | Steering whatever you assign under `Play > XY > X Axis` and `Y Axis`. *Shift* + tile parks it there after you let go, even when **Release** springs back to the center; a plain press un-parks it. |

Trigger Gate overrides the layer's saved `Trigger Prob.` mode without changing
it, so the patch keeps its saved mode. Hold *Fn* to check which layers are
blocked: they're magenta in the left column.

### Faders

Each column of the Faders page is a slider with two targets: `Target` for a
plain press and `S+Target` for *Shift* + press, picked under
`Play > Faders > Fader 1..8` from the same list as the LFOs and Aux turns. The
bottom row gives the target's **Range Min**, the top row its **Range Max**, and
**Invert** flips that. A press sets the value and lets go: turn the same knob in
the menu or load a preset and the bar follows. Columns without a target stay
dark and ignore you.

Want buttons too? Turn up **Action Cols** and that many columns, counted from
the right, become tiles. Set one up in `Play > Faders > Tile Map`: pick a
**Type**, then either two actions (`Action` for a plain press, `S+Action` for
*Shift* + press, the same ones an Aux click can run, and a behavior action
keeps working on the layer you picked it on) or a parameter `Target`
that a press nudges up one step (*Shift* + press: down), just like turning an
Aux encoder by one detent. Then **Map to Grid** and press the tile. Action
tiles glow white, parameter tiles yellow; *Back* leaves without changing
anything, and mapping type `none` clears a tile.

Want to finger-drum? Give a layer the `keys` behavior and point its
Link **Inst** at a Drum instrument. Now the stamped tiles are your pads, and
your taps get Link, probability and Play FX like any other layer. Your fingers
are just one more little world in the mix.

## Play FX

Set up momentary effects under `Play > FX`: choose an **FX Type**, a
**Target**, and its settings, then use **Map to Grid** and press a tile. That
tile remembers the whole lot. Mapping `none` clears a tile. Hold the tile to
play the effect; let go and it winds down on its own terms.

| Type | What happens while you hold it |
|---|---|
| `stutter` | Grabs a slice and loops it. |
| `freeze` | Catches the moment in an endless shimmering tail. |
| `filter sweep` | Closes a lowpass down, then opens it back up. |
| `pitch` | Glides the sound up or down by semitones. |
| `tape stop` | The tape slows to a halt, pitch and all. Let go and it spins back up. |
| `reverse` | After one Length of listening, plays what it heard backwards. |
| `gate` | Chops the sound in time with the tempo, from 1/32T to 1/4. |
| `dub` | Throws the sound into tempo-synced echoes that keep trailing after you let go. |
| `wash` | Swells into a big reverb that keeps listening, then fades over Release. |
| `HP riser` | Sweeps a highpass up for a build-up, then drops back open. |
| `crush` | Bitcrush and sample-rate grit, from a light fizz to a broken radio. |

The two tempo-synced types, `gate` and `dub`, follow the tempo,
including an external MIDI clock when you sync to one.

- **Target** is `master` (the whole mix), one of the FX buses, or a single
  instrument.
- Two momentary effects can run at once.
- Only one of each type at a time. A second tile of the same type is ignored
  while the first is held, and a toast tells you why. Blocked tiles dim.

## A useful patch recipe

1. Use `sequencer` or `looper` into a `sampler` or `drum` and build a basic
   rhythm to ground the track.
2. Add a generative layer (`life`, `raindrops`, `bounce`, `bubbles` or `dla`)
   into a `synth`.
3. Paint some probability onto the sequencer tiles so the rhythm breathes.
4. Bind one or two Aux encoders to whatever you keep reaching for (*Fn* +
   click an Aux encoder on the highlighted row).
5. Open Play Mix or Trigger Gate and perform the layers.

## Seeds: surprises you can keep

By default every random-ish world rolls fresh dice each time. If a layer just
did something you love, switch on `Build > Lx > Seeded`, and
`Link > Lx > Seeded` too if trigger probability or a random arp is part of the
magic.

Seeded layers all follow the one `Build > Seed` number (1 to 9999), and their
dice restart from it whenever you play from stop or load the patch. A world
that has already grown keeps growing from where it is, so for the same weather
again, reload the patch (or Reset the layer) and play it the same way. Turn
Seed to reroll every seeded layer at once, and leave other layers unseeded to
keep a bit of chaos around. Your live playing still nudges things, which is
rather the point.
