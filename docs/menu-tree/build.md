# Build Menu Tree

Part of the authoritative menu/control spec; see [`../menu-tree-spec.md`](../menu-tree-spec.md) for
the index and notation. Build is where each layer gets its world: which behavior runs, how often it
steps, and the knobs that behavior exposes.

## Tree

```
Build
├── L1: <label> (group)                          ← one group per layer; entering it makes that layer active
│   ├── Beh.: <label> (picker)                   ← browser-style selector, not an editable enum
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
│   │   ├── [Motion]
│   │   │   ├── ..
│   │   │   ├── bounce
│   │   │   ├── bubbles
│   │   │   ├── gravity
│   │   │   ├── boids
│   │   │   ├── lava lamp
│   │   │   ├── orbit
│   │   │   └── sand ripples
│   │   └── [Games]
│   │       ├── ..
│   │       ├── blockout
│   │       └── pong
│   ├── Auto Label: [on | off]                   ← on: label follows the behavior ID; off: manual text
│   ├── Layer Label: (text, max 32)              ← editing sets Auto Label off
│   ├── Step Rate: [1/32T | 1/32 | 1/16T | 1/16 | 1/8T | 1/8 | 1/4T | 1/4 | 1/2T | 1/2 | 1/1T | 1/1]  ← how often the behavior ticks, unless Link scanning is on (then Scan Unit sets it); hidden for none
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
  through the native runtime, and returns focus to the layer's `Beh.` row. The selector opens on
  the active behavior's category, and the category on the behavior. Only the affected Build
  layer rows refresh; the full menu tree is not rebuilt.
- With Auto Label on, the layer label follows the behavior ID (for example `life`, `brain`). Layer
  selectors (Fn + left column, Link layer groups) show the computed label, for example `L1: life`.
- For behavior `none` the layer group shows only Beh., Auto Label, and Layer Label. Step Rate,
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
  sequence in `savedState` as step-bucketed press/release events; live-held tiles and currently sounding
  playback tiles are not saved, and a loaded loop restarts from step 1.
- Patch-persistent, and round-tripped through preset/default/autosave payloads: Step Rate, behavior
  selection and config, Link mapping, trigger probabilities, instruments, mixer, the selected Play
  page, Play FX assignments, X/Y bindings, and all aux bindings, including ones that turn a System
  setting. Device/System settings stay local when a preset loads.
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
| Games | blockout, pong | Old arcade games played CPU against CPU: a ball, paddles that chase it, and a wall to break. |

## Behavior config rows

Rows come from each behavior's `configMenu()` in this order. `!` rows are actions; each fires the
behavior's own action key through the generic behavior-action path (see
[Inactive bindings](../menu-and-controls-spec.md#inactive-stale-bindings) for Aux press bindings).
The pattern behaviors (weave, Rhythm, Musical) label their rows in lowercase. They redraw their tiles only when the page turns, every `cycle` ticks, and each page continues the lanes where the previous one stopped (so the 5- to 15-step polyrhythm and euclid lanes play their full lengths across page turns): they are scan patterns. A settings change keeps the pattern's place in its cycle; a Rate resync restarts it on the bar.

| Behavior | Config Items | Type/Options |
|---|---|---|
| none | *(none)* | — |
| life | Spawn Count: [0..20] | number, step 1 (default 0) |
| life | Spawn Interval: [1..20] | number, step 1 (default 1) |
| life | Glider Interval: [0..20] | number, step 1 (default 0; 0 disables automatic glider injection) |
| life | Spawn Step: [0..63] | number, step 1 (default 0) |
| life | !Spawn Random | action |
| life | !Spawn Glider | action |
| sequencer | Pages X: [1..4] | number, step 1 (default 1): pages side by side on the canvas |
| sequencer | Pages Y: [1..4] | number, step 1 (default 1): pages stacked up |
| sequencer | Page X: [1..4] | number, step 1 (default 1): the column of pages in view, clamped to Pages X |
| sequencer | Page Y: [1..4] | number, step 1 (default 1): the row of pages in view, clamped to Pages Y |
| sequencer | Advance: [0..64] | number, step 1 (default 0 = off): turn to the next page every this many steps |
| keys | Quantize: [immediate, step] | enum (default immediate); `step` plays a tap released before the step for one step, and a new tap on a still-lit tile plays it again on the next step |
| looper | !Punch In/Out | action |
| looper | Length: [1..64] | number, step 1 (default 16) |
| looper | !Clear Loop | action |
| weave, Rhythm, Musical | density: [10..80] | number, step 5 (default set per behavior) |
| weave, Rhythm, Musical | variation: [0..100] | number, step 5 (default set per behavior); in polyrhythm and euclid each lane rolls five times against it, one extra hit per roll that lands |
| weave, Rhythm, Musical | cycle: [4..32] | number, step 1 (default set per behavior) |
| weave, Rhythm, Musical | seed: [1..9999] | number, step 1 (default set per behavior) |
| brain | Fire Threshold: [1..4] | number, step 1 (default 2) |
| brain | Seed Interval: [0..30] | number, step 1 (default 2; 0 disables scheduled seeding) |
| brain | Spawn Step: [0..63] | number, step 1 (default 0) |
| brain | Spawn Count: [0..20] | number, step 1 (default 2) |
| brain | !Seed Random | action |
| cyclic | States: [3..8] | number, step 1 (default 4) |
| cyclic | Threshold: [1..8] | number, step 1 (default 1); at Range 1 a side neighbor counts 2 and a corner 1, so all eight values differ |
| cyclic | Range: [1..2] | number, step 1 (default 1) |
| cyclic | !Seed Cycle | action |
| forest_fire | Tree Density: [0..100] | number, step 1 (default 34) |
| forest_fire | Grow Chance: [0..100] | number, step 1 (default 5) |
| forest_fire | Spread Chance: [0..100] | number, step 1 (default 70) |
| forest_fire | Reseed Thresh.: [0..100] | number, step 1 (default 5) |
| forest_fire | Lightning: [0..20] | number, step 1 (default 1; per-thousand chance) |
| forest_fire | !Ignite Random | action |
| predator_prey | Grass Grow: [0..100] | number, step 1 (default 45); 100 regrows a third of the empty tiles each tick |
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
| bubbles | Drift: [0..8] | number, step 1 (default 0); eighth-tile units |
| bubbles | Current: [-8..8] | number, step 1 (default 0); eighth-tile units |
| bubbles | Buoyancy: [1..8] | number, step 1 (default 3); eighth-tile units |
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
| shapes | Shape: [ring, filled, diamond, cross, x] | enum (default ring); `filled` relights its whole disk every tick it grows and starts no scheduled pulse while one is still growing, the others light only the tiles their edge newly reaches |
| shapes | Lifespan: [1..12] | number, step 1 (default 3) |
| shapes | Max Radius: [4..12] | number, step 1 (default 4) |
| shapes | Spawn Interval: [0..20] | number, step 1 (default 4; 0 disables scheduled pulses); scheduled pulses start at random tiles |
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
| ising | Field Strength: [0..100] | number, step 1 (default 5) |
| ising | Noise: [0..100] | number, step 1 (default 8) |
| ising | !Heat Pulse | action |
| ising | !Flip Field | action |
| ising | !Randomize Spins | action |
| reaction_diffusion | Feed: [0..100] | number, step 1 (default 50) |
| reaction_diffusion | Kill: [0..100] | number, step 1 (default 25) |
| reaction_diffusion | Diffusion: [0..100] | number, step 1 (default 75) |
| reaction_diffusion | Reaction: [0..100] | number, step 1 (default 50) |
| reaction_diffusion | Seed Interval: [0..64] | number, step 1 (default 8; 0 disables scheduled seeding) |
| reaction_diffusion | Spawn Step: [0..63] | number, step 1 (default 2) |
| reaction_diffusion | !Seed Chemicals | action |
| reaction_diffusion | !Clear Chemicals | action |
| rivers | Rain: [0..100] | number, step 1 (default 20) |
| rivers | Flow: [0..100] | number, step 1 (default 50) |
| rivers | Erosion: [0..100] | number, step 1 (default 15) |
| rivers | Evaporation: [0..100] | number, step 1 (default 50); 100 dries 16% of each tile's water a tick |
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
| dla | Cell Life: [0..256] | number, step 1 (default 96); `0` disables aging, and the cluster starts over once it reaches the rim |
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
| pong | CPU Skill: [0..100] | number, step 1 (default 80; the percent chance a paddle means to reach the ball) |
| pong | Paddle Size: [2..4] | number, step 1 (default 3) |
| pong | Color: [classic, lawn, sandcourt] | enum (default classic) |
| pong | !Serve | action |
| blockout | CPU Skill: [0..100] | number, step 1 (default 80) |
| blockout | Start Rows: [1..3] | number, step 1 (default 3) |
| blockout | Row Spawn: [stable, new] | enum (default new) |
| blockout | !Serve | action |
| blockout | !New Wall | action |

## Behavior notes

Event semantics that the menu rows above do not show. All coordinates are lower-left world space
(`y=0` is the bottom row); "exact tile" means the pressed world-space tile.

| Behavior | Notes |
|---|---|
| `life` | Conway's rules on a clipped, non-wrapping grid. |
| `sequencer` | A canvas of `Pages X` x `Pages Y` 8x8 pages; the grid is a viewport onto one page (`Page X`, `Page Y`) and shows, edits, and plays only that page. Pages never drawn on stay empty. `Advance` turns the view to the next page in reading order (along the row of pages, then up a row, wrapping) every `Advance` layer steps, counted from Play: steps 1 to N play the page in view and step N + 1 turns. A layer steps at its Step Rate, or at its Link `Scan Unit` while Link scanning is on (Step Rate is then unused): with Scan Unit 1/8 and an 8-column scan, Advance 8 turns on every bar and Advance 2 on every beat. Stop or reset restarts the count and, while `Advance` is on, turns the view back to page 1, 1; Link scanning settings and sections never change it. Changing these rows keeps the drawn pages (other behaviors re-initialize). Saves from before pages load as page 1, 1. |
| `looper` | `Punch In/Out` toggles between overdub and play, keeps the recorded loop and live playback, and toasts `Looper: Overdub` or `Looper: Play`. There is no editable mode row. Grid presses and keyboard notes record on the same step: while playing, a press in the first half of a step lands on that step and plays from the next pass; one in the second half lands on the next step, which skips it once on this pass so it does not hit twice. Play primes the Looper, so step 1 sounds at Play. Stopped or paused, it lands on the step the playhead waits on (single-step moves it). A release that would share its press's step moves one step later. |
| `bubbles` | Current, Drift, and Buoyancy use eighth-tile units per tick. `Add Bubble` spawns one bottom-origin rising bubble immediately. |
| `forest_fire` | Trees and burning cells render as visible, but events follow trigger types: tree-to-fire and manual ignition activate, burned-out cells deactivate, visible non-burning trees are stable, unrelated empty cells emit nothing. |
| `crystal_growth` | `cross` grows through cardinal neighbors, `diagonal` through diagonal neighbors, `snowflake` through cardinal neighbors plus parity-selected diagonals (even `x + y` uses NW/SE, odd uses NE/SW). A grid press seeds or refreshes the exact cell without toggling it off or changing its phase; scheduled/action seeding picks deterministic cells. A crystal grows into its neighbors only for its first 8 ticks, and crystals grow and seeds land only in tiles that were already empty when the tick began, so a dissolved crystal leaves its hole open until a seed lands near it, and growth always leaves one tile open. Old crystals age out after `Cell Life` ticks. A nearly full grid (fewer than 4 empty tiles) breaks off one crystal every 3 ticks, and a grid a seed or press filled completely breaks one off at once, so with the default `Cell Life` about half the crystals that disappear have aged out. |
| `lightning` | Target edges: north `y=max`, south `y=0`, east `x=max`, west `x=0`; automatic strikes start from the opposite edge. On the connection tick every visible lightning tile activates once, then stays stable during decay and deactivates when cleared. |
| `kuramoto` | Tiles are visible only near the sync/wrap window. Phase wraps activate, stable sync-window tiles stay quiet, and a grid press sets the exact tile just before wrap and activates it. `Desync Pulse` perturbs phases without immediate activations. |
| `wave` | Oscillating displacement/velocity, not ripple rings or diffusion. A grid press applies an impulse to the exact tile and activates it; threshold crossings activate/deactivate as the wave propagates and damps. `Impulse Interval` and `Spawn Step` schedule small deterministic impulses so the default patch keeps moving. |
| `gravity` | Falling granular sand only. Moving into a new tile activates the destination and deactivates the origin, settled grains are stable. The far edge stops pouring while three quarters of the grid holds sand, a pile over half the grid lets its settled floor grains fall out, and a pile where nothing moves drains one settled grain a step, so the grid never fills. Spawn and slide rolls come from a well-mixed per-tile hash, so grains no longer fall in bars. |
| `boids` | Many quantized flocking agents. Tile entry activates; a vacated tile deactivates only when no boid remains; scatter changes velocity without an immediate activation. |
| `lava_lamp` | Soft moving metaball-like blobs that merge and split. Not flocking, orbiting, or passive diffusion. `Heat` warms the bottom third, so blobs there rise; the top third cools them so they sink, and in between a blob keeps drifting its way. |
| `orbit` | Particles orbit one moving attractor. Particle entry activates; a vacated tile deactivates only when no particle remains; attractor-only tiles are stable except press/reset forced accents. |
| `sand_ripples` | Wind-driven grain transport and migrating crests, with no water flow or gravity avalanche. `Deposition` is the chance that blown sand raises the crest it lands on to the height of the tile it blew from, so high values leave standing ridges downwind. Sand blown past the downwind edge leaves the grid, and while fewer than 16 tiles show sand a fresh dune drops in, so the field neither fills nor empties. |
| `ink` | Diffuses toward the cardinal-neighbor average and fades. Direct drops force activate, passive threshold crossings activate/deactivate, low pigment stays quiet. `Drop Interval` and `Spawn Step` schedule small deterministic drops. |
| `ising` | Binary magnetic domains with temperature, noise, and field bias. Each tick updates one checkerboard half of the grid with the Metropolis rule, so domains grow and break instead of strobing; Temperature 35 sits near the critical point. Flips to +1 activate, flips to -1 deactivate, unchanged +1 tiles are stable, -1 tiles are quiet. `Randomize Spins` scatters a fresh random mix. |
| `reaction_diffusion` | Two-chemical Gray-Scott pattern formation. Each knob spans the band where an 8x8 grid keeps its spots: Feed 0 to 100 is a feed rate of 0.020 to 0.100, Kill a kill rate of 0.052 to 0.092, Diffusion chemical A's rate of 0 to 0.8 (B diffuses a third as fast), and Reaction scales the reaction term 0.7 to 1.3 times (50 is the plain term). The defaults are the rates 0.060 and 0.062. B concentration drives visibility; upward threshold crossings activate, downward visibility crossings deactivate. A grid press splashes chemicals into the exact tile plus its cardinal neighbors. `Seed Interval` and `Spawn Step` schedule small deterministic splashes. |
| `rivers` | Water flow over height with erosion/deposition, cardinal non-wrapping downhill movement, and visible-water threshold triggers. The terrain is high along the top row and falls toward the bottom row, where water with nowhere lower to go spills off the panel. A fresh world starts damp, just under visible, and only gathered water shows: the streams. `Rain Burst` rains on the top rows. |
| `cracks` | Crack tips propagate through stressed tiles; new tips activate; stress-only tiles are stable/quiet. A tip with no open neighbor settles into the crack, so the next impact can land. Shatter/replace removes visible pane tiles in bounded staged passes with deactivate triggers. |
| `coral` | Exposed cardinal colony surfaces grow, adjacent opposing colonies become skeletons instead of converting, and living coral bleaches into skeleton after 32 ticks. Only living coral is lit: becoming skeleton deactivates a tile, and a skeleton blocks growth, dark and silent, until it breaks away after `Breakaway Age`. A reef with no living coral left is seeded again along the bottom edge. |
| `physarum` | Bounded agents follow trail and food and deposit evaporating memory. Food is stable/quiet; Seed Slime forces new agent-tile activate accents. |
| `vines` | Directional tendril tips seek light and open space; branches reserve empty tiles without wrapping; pruning deactivates removed vines; full defaults shed a few old tiles deterministically; direct planting uses exact tiles. |
| `fractal_explorer` | Samples Mandelbrot/Julia regions with drift and zoom. A dive that zooms past 512, where the grid runs out of detail, flies on to the next region and starts over from the wide view, as Jump Region does. Class increases activate, class disappearance deactivates, and a grid press recenters on the exact tile with a one-shot forced activate. The mode action key is `toggleFractalMode`. |
| `maze_growth` | Carves one-tile corridors from frontiers and moves walkers over visible tiles, never wrapping. Collapse/removal deactivates visible tiles. `Restart Maze` places up to 8 walkers around the starting corridor; whenever fewer than `Walker Count` walk (one was pressed into a wall, or the count went up), the missing ones appear on open corridor tiles picked by the layer's random stream (never on a frontier, which keeps carving). |
| `predator_prey` | Grass persistence and regrowth are visible, quiet `Stable` background events. Animals activate on entry, birth, and reseed, deactivate on move-out or death, and are stable while persisting. A predator eating a herbivore adds a one-tick trigger-only cardinal burst without changing neighbor tiles, so visible grass may briefly activate as accents. A herbivore or predator that dies out is reseeded on a random open tile. |
| `twinkle` | Deterministic 1–5 star cap: births activate, deaths deactivate, persisting stars are stable, inactive tiles are quiet. A manual birth at the cap replaces one deterministic star. One tick performs at most one eligible fade and one birth. `Star Life` gates fading; `Cluster Bias` picks clipped, non-wrapping neighborhoods over global empty tiles. Missing saved tiles seed the deterministic default, while an explicit empty array stays empty. `Reseed Stars` and `Clear Stars` are its only actions; Build `Reset` stays the generic reset. |
| `cyclic` | Cells chase the next discrete state through a clipped Moore neighborhood. Only state 1, the front a resting cell moves into, is lit: entering it activates, leaving it deactivates. A press sets its tile to the front and activates it, even if it already was one. Each tick one random cell takes a random state, so the waves never settle into a fixed loop. `Seed Cycle` scatters every state at random; a world where no cell has moved by the rules for 8 ticks scatters itself again. |
| `pong` | Paddles fill `x=0` and `x=7`, `Paddle Size` tiles tall. The ball moves one column a step plus its slope (-1, 0 or +1) and turns at the top and bottom walls. The receiving CPU predicts where the ball lands and moves at most one tile a step toward the paddle tile it picked for that approach; with a `100 - CPU Skill` percent chance it means to miss. An end-tile hit sends the ball toward that end, a middle hit sends it back flat. A missed ball enters the empty paddle-column tile and scores for the other side (the OLED shows `left : right`), then leaves; after two empty steps the next ball serves from the middle toward the scorer. The ball's new tile activates and its old tile deactivates; paddles are stable, a paddle step activates its new end tile and deactivates the old one, and on a hit the struck paddle tile activates again while it stays lit. A grid press moves the paddle on the pressed half one tile toward the pressed row. `Color` only changes the colors: `classic` is white on black, `lawn` and `sandcourt` put a yellow ball and white paddles on a dark green or reddish-orange court. |
| `blockout` | A 3-tile CPU paddle on the bottom row and `Start Rows` full rows of blocks at the top. The ball moves one row a step plus its sideways slope, turns at the side and top walls, and breaks the block it would enter and turns back. While the ball rises the paddle follows its column; while it falls the paddle aims like pong's CPU (`100 - CPU Skill` percent miss chance), and an end tile angles the ball while the middle sends it straight up. A broken block stays lit in its row color for one more step and activates, then deactivates as it clears (unless the tile is still a block after a row drop). A row broken clear leaves the wall and a full row drops in at the top, pushing the rows above the cleared one down a row: pushed blocks make no events, and the new row activates (`Row Spawn` `new`) or arrives as stable tiles (`stable`). The ball's path and the paddle's ends activate and deactivate like pong's, the struck paddle tile activates again on a hit, and the rest of the paddle and the wall are stable. Each row keeps its own color (red, orange, green, cyan, blue, violet, cycling as rows arrive) with a white paddle and a yellow ball on dark blue. A dropped ball enters the bottom row, leaves, and after two empty steps serves from just above the paddle. A grid press on an empty block inside the wall rows fills it (activate); a press below the wall moves the paddle one tile toward the pressed column. `New Wall` rebuilds the starting wall and serves; Build `Reset` also zeroes the cleared-row count (OLED `B:<blocks> R:<rows cleared>`). |
| `dla` | Each spawn releases a random walker from a free rim tile (any free tile once the rim is full) that wanders until it sits next to the cluster and sticks; up to 8 walkers try per spawn. A cluster of 50 cells releases no walker until aging thins it, so a long `Cell Life` or a short `Spawn Interval` stops short of filling the grid. `Cell Life` ages out old aggregate cells so the cluster keeps renewing; `0` disables aging/removal, and then a cluster that reaches the rim starts over from the starter cluster at the next spawn. If aging removes the whole cluster, DLA reseeds its small starter cluster. |
