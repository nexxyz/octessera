# Behaviors and Play pages

Each layer runs one behavior. Think of a behavior as a small rule-based system: you seed it, nudge it, sometimes interrupt it, and listen to what it decides to become.

You can run several layers at once. One layer might be a stable pulse. Another might be a strange little colony. Another might be your fingers. The fun starts when they disagree politely.

## Behaviors

| Behavior | Category | What it is good for |
|---|---|---|
| `none` | Human | Inactive layer. Useful when you want space, or when you are setting up before sound. |
| `keys` | Human | Momentary finger-drumming. Press a cell to play; release it to stop. Immediate and human. |
| `sequencer` | Human | A manual grid sequence for anchoring the more generative layers. Add probability if it feels too square. |
| `looper` | Human | Records and replays grid presses/releases. Use Punch In/Out to overdub or perform. |
| `weave` | Human | Two interlaced melodic strands crossing over the grid. Useful for quick counterpoint-ish motion. |
| `polyrhythm` | Rhythm | Lanes with different loop lengths shifting against each other. Good for patterns that refuse to line up too neatly. |
| `breaks` | Rhythm | A rearranging breakbeat grid: kicks, cuts, gaps, and little glitchy fragments. |
| `fills` | Rhythm | A base groove that opens up near phrase ends with denser fill gestures. |
| `clave` | Rhythm | A compact clave-style rhythmic cell, rotated and shared across lanes. |
| `groove` | Rhythm | A coherent kick/snare/hat-style grid for fast beat foundations. |
| `euclid` | Rhythm | Evenly distributed hits across uneven lane lengths. Simple controls, surprisingly useful loops. |
| `ostinato` | Musical | A repeating melodic figure that shifts without losing its little anchor. |
| `motif` | Musical | A short melodic shape that mutates, folds, and returns. |
| `canon` | Musical | Delayed copies of a line, stacked into layered motion. Tiny grid fugue, if it behaves. |
| `chords` | Musical | Moving triad-like cells for harmonic blocks and chord strikes. |
| `contour` | Musical | Melody-like cells following rise, fall, arch, and valley shapes. |
| `cadence` | Musical | Harmonic movement with repeated tension and resolution points. |
| `phrase` | Musical | A phrase-shaped pattern with opening, development, rest, and return. |
| `life` | Cellular | Conway-style cells that birth, survive, and die. It also owns glider injection through Glider Interval, Spawn Step, and Spawn Glider; there is no separate `glider` behavior ID. |
| `brain` | Cellular | Brian's Brain style states. It tends to leave trails and pulses rather than simply living/dying. |
| `cyclic` | Cellular | Multi-state wave fronts chase each other around the grid. Bright, discrete, and a little arcade-creature-ish. |
| `forest_fire` | Cellular | Trees grow, catch from neighboring flames, and occasionally get zapped by lightning. Grid presses plant and ignite a cell. |
| `predator_prey` | Cellular | Grass feeds herbivores, herbivores feed predators, and starvation keeps the little ecosystem moving. |
| `ant` | Cellular | Langton-like motion. A tiny agent walks the grid and changes cell states as it goes. |
| `bounce` | Motion | Moving particles that bounce through the grid. Nice for kinetic patterns and repeating collisions. |
| `bubbles` | Motion | Bottom-born bubbles drift upward, merge when they touch, and vanish past the top. Good for light, buoyant motion. |
| `gravity` | Motion | Sand grains fall, slide, settle, and flip direction when you invert gravity. Crunchy little avalanches. |
| `boids` | Motion | A small flock steers by separation, alignment, and cohesion. It chirps when agents enter new cells. |
| `orbit` | Motion | Particles circle a moving attractor, making little orbital flickers as they cross grid cells. |
| `lava_lamp` | Motion | Soft blobs drift, merge, and split into a warm little metaball lamp. |
| `sand_ripples` | Motion | Wind pushes grains into migrating dune crests. Gust it, shift the wind, and listen to the ridges move. |
| `shapes` | Geometry | Geometric areas and edges as musical material. Good when you want a pattern with a visible skeleton. |
| `ink` | Fields | Pigment blooms, diffuses, and fades. Press or drop ink for a splash, then listen as it thins out. |
| `ising` | Fields | Magnetic domains flip between two states under temperature, noise, and field pressure. Tiny spin weather. |
| `kuramoto` | Fields | Coupled phase bubbles drift toward synchronization. Notes appear at wrap flashes rather than from a permanently lit grid. |
| `lightning` | Fields | Branching leaders crawl from one edge toward a target edge, flash when they connect, then decay and restart. |
| `raindrops` | Fields | Drops/ripples across the grid. Great for sparse starts that bloom into motion. |
| `reaction_diffusion` | Fields | Two little chemicals chase each other into spots and edges. Seed it, then let the pattern brew. |
| `rivers` | Fields | Rain falls, water finds downhill paths, and the terrain slowly erodes and deposits sediment. |
| `wave` | Fields | A little vibrating membrane: impulses travel, reflect at the edges, and fade as damping eats the motion. |
| `cracks` | Growth | Crack tips crawl through stressed glass until the pane shatters and clears. Sharp, brittle, and dramatic. |
| `coral` | Growth | Competing colonies grow along exposed edges, leave skeletons, and break away in little reef chunks. |
| `crystal_growth` | Growth | Icy crystal clusters that spread by cross, diagonal, or snowflake symmetry. Press a cell to seed or refresh it. |
| `dla` | Growth | Diffusion-limited aggregation. Slow-growing clusters; more sculpture than step sequencer. |
| `physarum` | Growth | Slime agents sniff trails and food, leaving evaporating paths as they wander. |
| `vines` | Growth | Tendrils climb toward light, branch into open space, leaf out, and prune themselves back. |
| `fractal_explorer` | Geometry | A drifting Mandelbrot/Julia explorer that zooms through regions and turns detail changes into accents. |
| `maze_growth` | Geometry | Tiny maze corridors carve, walkers wander them, and old passages sometimes crumble back to wall. |

The canonical behavior IDs are `none`, `life`, `sequencer`, `keys`, `looper`, `brain`, `cyclic`, `forest_fire`, `predator_prey`, `ant`, `bounce`, `bubbles`, `gravity`, `boids`, `lava_lamp`, `orbit`, `sand_ripples`, `fractal_explorer`, `maze_growth`, `shapes`, `ink`, `ising`, `kuramoto`, `lightning`, `raindrops`, `reaction_diffusion`, `rivers`, `wave`, `coral`, `cracks`, `crystal_growth`, `dla`, `physarum`, `vines`, `weave`, `polyrhythm`, `breaks`, `fills`, `clave`, `groove`, `euclid`, `ostinato`, `motif`, `canon`, `chords`, `contour`, `cadence`, and `phrase`.

## Trigger types

| Trigger | Meaning |
|---|---|
| `activate` | A cell becomes active. |
| `stable` | A cell remains active. |
| `deactivate` | A cell turns off. |
| `scanned` | The scan layer finds an active cell while scanning is enabled. |
| `scanned empty` | The scan layer visits an inactive cell while scanning is enabled. |

These triggers feed Link, probability, note mapping, instruments, FX routing, and output. That is the bridge from cell state to sound.

## Play pages

Play pages are performance overlays. They temporarily borrow the grid so you can play the whole instrument, not just edit it.

Hold **Fn** and use the right grid column to choose a Play page.

| Page | Grid role | Use it for |
|---|---|---|
| Mix | Grid turns into a mixer. | Change the volume of each layer. |
| Pan | Grid becomes a stereo field. | Move around the layers' stereo position. |
| FX | Grid cells hold live effects. | Press mapped cells to trigger effects; release to stop them. |
| Trigger Gate | Grid becomes a trigger gate. | Quickly block, allow, or use custom probability for each layer's triggers. |
| Transpose | Grid becomes a pitch offset picker. | Temporarily transpose eligible synth and MIDI layers. |
| XY | Grid becomes a mappable two-axis surface. | Live-manipulate assigned parameters with X/Y touch position. |

## Play FX details

Play FX are momentary. Pressing a mapped grid cell starts the effect. Releasing it stops the effect. Octessera limits them so the Pi does not melt into a sad little cracker:

- At most two momentary FX are active at once.
- Only one active cell of the same FX type is allowed.
- A second cell of the same FX type is ignored while that type is already active.
- Targets can be `global`, an FX bus, or an instrument insertion point.

## A useful patch recipe

The factory preset is a good orientation point: it has a self-sustaining `life` pattern on Layer 1 and a basic `sequencer` rhythm on Layer 2.

1. Use `sequencer` or `looper`, route it to a `sampler`, and create a basic rhythm loop to ground your track.
2. Add a generative layer: `life`, `raindrops`, `bounce`, `bubbles`, or `dla`, and route it to a `synth`.
3. Use probability on your sequencer cells to make patterns play back in a more interesting way.
4. Bind one or two aux encoders to the parameters or actions you keep reaching for.
5. Open Play Mix or Trigger Gate and perform the layers like little weather systems.

That is the heart of octessera for me: not a song file, not a rigid pattern, but a few small systems making music together.
