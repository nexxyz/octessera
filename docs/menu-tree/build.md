# Build Menu Tree

Part of the authoritative menu/control spec; see [`../menu-tree-spec.md`](../menu-tree-spec.md) for
the index and notation. Build is where each layer gets its world: which behavior runs, how often it
steps, and the knobs that behavior exposes.

## Tree

```
Build
├── L1: <label> (group)                          ← one group per layer; entering it makes that layer active
│   ├── Behavior: <label> (group)                ← browser-style selector, not an editable enum
│   │   ├── [Human]
│   │   │   ├── ..
│   │   │   ├── keys
│   │   │   ├── looper
│   │   │   ├── none
│   │   │   ├── sequencer
│   │   │   └── weave
│   │   ├── [Rhythm]
│   │   │   ├── ..
│   │   │   ├── polyrhythm
│   │   │   ├── breaks
│   │   │   ├── fills
│   │   │   ├── clave
│   │   │   ├── groove
│   │   │   └── euclid
│   │   ├── [Musical]
│   │   │   ├── ..
│   │   │   ├── ostinato
│   │   │   ├── motif
│   │   │   ├── canon
│   │   │   ├── chords
│   │   │   ├── contour
│   │   │   ├── cadence
│   │   │   └── phrase
│   │   ├── [Cellular]
│   │   │   ├── ..
│   │   │   ├── ant
│   │   │   ├── brain
│   │   │   ├── cyclic
│   │   │   ├── forest fire
│   │   │   ├── life
│   │   │   ├── predator prey
│   │   │   └── twinkle
│   │   ├── [Fields]
│   │   │   ├── ..
│   │   │   ├── ink
│   │   │   ├── ising
│   │   │   ├── kuramoto
│   │   │   ├── lightning
│   │   │   ├── raindrops
│   │   │   ├── reaction diffusion
│   │   │   ├── rivers
│   │   │   └── wave
│   │   ├── [Geometry]
│   │   │   ├── ..
│   │   │   ├── fractal explorer
│   │   │   ├── maze growth
│   │   │   └── shapes
│   │   ├── [Growth]
│   │   │   ├── ..
│   │   │   ├── coral
│   │   │   ├── cracks
│   │   │   ├── crystal growth
│   │   │   ├── dla
│   │   │   ├── physarum
│   │   │   └── vines
│   │   └── [Motion]
│   │       ├── ..
│   │       ├── bounce
│   │       ├── bubbles
│   │       ├── gravity
│   │       ├── boids
│   │       ├── lava lamp
│   │       ├── orbit
│   │       └── sand ripples
│   ├── Auto Label: [on | off]                   ← on: label follows the behavior ID; off: manual text
│   ├── Layer Label: (text, max 32)              ← editing sets Auto Label off
│   ├── Step Rate: [1/32T | 1/32 | 1/16T | 1/16 | 1/8T | 1/8 | 1/4T | 1/4 | 1/2T | 1/2 | 1/1T | 1/1]  ← how often the behavior ticks; hidden for none
│   ├── ... behavior config rows from configMenu() ← hidden for none
│   ├── Seeded: [on | off]                       ← default off; hidden for none
│   └── Reset (action)                           ← reinitializes the behavior state; hidden for none
├── L2: <label> (group)
├── ...                                          ← layerCount groups in total
└── Seed: [1..9999] step 1                       ← default 1; shared by every Seeded Build and Link layer
```

- Selector items show catalog labels. The persisted ID replaces spaces with underscores
  (`forest fire` is `forest_fire`, `predator prey` is `predator_prey`, and so on); every other label
  equals its ID. Category keys for help are `behavior.category.*`.
- Selecting a behavior writes its ID to the layer's persisted `behaviorId`, switches that layer
  through the native runtime, and returns focus to the layer's Behavior row. Only the affected Build
  layer rows refresh; the full menu tree is not rebuilt.
- With Auto Label on, the layer label follows the behavior ID (for example `life`, `brain`). Layer
  selectors (Fn + left column, Link layer groups) show the computed label, for example `L1: life`.
- For behavior `none` the layer group shows only Behavior, Auto Label, and Layer Label. Step Rate,
  config rows, Seeded, and Reset are hidden; stored values are kept.
- `Seed` sits after the layer groups so the Build cursor still lands on the active layer. It is
  persisted per patch as `runtimeConfig.randomSeed`. What Seeded changes and when streams restart is
  defined in [Seeded Randomness](../menu-and-controls-spec.md#seeded-randomness).
- Behavior targets for modulation, Aux, and XY pickers are described under
  [Parameter target pickers](../menu-and-controls-spec.md#parameter-target-pickers).
- Removed IDs: `arp` is not a Build behavior (arpeggiation is `Link > Lx > Arp`), `glider` is gone
  (its injection controls are part of `life`), and there are no `forest`, `crystal`, or `crystals`
  aliases. No other behavior has aliases.

## Layer runtime behavior

- All layers run in parallel while transport runs. Switching the active layer never clears or resets
  any layer, and the newly selected layer's current state shows immediately.
- Preset/default save payloads always include each layer's grid state. `looper` stores its recorded
  sequence in `savedState` as step-bucketed press/release events; live-held cells and currently sounding
  playback cells are not saved, and a loaded loop restarts from step 1.
- Patch-persistent, and round-tripped through preset/default/autosave payloads: Step Rate, behavior
  selection and config, Link mapping, trigger probabilities, instruments, mixer, the selected Play
  page, Play FX assignments, X/Y bindings, and musical aux bindings. Device/System settings and
  device/System aux bindings stay local when a preset loads.
- Transient, never restored from those payloads: active overlays, assignment modes, held modifiers,
  active momentary FX, live X/Y touch, help popups, and toasts.

## Behavior categories

| Category | Behaviors (menu order) | Description |
|---|---|---|
| Human | keys, looper, none, sequencer, weave | Direct performance, recording, silence, step-style behaviors, and hand-playable pattern nudges. |
| Rhythm | polyrhythm, breaks, fills, clave, groove, euclid | Rhythm-first pattern worlds for interlocking pulses, asymmetry, breaks, fills, and Euclidean-style grids. |
| Musical | ostinato, motif, canon, chords, contour, cadence, phrase | Pitch-shape pattern worlds for repeated figures, delayed echoes, chord blocks, contours, cadences, and longer phrases. |
| Cellular | ant, brain, cyclic, forest_fire, life, predator_prey, twinkle | Cell-state simulations where neighboring cells or agents create evolving patterns. |
| Fields | ink, ising, kuramoto, lightning, raindrops, reaction_diffusion, rivers, wave | Field-style activity that spreads from localized events. |
| Geometry | fractal_explorer, maze_growth, shapes | Fractal exploration, maze carving, and explicit geometric pulse patterns. |
| Growth | coral, cracks, crystal_growth, dla, physarum, vines | Coral, crack, crystal, slime, vine, and diffusion-limited clusters that grow from seeded particles. |
| Motion | bounce, bubbles, gravity, boids, lava_lamp, orbit, sand_ripples | Moving objects, blobs, flocking/orbiting agents, bubbles, dunes, and falling sand. |

## Behavior config rows

Rows come from each behavior's `configMenu()` in this order. `!` rows are actions; each fires the
behavior's own action key through the generic behavior-action path (see
[Inactive bindings](../menu-and-controls-spec.md#inactive-stale-bindings) for Aux press bindings).
The pattern behaviors (weave, Rhythm, Musical) label their rows in lowercase.

| Behavior | Config Items | Type/Options |
|---|---|---|
| none | *(none)* | — |
| life | Spawn Count: [0..20] | number, step 1 (default 0) |
| life | Spawn Interval: [1..20] | number, step 1 (default 1) |
| life | Glider Interval: [0..20] | number, step 1 (default 0; 0 disables automatic glider injection) |
| life | Spawn Step: [0..63] | number, step 1 |
| life | !Spawn Random | action |
| life | !Spawn Glider | action |
| sequencer | Pages X: [1..4] | number, step 1 (default 1): pages side by side on the canvas |
| sequencer | Pages Y: [1..4] | number, step 1 (default 1): pages stacked up |
| sequencer | Page X: [1..4] | number, step 1 (default 1): the column of pages in view, clamped to Pages X |
| sequencer | Page Y: [1..4] | number, step 1 (default 1): the row of pages in view, clamped to Pages Y |
| sequencer | Advance: [0..64] | number, step 1 (default 0 = off): turn to the next page every this many steps |
| keys | Quantize: [immediate, step] | enum (default immediate) |
| looper | !Punch In/Out | action |
| looper | Length: [1..64] | number, step 1 (default 16) |
| looper | !Clear Loop | action |
| weave, Rhythm, Musical | density: [10..80] | number, step 5 (default set per behavior) |
| weave, Rhythm, Musical | variation: [0..100] | number, step 5 (default set per behavior) |
| weave, Rhythm, Musical | cycle: [4..32] | number, step 1 (default set per behavior) |
| weave, Rhythm, Musical | seed: [1..9999] | number, step 1 (default set per behavior) |
| brain | Fire Threshold: [1..4] | number, step 1 (default 2) |
| brain | Seed Interval: [0..30] | number, step 1 (default 2; 0 disables scheduled seeding) |
| brain | Spawn Step: [0..63] | number, step 1 (default 0) |
| brain | Spawn Count: [0..20] | number, step 1 (default 2) |
| brain | !Seed Random | action |
| cyclic | States: [3..8] | number, step 1 (default 4) |
| cyclic | Threshold: [1..8] | number, step 1 (default 1) |
| cyclic | Range: [1..2] | number, step 1 (default 1) |
| cyclic | !Seed Cycle | action |
| forest_fire | Tree Density: [0..100] | number, step 1 (default 34) |
| forest_fire | Grow Chance: [0..100] | number, step 1 (default 5) |
| forest_fire | Spread Chance: [0..100] | number, step 1 (default 70) |
| forest_fire | Reseed Thresh.: [0..100] | number, step 1 (default 5) |
| forest_fire | Lightning: [0..20] | number, step 1 (default 1; per-thousand chance) |
| forest_fire | !Ignite Random | action |
| predator_prey | Grass Grow: [0..100] | number, step 1 (default 15) |
| predator_prey | Herbiv. Repro: [0..100] | number, step 1 (default 15) |
| predator_prey | Predator Repro: [0..100] | number, step 1 (default 8) |
| predator_prey | Starve Ticks: [1..32] | number, step 1 (default 8) |
| predator_prey | !Reseed Ecosystem | action |
| twinkle | Density: [1..5] | number, step 1 (default 3; hard star cap) |
| twinkle | Birth Chance: [0..100] | number, step 1 (default 70) |
| twinkle | Fade Chance: [0..100] | number, step 1 (default 35) |
| twinkle | Star Life: [1..32] | number, step 1 (default 8; minimum fade age) |
| twinkle | Cluster Bias: [0..100] | number, step 1 (default 40; clipped neighborhood preference) |
| twinkle | Seed: [0..65535] | number, step 1 (default 1) |
| twinkle | !Reseed Stars | action, deterministic reset using `seed` |
| twinkle | !Clear Stars | action, clears the constellation without changing configuration |
| ant | Max Ants: [1..100] | number, step 1 (default 50) |
| ant | Spawn Interval: [0..20] | number, step 1 (default 16) |
| ant | Spawn Step: [0..63] | number, step 1 (default 5) |
| ant | !Spawn Ant | action |
| bounce | Spawn Interval: [0..30] | number, step 1 (default 16) |
| bounce | Spawn Step: [0..63] | number, step 1 (default 7) |
| bounce | Max Balls: [1..100] | number, step 1 (default 60) |
| bounce | !Add Ball | action |
| bubbles | Spawn Interval: [0..30] | number, step 1 (default 3) |
| bubbles | Spawn Step: [0..63] | number, step 1 (default 0) |
| bubbles | Spawn Count: [1..8] | number, step 1 (default 2) |
| bubbles | Min Radius: [1..4] | number, step 1 (default 1) |
| bubbles | Max Radius: [1..4] | number, step 1 (default 3) |
| bubbles | Drift: [0..8] | number, step 1 (default 0); eighth-cell units |
| bubbles | Current: [-8..8] | number, step 1 (default 0); eighth-cell units |
| bubbles | Buoyancy: [1..8] | number, step 1 (default 3); eighth-cell units |
| bubbles | Max Bubbles: [1..64] | number, step 1 (default 24) |
| bubbles | !Add Bubble | action |
| gravity | Spawn Rate: [0..100] | number, step 1 (default 20) |
| gravity | Slide Chance: [0..100] | number, step 1 (default 60) |
| gravity | Settle Age: [1..32] | number, step 1 (default 8) |
| gravity | Gravity Dir: [down, left, up, right] | enum (default down) |
| gravity | !Drop Sand | action |
| gravity | !Clear Bottom | action |
| gravity | !Invert Gravity | action |
| boids | Flock Size: [1..24] | number, step 1 (default 12) |
| boids | Separation: [0..100] | number, step 1 (default 45) |
| boids | Alignment: [0..100] | number, step 1 (default 35) |
| boids | Cohesion: [0..100] | number, step 1 (default 25) |
| boids | !Scatter Flock | action |
| boids | !Seed Flock | action |
| orbit | Particle Count: [1..16] | number, step 1 (default 8) |
| orbit | Attraction: [0..100] | number, step 1 (default 45) |
| orbit | Orbit: [0..100] | number, step 1 (default 55) |
| orbit | Repel Mode: [off, always] | enum (default off) |
| orbit | !Reset Orbit | action |
| orbit | !Nudge Attractor | action |
| lava_lamp | Blob Count: [1..8] | number, step 1 (default 4) |
| lava_lamp | Viscosity: [0..100] | number, step 1 (default 30) |
| lava_lamp | Heat: [0..100] | number, step 1 (default 45) |
| lava_lamp | Merge: [0..100] | number, step 1 (default 25) |
| lava_lamp | !Heat Lamp | action |
| lava_lamp | !Reset Blobs | action |
| sand_ripples | Wind Strength: [0..100] | number, step 1 (default 45) |
| sand_ripples | Deposition: [0..100] | number, step 1 (default 35) |
| sand_ripples | Erosion: [0..100] | number, step 1 (default 25) |
| sand_ripples | !Gust | action |
| sand_ripples | !Shift Wind | action |
| sand_ripples | !Seed Dunes | action |
| shapes | Shape: [ring, filled, diamond, cross, x] | enum (default ring) |
| shapes | Lifespan: [1..12] | number, step 1 (default 3) |
| shapes | Max Radius: [4..32] | number, step 1 (default 12) |
| shapes | Spawn Interval: [0..20] | number, step 1 (default 4; 0 disables scheduled pulses) |
| shapes | Spawn Step: [0..63] | number, step 1 (default 2) |
| shapes | !Spawn Pulse | action |
| fractal_explorer | Zoom Rate: [0..100] | number, step 1 (default 8) |
| fractal_explorer | Drift: [0..100] | number, step 1 (default 20) |
| fractal_explorer | Iteration Limit: [8..64] | number, step 1 (default 24) |
| fractal_explorer | Fractal: [mandelbrot, julia] | enum (default mandelbrot) |
| fractal_explorer | !Jump Region | action |
| fractal_explorer | !Toggle Fractal Mode | action |
| maze_growth | Carve: [0..100] | number, step 1 (default 100) |
| maze_growth | Collapse Age: [1..64] | number, step 1 (default 32) |
| maze_growth | Walker Count: [1..8] | number, step 1 (default 2) |
| maze_growth | !Restart Maze | action |
| maze_growth | !Collapse Maze | action |
| ink | Diffusion: [0..100] | number, step 1 (default 30) |
| ink | Fade: [0..100] | number, step 1 (default 5) |
| ink | Drop Strength: [1..255] | number, step 1 (default 180) |
| ink | Drop Interval: [0..64] | number, step 1 (default 16) |
| ink | Spawn Step: [0..63] | number, step 1 (default 5) |
| ink | !Drop Ink | action |
| ink | !Clear Ink | action |
| ising | Temperature: [0..100] | number, step 1 (default 35) |
| ising | Field Strength: [0..100] | number, step 1 (default 15) |
| ising | Noise: [0..100] | number, step 1 (default 8) |
| ising | !Heat Pulse | action |
| ising | !Flip Field | action |
| ising | !Randomize Spins | action |
| reaction_diffusion | Feed: [0..100] | number, step 1 (default 35) |
| reaction_diffusion | Kill: [0..100] | number, step 1 (default 55) |
| reaction_diffusion | Diffusion: [0..100] | number, step 1 (default 35) |
| reaction_diffusion | Reaction: [0..100] | number, step 1 (default 50) |
| reaction_diffusion | Seed Interval: [0..64] | number, step 1 (default 3; 0 disables scheduled seeding) |
| reaction_diffusion | Spawn Step: [0..63] | number, step 1 (default 2) |
| reaction_diffusion | !Seed Chemicals | action |
| reaction_diffusion | !Clear Chemicals | action |
| rivers | Rain: [0..100] | number, step 1 (default 20) |
| rivers | Flow: [0..100] | number, step 1 (default 50) |
| rivers | Erosion: [0..100] | number, step 1 (default 15) |
| rivers | Evaporation: [0..100] | number, step 1 (default 8) |
| rivers | !Rain Burst | action |
| rivers | !Reset Terrain | action |
| kuramoto | Coupling: [0..100] | number, step 1 (default 10) |
| kuramoto | Frequency Spread: [0..32] | number, step 1 (default 10) |
| kuramoto | Jitter: [0..100] | number, step 1 (default 20) |
| kuramoto | !Desync Pulse | action |
| lightning | Branch Chance: [0..100] | number, step 1 (default 25) |
| lightning | Jitter Chance: [0..100] | number, step 1 (default 20) |
| lightning | Decay Ticks: [1..16] | number, step 1 (default 4) |
| lightning | Leader Limit: [1..8] | number, step 1 (default 3) |
| lightning | Target Edge: [north, east, south, west] | enum (default south) |
| lightning | !Strike Now | action |
| wave | Damping: [0..100] | number, step 1 (default 4) |
| wave | Tension: [0..100] | number, step 1 (default 25) |
| wave | Impulse Strength: [1..127] | number, step 1 (default 80) |
| wave | Impulse Interval: [0..64] | number, step 1 (default 4; 0 disables scheduled impulses) |
| wave | Spawn Step: [0..63] | number, step 1 (default 3) |
| wave | !Drop Impulse | action |
| raindrops | Spawn Interval: [1..20] | number, step 1 (default 3) |
| raindrops | Spawn Step: [0..63] | number, step 1 (default 0) |
| raindrops | Splash Radius: [0..12] | number, step 1 (default 6) |
| raindrops | !Drop Now | action |
| coral | Growth: [0..100] | number, step 1 (default 35) |
| coral | Competition: [0..100] | number, step 1 (default 20) |
| coral | Breakaway Age: [1..64] | number, step 1 (default 30) |
| coral | !Seed Coral | action |
| coral | !Break Coral | action |
| cracks | Stress: [0..100] | number, step 1 (default 20) |
| cracks | Branch: [0..100] | number, step 1 (default 18) |
| cracks | Propagation: [0..100] | number, step 1 (default 100) |
| cracks | Shatter Threshold: [1..64] | number, step 1 (default 24) |
| cracks | !Impact | action |
| cracks | !Replace Pane | action |
| crystal_growth | Growth Chance: [0..100] | number, step 1 (default 45) |
| crystal_growth | Seed Interval: [0..64] | number, step 1 (default 16; 0 disables scheduled seeding) |
| crystal_growth | Seed Step: [0..63] | number, step 1 (default 0) |
| crystal_growth | Cell Life: [0..256] | number, step 1 (default 128); `0` disables aging/removal |
| crystal_growth | Symmetry: [cross, diagonal, snowflake] | enum (default cross) |
| crystal_growth | !Seed Crystal | action |
| dla | Spawn Interval: [1..20] | number, step 1 (default 2) |
| dla | Spawn Step: [0..63] | number, step 1 (default 0) |
| dla | Cell Life: [0..256] | number, step 1 (default 96); `0` disables aging/removal |
| dla | !Seed Cluster | action |
| physarum | Agent Count: [1..32] | number, step 1 (default 20) |
| physarum | Sense Distance: [1..3] | number, step 1 (default 1) |
| physarum | Turn Bias: [0..100] | number, step 1 (default 55) |
| physarum | Deposit Amount: [1..64] | number, step 1 (default 18) |
| physarum | Evaporation: [0..100] | number, step 1 (default 18) |
| physarum | !Relocate Food | action |
| physarum | !Seed Slime | action |
| vines | Growth: [0..100] | number, step 1 (default 55) |
| vines | Branch: [0..100] | number, step 1 (default 18) |
| vines | Prune Age: [1..64] | number, step 1 (default 24) |
| vines | Light Bias: [0..100] | number, step 1 (default 50) |
| vines | !Plant Seed | action |
| vines | !Prune Vines | action |

## Behavior notes

Event semantics that the menu rows above do not show. All coordinates are lower-left world space
(`y=0` is the bottom row); "exact cell" means the pressed world-space cell.

| Behavior | Notes |
|---|---|
| `sequencer` | A canvas of `Pages X` x `Pages Y` 8x8 pages; the grid is a viewport onto one page (`Page X`, `Page Y`) and shows, edits, and plays only that page. Pages never drawn on stay empty. `Advance` turns the view to the next page in reading order (along the row of pages, then up a row, wrapping) every `Advance` steps of the layer's Step Rate, counted from Play: steps 1 to N play the page in view and step N + 1 turns. Step Rate also paces the layer's scanning, so keep it at the scan unit: with Step Rate 1/8 and an 8-column scan, Advance 8 turns on every bar and Advance 2 on every beat. Stop or reset restarts the count; Link scanning settings and sections never change it. Changing these rows keeps the drawn pages (other behaviors re-initialize). Saves from before pages load as page 1, 1. |
| `looper` | `Punch In/Out` toggles between overdub and play, keeps the recorded loop and live playback, and toasts `Looper: Overdub` or `Looper: Play`. There is no editable mode row. |
| `bubbles` | Current, Drift, and Buoyancy use eighth-cell units per tick. `Add Bubble` spawns one bottom-origin rising bubble immediately. |
| `forest_fire` | Trees and burning cells render as visible, but events follow trigger types: tree-to-fire and manual ignition activate, burned-out cells deactivate, visible non-burning trees are stable, unrelated empty cells emit nothing. |
| `crystal_growth` | `cross` grows through cardinal neighbors, `diagonal` through diagonal neighbors, `snowflake` through cardinal neighbors plus parity-selected diagonals (even `x + y` uses NW/SE, odd uses NE/SW). A grid press seeds or refreshes the exact cell without toggling it off or changing its phase; scheduled/action seeding picks deterministic cells. |
| `lightning` | Target edges: north `y=max`, south `y=0`, east `x=max`, west `x=0`; automatic strikes start from the opposite edge. On the connection tick every visible lightning cell activates once, then stays stable during decay and deactivates when cleared. |
| `kuramoto` | Cells are visible only near the sync/wrap window. Phase wraps activate, stable sync-window cells stay quiet, and a grid press sets the exact cell just before wrap and activates it. `Desync Pulse` perturbs phases without immediate activations. |
| `wave` | Oscillating displacement/velocity, not ripple rings or diffusion. A grid press applies an impulse to the exact cell and activates it; threshold crossings activate/deactivate as the wave propagates and damps. `Impulse Interval` and `Spawn Step` schedule small deterministic impulses so the default patch keeps moving. |
| `gravity` | Falling granular sand only. Moving into a new cell activates the destination and deactivates the origin, settled grains are stable, and near-saturated or stalled defaults drain a few settled grains instead of staying full. |
| `boids` | Many quantized flocking agents. Cell entry activates; a vacated cell deactivates only when no boid remains; scatter changes velocity without an immediate activation. |
| `lava_lamp` | Soft moving metaball-like blobs that merge and split. Not flocking, orbiting, or passive diffusion. |
| `orbit` | Particles orbit one moving attractor. Particle entry activates; a vacated cell deactivates only when no particle remains; attractor-only cells are stable except press/reset forced accents. |
| `sand_ripples` | Wind-driven grain transport and migrating crests, with no water flow or gravity avalanche. Saturated defaults shed tiny deterministic gaps so dunes keep moving. |
| `ink` | Diffuses toward the cardinal-neighbor average and fades. Direct drops force activate, passive threshold crossings activate/deactivate, low pigment stays quiet. `Drop Interval` and `Spawn Step` schedule small deterministic drops. |
| `ising` | Binary magnetic domains with temperature, noise, and field bias. Flips to +1 activate, flips to -1 deactivate, unchanged +1 cells are stable, -1 cells are quiet. |
| `reaction_diffusion` | Two-chemical Gray-Scott-style integer pattern formation. B concentration drives visibility; upward threshold crossings activate, downward visibility crossings deactivate. A grid press splashes chemicals into the exact cell plus its cardinal neighbors. `Seed Interval` and `Spawn Step` schedule small deterministic splashes. |
| `rivers` | Water flow over height with erosion/deposition, cardinal non-wrapping downhill movement, and visible-water threshold triggers. |
| `cracks` | Crack tips propagate through stressed cells; new tips activate; stress-only cells are stable/quiet. Shatter/replace removes visible pane cells in bounded staged passes with deactivate triggers. |
| `coral` | Exposed cardinal colony surfaces grow, adjacent opposing colonies become skeletons instead of converting, breakaway clearing deactivates removed cells, and full defaults thin a few cells deterministically. |
| `physarum` | Bounded agents follow trail and food and deposit evaporating memory. Food is stable/quiet; Seed Slime forces new agent-cell activate accents. |
| `vines` | Directional tendril tips seek light and open space; branches reserve empty cells without wrapping; pruning deactivates removed vines; full defaults shed a few old cells deterministically; direct planting uses exact cells. |
| `fractal_explorer` | Samples Mandelbrot/Julia regions with drift and zoom. Class increases activate, class disappearance deactivates, and a grid press recenters on the exact cell with a one-shot forced activate. The mode action key is `toggleFractalMode`. |
| `maze_growth` | Carves one-cell corridors from frontiers and moves walkers over visible cells, never wrapping. Collapse/removal deactivates visible cells. |
| `predator_prey` | Grass persistence and regrowth are visible, quiet `Stable` background events. Animals activate on entry, birth, and reseed, deactivate on move-out or death, and are stable while persisting. A predator eating a herbivore adds a one-tick trigger-only cardinal burst without changing neighbor cells, so visible grass may briefly activate as accents. Saturated defaults reopen a few cells. |
| `twinkle` | Deterministic 1–5 star cap: births activate, deaths deactivate, persisting stars are stable, inactive cells are quiet. A manual birth at the cap replaces one deterministic star. One tick performs at most one eligible fade and one birth. `Star Life` gates fading; `Cluster Bias` picks clipped, non-wrapping neighborhoods over global empty cells. Missing saved cells seed the deterministic default, while an explicit empty array stays empty. `Reseed Stars` and `Clear Stars` are its only actions; Build `Reset` stays the generic reset. |
| `cyclic` | Cells chase the next discrete state through a clipped Moore neighborhood. Advancing activates, except wrap-to-zero, which deactivates. Zero cells are inactive and quiet. |
| `dla` | `Cell Life` ages out old aggregate cells so the cluster keeps renewing; `0` disables aging/removal. If aging removes the whole cluster, DLA reseeds its small starter cluster. |
