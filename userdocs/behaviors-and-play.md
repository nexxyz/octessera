# Behaviors and Play pages

Each of the eight layers runs one behavior. A behavior is a small rule-based
world: you seed it, nudge it, sometimes interrupt it, and listen to what it
decides to become.

Run several at once. One layer might be a steady pulse, another a strange
little colony, another your own fingers. Things get interesting when they
disagree politely.

The grid only shows the layer you're on. To keep an eye on the others, switch on
**System > Display > Ghost Cells**: their lit cells then glow faintly green
underneath, and anything the current layer draws sits on top.

How a behavior's cells turn into notes is covered in
[Sound and Link](sound-and-link.md).

## Start with a small patch

The shipped patch is a good place to start. Layer 1 runs a self-sustaining
`life` pattern into a synth, Layer 2 a `sequencer` into the sampler, Layer 3 a
`looper` into a plucked string, and Layer 4 another `sequencer` into the FM
voice. These five behaviors make a good first tour:

| Behavior | First thing to try |
|---|---|
| `life` | Seed a few cells and listen to birth, survival and death move through the grid. Glider Interval, Spawn Step and Spawn Glider add gliders. |
| `sequencer` | Set a manual rhythm, then add probability if it feels too square. |
| `keys` | Press cells to play; release to stop. Finger drumming, basically. |
| `raindrops` | Start with a sparse cell and let drops and ripples bloom outward. |
| `bounce` | Seed moving particles and listen to their repeating collisions. |

## Full behavior catalog

Names are shown as they appear on the OLED. Behaviors are grouped by category
in **Build**.

| Behavior | Category | What it's good for |
|---|---|---|
| `none` | Human | Inactive layer, for when you want space or are setting up. |
| `keys` | Human | Press a cell to play, release it to stop. Immediate and human. |
| `sequencer` | Human | A manual grid sequence for anchoring the more generative layers. It can hold up to 4x4 pages of 8x8: pick the page in view with Page X and Page Y, or let Advance turn the pages for you every few steps (at Step Rate 1/8, Advance 8 turns once a bar). |
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
| `chords` | Musical | Moving triad-like cells for harmonic blocks and chord strikes. |
| `contour` | Musical | Melody-like cells following rise, fall, arch and valley shapes. |
| `cadence` | Musical | Harmonic movement with repeated tension and resolution points. |
| `phrase` | Musical | A phrase-shaped pattern with opening, development, rest and return. |
| `life` | Cellular | Conway-style cells that are born, survive and die, plus optional glider injection. |
| `brain` | Cellular | Brian's Brain states. Leaves trails and pulses rather than plain living and dying. |
| `cyclic` | Cellular | Multi-state wave fronts chasing each other round the grid. |
| `forest fire` | Cellular | Trees grow, catch from burning neighbours and occasionally get zapped by lightning. Presses plant and ignite. |
| `predator prey` | Cellular | Grass feeds herbivores, herbivores feed predators, and starvation keeps the ecosystem moving. |
| `ant` | Cellular | Langton-style: a tiny agent walks the grid and flips cells as it goes. |
| `twinkle` | Cellular | Stars appear one at a time, linger and fade. Cluster Bias decides whether they bunch up or scatter. |
| `bounce` | Motion | Particles bouncing through the grid. Good for kinetic, repeating patterns. |
| `bubbles` | Motion | Bubbles rise from the bottom, merge when they touch and vanish past the top. |
| `gravity` | Motion | Sand grains fall, slide and settle, and flip direction when you invert gravity. |
| `boids` | Motion | A small flock steering by separation, alignment and cohesion. It chirps when agents enter new cells. |
| `orbit` | Motion | Particles circling a moving attractor, flickering as they cross cells. |
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

## Trigger types

| Trigger | Meaning |
|---|---|
| `activate` | A cell turns on. |
| `stable` | A cell stays on. |
| `deactivate` | A cell turns off. |
| `scanned` | The scan cursor finds a lit cell (scanning layers only). |
| `scanned_empty` | The scan cursor crosses a dark cell (scanning layers only). |

Link turns these into notes. See [Sound and Link](sound-and-link.md#link-what-counts-as-a-note).

## Play pages

Play pages are performance overlays. They borrow the grid for a moment so you
can play the whole instrument instead of editing one layer.

Hold *Fn* and press a cell in the right-hand grid column to open a page.
From the bottom up: Mix, Pan, FX, Trigger Gate, Transpose, XY, Drums (the top
cell is unused). *Back*, or *Fn* plus a layer in the left column, leaves the
page.

| Page | What the grid becomes | Use it for |
|---|---|---|
| Mix | One column per instrument; bottom row is mute, top row is 100%. | Setting each instrument's volume. |
| Pan | One row per instrument; left edge is hard left, right edge hard right. | Placing instruments in the stereo field. Bus-routed instruments move their bus pan. |
| FX | Cells you've mapped with momentary effects. | Press to start an effect, release to stop it. |
| Trigger Gate | One row per layer, bottom row is Layer 1. The first three columns pick `0%`, `custom` or `100%`. | Blocking, allowing or probability-gating each layer live. The bottom row's right three cells set every layer at once. |
| Transpose | Left column picks which layers follow; the rest is a three-octave piano. | Shifting Synth, FM, Plucked and enabled MIDI notes. Sampler and Drum cells only follow when their **Follow Transp.** is on. Not saved with the patch. |
| XY | A two-axis touch surface. | Steering whatever you assign under `Play > XY > X Axis` and `Y Axis`. |
| Drums | The cells stamped on the selected Drum slot. | Tapping drum hits directly, even while stopped. Empty cells stay quiet. |

Trigger Gate overrides the layer's saved `Trigger Prob.` mode without changing
it, so the patch keeps its saved mode.

## Play FX

Set up momentary effects under `Play > FX`: choose an **FX Type** (`stutter`,
`freeze`, `filter_sweep` or `pitch_shift`), a **Target**, and its settings,
then use **Map to Grid** and press a cell. That cell remembers the whole lot.
Mapping `none` clears a cell.

- **Target** is `master` (the whole mix), one of the FX buses, or a single
  instrument.
- Two momentary effects can run at once.
- Only one of each type at a time. A second cell of the same type is ignored
  while the first is held, and a toast tells you why. Blocked cells dim.

## A useful patch recipe

1. Use `sequencer` or `looper` into a `sampler` or `drum` and build a basic
   rhythm to ground the track.
2. Add a generative layer (`life`, `raindrops`, `bounce`, `bubbles` or `dla`)
   into a `synth`.
3. Paint some probability onto the sequencer cells so the rhythm breathes.
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
