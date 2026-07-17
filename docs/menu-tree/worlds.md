# Build Menu Tree

This file is part of the canonical split-out menu tree spec. See [`../menu-tree-spec.md`](../menu-tree-spec.md) for the canonical index.

### Build

```
Build
├── L1: ... (group)                              ← one group per layer, label computed from the layer label
│   ├── Behavior: <id> (group)                   ← browser-style selector for this layer's behavior
│   │   ├── [Human]
│   │   │   ├── ..
│   │   │   ├── keys
│   │   │   ├── looper
│   │   │   ├── none
│   │   │   └── sequencer
│   │   ├── [Cellular]
│   │   │   ├── ..
│   │   │   ├── ant
│   │   │   ├── brain
│   │   │   ├── cyclic
│   │   │   ├── forest_fire
│   │   │   ├── life
│   │   │   └── predator_prey
│   │   ├── [Fields]
│   │   │   ├── ..
│   │   │   ├── ink
│   │   │   ├── ising
│   │   │   ├── kuramoto
│   │   │   ├── lightning
│   │   │   ├── raindrops
│   │   │   ├── reaction_diffusion
│   │   │   ├── rivers
│   │   │   └── wave
│   │   ├── [Geometry]
│   │   │   ├── ..
│   │   │   ├── fractal_explorer
│   │   │   ├── maze_growth
│   │   │   └── shapes
│   │   ├── [Growth]
│   │   │   ├── ..
│   │   │   ├── coral
│   │   │   ├── cracks
│   │   │   ├── crystal_growth
│   │   │   ├── dla
│   │   │   ├── physarum
│   │   │   └── vines
│   │   ├── [Motion]
│   │   │   ├── ..
│   │   │   ├── bounce
│   │   │   ├── bubbles
│   │   │   ├── gravity
│   │   │   ├── boids
│   │   │   ├── lava_lamp
│   │   │   ├── orbit
│   │   │   └── sand_ripples
│   ├── Auto Label: [on | off]                   ← on: label auto-derives from behavior ID; off: label is manual text
│   ├── Layer Label: (text, max 32)               ← display label; editing sets Auto Label off
│   ├── Step Rate: [1/32T, 1/32, 1/16T, 1/16, 1/8T, 1/8, 1/4T, 1/4, 1/2T, 1/2, 1/1T, 1/1]   ← controls how often onTick() is called; hidden when Behavior is `none`
│   ├── ... per-behavior dynamic config from behavior's configMenu()
│   └── Reset                                    ← reinitializes the active behavior state; hidden when Behavior is `none`
├── L2: ... (group)
└── L3: ... (group)                              ← up to layerCount layers total
```

Rows that open submenus or selectors render with a trailing `>`. Selecting a behavior row switches that layer immediately through the native runtime and returns focus to the layer's Behavior row. It does not rebuild the full menu tree; only the affected Build layer rows are refreshed. Behavior IDs remain the persisted payload values under each layer's `behaviorId`.
`glider` is no longer selectable. Its glider injection controls are part of `life`.
When Auto Label is on, the layer label is derived from the active behavior ID (e.g. `life`, `brain`). Editing the Layer Label text field switches Auto Label off.
Layer selectors (Fn+column selection, Link Layer selector) display the computed layer label (e.g. `L1: life`, `L2: rain`).
When a layer's behavior is `none`, the Build layer group shows Behavior, Auto Label, and Layer Label only; Step Rate, dynamic behavior config rows, and Reset are hidden without deleting stored values.
Parameter target pickers mirror the main menu root order (`Build`, `Link`, `Shape`, `Play`, `System`). Within `Build`, behavior `none` layers expose no Behavior targets, while real behavior layers expose `layers.N.algorithmStep` and `layers.N.worlds.behaviorConfig.*` targets under their own layer label.

Behavior categories:

| Category | Behaviors | Description |
|---|---|---|
| Human | keys, looper, none, sequencer | Direct performance, recording, silence, or step-style behaviors. |
| Cellular | ant, brain, cyclic, forest_fire, life, predator_prey | Cell-state simulations where neighboring cells or agents create evolving patterns. |
| Fields | ink, ising, kuramoto, lightning, raindrops, reaction_diffusion, rivers, wave | Field-style activity that spreads from localized events. |
| Geometry | fractal_explorer, maze_growth, shapes | Dynamic fractal exploration, maze carving, and explicit geometric pulse patterns. |
| Growth | coral, cracks, crystal_growth, dla, physarum, vines | Coral, crack, crystal, slime, vine, and diffusion-limited clusters that grow from seeded particles. |
| Motion | bounce, bubbles, gravity, boids, lava_lamp, orbit, sand_ripples | Moving objects, blobs, flocking/orbiting agents, bubbles, dunes, and granular avalanches through the grid. |

Behavior-specific config items (from `configMenu()`):

| Behavior | Config Items | Type/Options |
|---|---|---|
| none | *(none)* | — |
| life | Spawn Count: [0..20] | number, step 1 (default 12) |
| life | Spawn Interval: [1..20] | number, step 1 (default 1) |
| life | Glider Interval: [0..20] | number, step 1 (default 0; 0 disables automatic glider injection) |
| life | Spawn Step: [0..63] | number, step 1 |
| life | !Spawn Random | action, shared route `trigger.life.spawn_now` |
| life | !Spawn Glider | action, shared route `trigger.life.spawn_now` |
| sequencer | *(none)* | — |
| keys | Quantize: [immediate, step] | enum |
| looper | !Punch In/Out | action |
| looper | Length: [1..64] | number, step 1 (default 16) |
| looper | !Clear Loop | action |
| brain | Fire Threshold: [1..6] | number, step 1 |
| brain | !Seed Random | action, shared route `trigger.life.spawn_now` |
| cyclic | States: [3..8] | number, step 1 (default 4) |
| cyclic | Threshold: [1..8] | number, step 1 (default 2) |
| cyclic | Range: [1..2] | number, step 1 (default 1) |
| cyclic | !Seed Cycle | action, shared route `trigger.life.spawn_now` |
| forest_fire | Tree Density: [0..100] | number, step 1 (default 34) |
| forest_fire | Grow Chance: [0..100] | number, step 1 (default 5) |
| forest_fire | Spread Chance: [0..100] | number, step 1 (default 70) |
| forest_fire | Reseed Threshold: [0..100] | number, step 1 (default 5) |
| forest_fire | Lightning: [0..20] | number, step 1 (default 1; per-thousand chance) |
| forest_fire | !Ignite Random | action, shared route `trigger.life.spawn_now` |
| predator_prey | Grass Grow: [0..100] | number, step 1 (default 15) |
| predator_prey | Herbivore Repro: [0..100] | number, step 1 (default 15) |
| predator_prey | Predator Repro: [0..100] | number, step 1 (default 8) |
| predator_prey | Starve Ticks: [1..32] | number, step 1 (default 8) |
| predator_prey | !Reseed Ecosystem | action, shared route `trigger.life.spawn_now` |
| ant | Max Ants: [1..10] | number, step 1 |
| ant | !Spawn Ant | action, shared route `trigger.life.spawn_now` |
| bounce | Max Balls: [1..20] | number, step 1 |
| bounce | !Add Ball | action, shared route `trigger.life.spawn_now` |
| bubbles | Spawn Interval: [0..30] | number, step 1 |
| bubbles | Spawn Step: [0..63] | number, step 1 |
| bubbles | Spawn Count: [1..8] | number, step 1 |
| bubbles | Min Radius: [1..4] | number, step 1 |
| bubbles | Max Radius: [1..4] | number, step 1 |
| bubbles | Drift: [0..8] | number, step 1; eighth-cell units |
| bubbles | Current: [-8..8] | number, step 1; eighth-cell units |
| bubbles | Buoyancy: [1..8] | number, step 1; eighth-cell units |
| bubbles | Max Bubbles: [1..64] | number, step 1 |
| bubbles | !Add Bubble | action, shared route `trigger.life.spawn_now` |
| gravity | Spawn Rate: [0..100] | number, step 1 (default 20) |
| gravity | Slide Chance: [0..100] | number, step 1 (default 60) |
| gravity | Settle Age: [1..32] | number, step 1 (default 8) |
| gravity | Gravity Dir: [down, left, up, right] | enum |
| gravity | !Drop Sand | action, shared route `trigger.life.spawn_now` |
| gravity | !Clear Bottom | action, shared route `trigger.life.spawn_now` |
| gravity | !Invert Gravity | action, shared route `trigger.life.spawn_now` |
| boids | Flock Size: [1..24] | number, step 1 (default 12) |
| boids | Separation: [0..100] | number, step 1 (default 45) |
| boids | Alignment: [0..100] | number, step 1 (default 35) |
| boids | Cohesion: [0..100] | number, step 1 (default 25) |
| boids | !Scatter Flock | action, shared route `trigger.life.spawn_now` |
| boids | !Seed Flock | action, shared route `trigger.life.spawn_now` |
| orbit | Particle Count: [1..16] | number, step 1 (default 8) |
| orbit | Attraction: [0..100] | number, step 1 (default 45) |
| orbit | Orbit: [0..100] | number, step 1 (default 55) |
| orbit | Repel Mode: [off, always] | enum |
| orbit | !Reset Orbit | action, shared route `trigger.life.spawn_now` |
| orbit | !Nudge Attractor | action, shared route `trigger.life.spawn_now` |
| lava_lamp | Blob Count: [1..8] | number, step 1 (default 4) |
| lava_lamp | Viscosity: [0..100] | number, step 1 (default 40) |
| lava_lamp | Heat: [0..100] | number, step 1 (default 35) |
| lava_lamp | Merge: [0..100] | number, step 1 (default 25) |
| lava_lamp | !Heat Lamp | action, shared route `trigger.life.spawn_now` |
| lava_lamp | !Reset Blobs | action, shared route `trigger.life.spawn_now` |
| sand_ripples | Wind Strength: [0..100] | number, step 1 (default 45) |
| sand_ripples | Deposition: [0..100] | number, step 1 (default 35) |
| sand_ripples | Erosion: [0..100] | number, step 1 (default 25) |
| sand_ripples | !Gust | action, shared route `trigger.life.spawn_now` |
| sand_ripples | !Shift Wind | action, shared route `trigger.life.spawn_now` |
| sand_ripples | !Seed Dunes | action, shared route `trigger.life.spawn_now` |
| shapes | Shape: [ring, heart, star, plus, x] | enum |
| shapes | Expansion Speed: [1..5] | number, step 1 |
| shapes | Auto Spawn Int: [0=off, 10, 20, 50] | enum |
| shapes | !Spawn Pulse | action, shared route `trigger.life.spawn_now` |
| fractal_explorer | Zoom Rate: [0..100] | number, step 1 (default 8) |
| fractal_explorer | Drift: [0..100] | number, step 1 (default 20) |
| fractal_explorer | Iteration Limit: [8..64] | number, step 1 (default 24) |
| fractal_explorer | Fractal Mode: [mandelbrot, julia] | enum |
| fractal_explorer | !Jump Region | action, shared route `trigger.life.spawn_now` |
| fractal_explorer | !Toggle Fractal Mode | action, shared route `trigger.life.spawn_now` |
| maze_growth | Carve: [0..100] | number, step 1 (default 55) |
| maze_growth | Collapse Age: [1..64] | number, step 1 (default 32) |
| maze_growth | Walker Count: [1..8] | number, step 1 (default 2) |
| maze_growth | !Restart Maze | action, shared route `trigger.life.spawn_now` |
| maze_growth | !Collapse Maze | action, shared route `trigger.life.spawn_now` |
| ink | Diffusion: [0..100] | number, step 1 (default 30) |
| ink | Fade: [0..100] | number, step 1 (default 8) |
| ink | Drop Strength: [1..255] | number, step 1 (default 180) |
| ink | !Drop Ink | action, shared route `trigger.life.spawn_now` |
| ink | !Clear Ink | action, shared route `trigger.life.spawn_now` |
| ising | Temperature: [0..100] | number, step 1 (default 35) |
| ising | Field Strength: [0..100] | number, step 1 (default 15) |
| ising | Noise: [0..100] | number, step 1 (default 8) |
| ising | !Heat Pulse | action, shared route `trigger.life.spawn_now` |
| ising | !Flip Field | action, shared route `trigger.life.spawn_now` |
| ising | !Randomize Spins | action, shared route `trigger.life.spawn_now` |
| reaction_diffusion | Feed: [0..100] | number, step 1 (default 35) |
| reaction_diffusion | Kill: [0..100] | number, step 1 (default 55) |
| reaction_diffusion | Diffusion: [0..100] | number, step 1 (default 35) |
| reaction_diffusion | Reaction: [0..100] | number, step 1 (default 50) |
| reaction_diffusion | !Seed Chemicals | action, shared route `trigger.life.spawn_now` |
| reaction_diffusion | !Clear Chemicals | action, shared route `trigger.life.spawn_now` |
| rivers | Rain: [0..100] | number, step 1 (default 20) |
| rivers | Flow: [0..100] | number, step 1 (default 50) |
| rivers | Erosion: [0..100] | number, step 1 (default 15) |
| rivers | Evaporation: [0..100] | number, step 1 (default 8) |
| rivers | !Rain Burst | action, shared route `trigger.life.spawn_now` |
| rivers | !Reset Terrain | action, shared route `trigger.life.spawn_now` |
| kuramoto | Coupling: [0..100] | number, step 1 (default 35) |
| kuramoto | Frequency Spread: [0..32] | number, step 1 (default 10) |
| kuramoto | Jitter: [0..100] | number, step 1 (default 3) |
| kuramoto | !Desync Pulse | action, shared route `trigger.life.spawn_now` |
| lightning | Branch Chance: [0..100] | number, step 1 (default 25) |
| lightning | Jitter Chance: [0..100] | number, step 1 (default 20) |
| lightning | Decay Ticks: [1..16] | number, step 1 (default 4) |
| lightning | Leader Limit: [1..8] | number, step 1 (default 3) |
| lightning | Target Edge: [north, east, south, west] | enum (default south) |
| lightning | !Strike Now | action, shared route `trigger.life.spawn_now` |
| wave | Damping: [0..100] | number, step 1 (default 14) |
| wave | Tension: [0..100] | number, step 1 (default 45) |
| wave | Impulse Strength: [1..127] | number, step 1 (default 80) |
| wave | !Drop Impulse | action, shared route `trigger.life.spawn_now` |
| raindrops | !Drop Now | action, shared route `trigger.life.spawn_now` |
| cracks | Stress: [0..100] | number, step 1 (default 20) |
| coral | Growth: [0..100] | number, step 1 (default 35) |
| coral | Competition: [0..100] | number, step 1 (default 20) |
| coral | Breakaway Age: [1..64] | number, step 1 (default 30) |
| coral | !Seed Coral | action, shared route `trigger.life.spawn_now` |
| coral | !Break Coral | action, shared route `trigger.life.spawn_now` |
| cracks | Branch: [0..100] | number, step 1 (default 18) |
| cracks | Propagation: [0..100] | number, step 1 (default 65) |
| cracks | Shatter Threshold: [1..64] | number, step 1 (default 24) |
| cracks | !Impact | action, shared route `trigger.life.spawn_now` |
| cracks | !Replace Pane | action, shared route `trigger.life.spawn_now` |
| crystal_growth | Growth Chance: [0..100] | number, step 1 (default 45) |
| crystal_growth | Seed Interval: [0..64] | number, step 1 (default 16; 0 disables scheduled seeding) |
| crystal_growth | Seed Step: [0..63] | number, step 1 |
| crystal_growth | Cell Life: [0..256] | number, step 1; `0` disables aging/removal |
| crystal_growth | Symmetry: [cross, diagonal, snowflake] | enum |
| crystal_growth | !Seed Crystal | action, shared route `trigger.life.spawn_now` |
| dla | Spawn Interval: [1..20] | number, step 1 |
| dla | Spawn Step: [0..63] | number, step 1 |
| dla | Cell Life: [0..256] | number, step 1; `0` disables DLA aging/removal |
| dla | !Seed Cluster | action, shared route `trigger.life.spawn_now` |
| physarum | Agent Count: [1..32] | number, step 1 (default 20) |
| physarum | Sense Distance: [1..3] | number, step 1 (default 1) |
| physarum | Turn Bias: [0..100] | number, step 1 (default 45) |
| physarum | Deposit Amount: [1..64] | number, step 1 (default 24) |
| physarum | Evaporation: [0..100] | number, step 1 (default 10) |
| physarum | !Relocate Food | action, shared route `trigger.life.spawn_now` |
| physarum | !Seed Slime | action, shared route `trigger.life.spawn_now` |
| vines | Growth: [0..100] | number, step 1 (default 55) |
| vines | Branch: [0..100] | number, step 1 (default 18) |
| vines | Prune Age: [1..64] | number, step 1 (default 24) |
| vines | Light Bias: [0..100] | number, step 1 (default 50) |
| vines | !Plant Seed | action, shared route `trigger.life.spawn_now` |
| vines | !Prune Vines | action, shared route `trigger.life.spawn_now` |
