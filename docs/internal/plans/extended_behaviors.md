# Octessera Behavior Roadmap

## Purpose

Octessera is an instrument for **performing with emergent systems**.

Its behaviors should not merely generate random notes or draw static patterns. Each behavior should create a small world with simple rules, recognizable internal logic, useful musical motion, and enough renewal that it remains interesting over time.

The core formula is:

> generative behavior + enough structure for listenability + live intervention = emergent music

This document proposes:

- a category tree for the behavior menu
- priorities for new behaviors
- transience and reset strategies
- computational constraints
- a review of existing behaviors
- recommendations for avoiding redundant additions

---

# 1. Core Selection Criteria

A behavior is a strong fit for Octessera when it satisfies most of the following:

## 1.1 Distinct Musical Character

It should produce a kind of motion, rhythm, density, or interaction that is meaningfully different from existing behaviors.

Adding another academically interesting algorithm is not enough if it feels nearly identical in use.

## 1.2 Simple Underlying Rules

The behavior should be explainable in a short paragraph.

Complexity should emerge from the interaction of simple rules rather than from a large number of special cases.

## 1.3 Computationally Bounded

The device is also a synthesizer. Behavior simulation must remain lightweight and predictable.

A behavior should have:

- fixed or bounded memory use
- fixed or bounded work per tick
- no unbounded searches
- no uncontrolled recursion
- no dependence on high-resolution rendering
- no simulation work in the realtime audio callback

## 1.4 Transience

The world should not merely fill up and remain full.

Structures should be able to:

- appear
- evolve
- interact
- decay
- disappear
- be replaced

## 1.5 Natural or Designed Renewal

A behavior may renew itself through:

- native birth/death rules
- fading or evaporation
- cell lifetime
- erosion
- destructive agents
- competing instances
- parity or XOR combination
- milestone-triggered reset
- partial reset with preserved fragments
- phase changes

## 1.6 Meaningful Interaction

Octessera already allows parameters and actions to be mapped flexibly.

The relevant question is therefore not whether interaction is possible, but whether the behavior exposes parameters whose changes create interesting musical consequences.

---

# 2. Computational Model

## 2.1 Recommended Update Rates

Behavior simulation should generally run at control rate rather than audio rate.

Typical ranges:

- 5–15 Hz for slow growth systems
- 10–30 Hz for most cellular and agent systems
- up to 60 Hz for motion-heavy systems

## 2.2 Recommended Limits

Suggested default ceilings:

- internal grid: 8×8 by default
- higher-resolution internal field: only when clearly beneficial
- agents: usually 8–64
- attractors or food sources: small fixed count
- bounded steps per walker per tick
- no heap allocation during steady-state updates

## 2.3 Internal Resolution

An 8×8 simulation is ideal when the behavior remains legible at that scale.

A 16×16 or 32×32 internal field may be justified for:

- reaction–diffusion
- wave propagation
- fractal sampling
- slime-mold trail fields

The internal field should still be sampled or reduced to the 8×8 board.

## 2.4 Audio Safety

Behavior simulation should remain outside the realtime audio callback.

The audio engine should consume:

- cell-state snapshots
- trigger events
- note events
- modulation values
- compact shared state

The simulation must avoid realtime hazards such as allocation, locking, or unpredictable loops.

---

# 3. Transience and Renewal

## 3.1 Native Transience

Some systems already contain construction and destruction.

Examples:

- Brian's Brain
- Forest Fire
- Wave Equation with damping
- Slime Mold with evaporation
- Predator–Prey
- Cyclic Cellular Automata
- Pulse-coupled oscillators

These are especially desirable because renewal is part of the world itself.

## 3.2 Cell Lifetime

Each created cell receives a finite lifetime.

When its time-to-live expires, it disappears.

Best for:

- Shapes
- Crystal Growth
- Vines
- Fractal samples
- persistent particle trails

## 3.3 Age-Based Erosion

Older cells become increasingly likely to disappear.

This is less mechanical than a fixed lifetime and creates gradual renewal.

## 3.4 XOR or Toggle Combination

A new event toggles a cell instead of always setting it.

Repeated or overlapping activity can therefore remove cells.

This is especially useful for:

- Shapes
- line-based growth
- repeated geometric generators
- overlapping pattern systems
- dual-source simulations

## 3.5 Even/Odd Parity

Count how many influences affect a cell.

- odd count: active
- even count: inactive

This makes overlap destructive as well as constructive.

## 3.6 Competing Instances

Run two instances of the same or related process.

One creates; the other removes.

The destructive instance may be:

- mirrored
- phase-shifted
- delayed
- offset
- independently seeded
- slower or faster

This is a strong option when the original algorithm is attractive but accumulative.

## 3.7 Destructive Agents

Add walkers, predators, erasers, or waves that remove existing cells.

Examples:

- eraser ants
- sweeping fronts
- decay particles
- predators
- cutting cracks
- dissolving droplets

## 3.8 Density-Based Death

Overcrowded areas erode or collapse.

This prevents the board from saturating.

## 3.9 Resource Depletion

Cells consume a local resource.

When the resource is exhausted, cells die.

The resource then regenerates.

This works particularly well for:

- vines
- coral
- ecosystems
- slime mold
- crystal growth

## 3.10 Alternating Phases

The world alternates between phases such as:

- growth
- stability
- erosion
- reseeding

This can be deterministic or condition-driven.

## 3.11 Pattern-Based Erasure

Inject destructive patterns:

- waves
- circles
- lines
- masks
- gliders
- expanding voids

These create renewal while keeping the world's existing structure partially intact.

---

# 4. World Events and Conditional Reset

A reset should preferably happen because the world reached a meaningful state, not merely because a timer expired.

The reset becomes part of the behavior's narrative.

## 4.1 Full Reset

The entire state is regenerated.

Examples:

- Cracks: a crack spans the pane, so the window is replaced
- Maze: the maze completes, so a new maze begins
- Fractal Explorer: the view becomes uninformative, so a new region is selected
- Forest Fire: nearly all vegetation is gone, so a new season begins

## 4.2 Partial Reset

Only part of the world is replaced.

Examples:

- DLA: preserve one fragment, regrow from it
- Coral: a storm removes large sections
- Physarum: relocate food sources but keep some trails
- Gravity: clear only the settled bottom rows
- Crystal Growth: dissolve one region and reseed another

## 4.3 Phase Transition

The world changes mode instead of resetting.

Examples:

- Cracks: intact glass → cracking → shatter → new pane
- Forest Fire: growth season → fire season → recovery
- Ecosystem: abundance → collapse → regrowth
- Wave: excitation → interference → calm → new impulse
- Fractal Explorer: zoom-in → drift → zoom-out → teleport

## 4.4 Suggested World Events

| Behavior | Condition | Event |
|---|---|---|
| Cracks | Crack connects opposite edges | Glass shatters; replace pane |
| Forest Fire | Tree density falls below threshold | Begin new season |
| Boids | Flock becomes too compact | Scatter event or predator |
| Physarum | Food exhausted or trail field saturates | Relocate food sources |
| DLA | Aggregate reaches edge or density limit | Fracture, erode, or reseed |
| Gravity | Stable pile reaches height threshold | Avalanche or clear zone |
| Brain | Activity falls below threshold | Random excitation |
| Life | Extinction or stable repetition detected | Reseed or inject pattern |
| Wave | Total energy falls below threshold | New impulse |
| Crystals | Crystal covers too much of board | Dissolve selected region |
| Predator–Prey | One population collapses | Reintroduce population |
| Fractal Explorer | Detail score becomes too low/high | Shift center or reset zoom |

---

# 5. Existing Behaviors: Keep, Merge, or Reconsider

Current behaviors:

- None
- Life
- Sequencer
- Keys
- Looper
- Brain
- Cyclic
- Ant
- Bounce
- Bubbles
- Gravity
- Boids
- Orbit
- Shapes
- Raindrops
- DLA
- Forest Fire
- Predator–Prey
- Kuramoto
- Wave
- Ink
- Ising
- Coral
- Cracks
- Crystal Growth
- Physarum
- Vines
- Fractal Explorer
- Maze Growth
- Lightning
- Reaction–Diffusion
- Rivers
- Sand Ripples
- Lava Lamp

## 5.1 Definitely Keep

### None

A true null behavior.

It performs no simulation, no processing, and no autonomous activity.

It is an empty layer, not a manual-performance mode.

### Keys

Meaningfully distinct because it is direct live performance rather than autonomous generation.

### Sequencer

Provides deliberate rhythmic structure and listenability.

It serves a different role from generative behaviors.

### Looper

Distinct because it works with captured performance and repetition rather than simulated state.

### Ant

Langton's Ant has a clear identity:

- one moving agent
- deterministic local transformation
- path-dependent structure
- strong long-term evolution

It is meaningfully distinct from Life and Brain.

### Bounce

Distinct physical motion.

Its identity is trajectory, collision, and reflection rather than cellular evolution.

### Raindrops

Distinct if implemented as falling droplets followed by local splashes.

It should remain focused on impact and splash lifecycle, not general wave interference.

### DLA

Distinct as an accretive branching-growth process.

It should remain, but its renewal strategy deserves attention because classical DLA accumulates.

Recommended renewal:

- age-based erosion
- destructive walkers
- branch fracture
- condition-triggered restart
- competing destructive DLA
- preserve a fragment and regrow

### Shapes

Likely worth keeping because it represents deliberate geometric generation rather than naturalistic simulation.

Its distinctness depends on implementation.

To avoid overlap with future geometry or fractal behaviors, Shapes should remain focused on discrete geometric construction and transformations.

## 5.2 Keep, but Clarify the Relationship

### Life

Keep as the general Conway-style world.

Its identity should be free evolution from a board state or seed.

### Glider

Glider is intentionally merged into Life for now.

It uses Conway-style Game of Life with glider injection, so its simulation core overlaps almost entirely with Life. Octessera exposes that idea as Life controls instead of a separate behavior ID:

- `Glider Interval` for automatic injections
- `Spawn Step` for timing position
- `Spawn Glider` for immediate manual injection

Only split Glider into its own behavior later if it gains a materially different performance loop, such as directional sources, steering, glider-only musical mapping, or a distinct relationship between player input and glider flow.

### Brain

Keep.

Although Brain and Life are both cellular automata, they behave differently enough:

- Life produces persistent structures, still lifes, oscillators, and moving objects
- Brain produces firing fronts, refractory trails, pulses, and excitable-wave behavior

Brain has a distinct temporal and rhythmic identity.

## 5.3 Main Redundancy Risk

The largest risk is not in the current list but in future additions.

Avoid collecting many near-neighbor cellular automata merely because they have different rule tables.

Examples to treat cautiously:

- HighLife
- Seeds
- Day & Night
- Morley
- multiple minor Life variants
- multiple excitable-media variants

A new cellular automaton should be added only when it creates a clearly different performance experience.

---

# 6. Recommended Category Tree

A flat list will become difficult to navigate as the library grows.

The menu should separate direct musical tools from algorithmic worlds, then group worlds by how they behave.

```text
Behaviors
├── Empty & Performance
│   ├── None
│   ├── Keys
│   ├── Sequencer
│   └── Looper
│
├── Cellular Build
│   ├── Life
│   ├── Brain
│   ├── Ant
│   ├── Cyclic
│   ├── Forest Fire
│   └── Predator–Prey
│
├── Particles & Motion
│   ├── Bounce
│   ├── Bubbles
│   ├── Gravity
│   ├── Boids
│   ├── Orbit
│   ├── Flow Field
│   └── Lightning
│
├── Growth & Decay
│   ├── Crystal Growth (implemented)
│   ├── DLA
│   ├── Physarum
│   ├── Vines
│   ├── Coral
│   └── Cracks
│
├── Fields & Waves
│   ├── Raindrops
│   ├── Wave
│   ├── Diffusion
│   ├── Magnetic Domains
│   ├── Ink
│   └── Reaction–Diffusion
│
├── Rhythm & Synchronization
│   ├── Pulse Network
│   ├── Kuramoto
│   ├── Fireflies
│   └── Coupled Clocks
│
├── Geometry & Space
│   ├── Shapes
│   ├── Maze
│   ├── Voronoi
│   ├── Tessellation
│   └── Fractal Explorer
│
└── Experimental
    ├── Rivers
    ├── Sand Ripples
    ├── Lava Lamp
    └── hybrid or unfinished systems
```

## 6.1 Practical Menu Recommendation

Do not create too many top-level categories immediately.

A good first version would use six groups:

```text
Behaviors
├── Human
├── Cellular
├── Motion
├── Growth
├── Fields
└── Geometry
```

Only add separate categories such as Synchronization or Ecology once each contains enough behaviors to justify a submenu.

## 6.2 Alternative Category Names

More technical:

- Performance
- Cellular Automata
- Agent Systems
- Particle Systems
- Growth Systems
- Field Systems
- Dynamical Systems
- Geometry

More approachable:

- Human
- Cells
- Swarms
- Motion
- Growth
- Waves
- Patterns

Recommendation:

> Use approachable menu labels and technical descriptions in documentation.

---

# 7. Prioritized New Behaviors

## Priority 1: Highest Value

These are strongly distinct from the current set, computationally cheap enough, and well aligned with Octessera.

## 7.1 Kuramoto / Pulse-Coupled Oscillators — implemented/current

**Category:** Rhythm & Synchronization
**Priority:** Very high
**Cost:** Very low to low

Each cell contains an oscillator with:

- phase
- natural frequency
- local coupling strength

Neighboring oscillators gradually synchronize.

Possible variants:

- continuous Kuramoto-style phase coupling
- discrete pulse-coupled firefly model
- cells flash when crossing a phase threshold
- flashes advance neighboring phases

Why it is valuable:

- creates emergent rhythm rather than merely spatial pattern
- extremely suitable for musical output
- computationally trivial with 64 local oscillators
- meaningfully different from existing behaviors
- naturally cycles between disorder and synchronization

Renewal:

- slight frequency drift
- random phase perturbations
- local resets
- periodic desynchronization pulse

Search terms:

- Kuramoto model local coupling
- pulse-coupled oscillators
- Mirollo Strogatz firefly synchronization
- cellular oscillator synchronization

## 7.2 Forest Fire — implemented/current

**Category:** Cellular Build
**Priority:** Very high
**Cost:** Very low

Cells are:

- empty
- tree
- burning

Rules:

- trees regrow
- fires spread
- lightning occasionally starts fires
- burned cells become empty

Why it is valuable:

- natural birth, destruction, and regrowth
- easy to understand
- strong rhythmic waves
- almost free computationally
- visually and musically distinct from Life and Brain

World events:

- new season after near-total burn
- wind shift after a large fire
- partial reseeding

Search terms:

- Drossel Schwabl forest fire
- forest fire cellular automaton

## 7.3 Physarum / Slime Mold — implemented/current

**Category:** Growth & Decay
**Priority:** Very high
**Cost:** Low to moderate in miniature form

Use a bounded population of agents.

Each agent:

- samples trail values ahead and to either side
- turns toward stronger trails
- moves
- deposits trail

The trail field evaporates over time.

Why it is valuable:

- combines agents, memory, path formation, and decay
- visually unlike DLA
- naturally transient through evaporation
- strong potential for evolving networks

Recommended scale:

- 16–64 agents
- 8×8, 16×16, or 32×32 trail field
- fixed sensing directions
- fixed maximum work per tick

World events:

- move food sources
- erase over-saturated trail regions
- preserve one network fragment and restart

Search terms:

- Physarum agent simulation
- Jeff Jones slime mold algorithm
- trail-following agent model

## 7.4 Cracks / Glass — implemented/current

**Category:** Growth & Decay
**Priority:** Very high
**Cost:** Very low to low

The board is a sheet of glass.

Stress accumulates.

Cracks propagate from:

- impact points
- stressed cells
- existing crack tips

A crack may branch or turn based on local stress.

Why it is valuable:

- extremely clear lifecycle
- strong visual narrative
- deterministic enough to feel structured
- reset condition is naturally meaningful

World event:

> When a connected crack reaches or splits opposite edges, the pane shatters and is replaced.

Possible post-shatter options:

- full reset
- preserve a few fragments
- temporarily scatter debris
- make the next pane inherit stress scars

Search terms:

- procedural crack propagation
- glass fracture simulation grid
- cellular fracture model
- lattice fracture simulation

## 7.5 Boids — implemented/current

**Category:** Particles & Motion
**Priority:** High
**Cost:** Low with a small population

Each agent follows:

- separation
- alignment
- cohesion

Why it is valuable:

- iconic emergent system
- fluid and collective movement
- different from Bounce because movement is socially coupled
- a small population is sufficient

Recommended scale:

- 8–24 boids
- naive pairwise interaction is acceptable at this size
- wrap, reflect, or attract at boundaries

Renewal:

- scatter event when flock becomes too compact
- temporary predator
- random new boid heading
- split flock when stable

Search terms:

- Craig Reynolds Boids
- flocking simulation local rules

## 7.6 Wave — implemented/current

**Category:** Fields & Waves
**Priority:** High
**Cost:** Very low to low

Maintain displacement and velocity per cell.

Energy propagates through neighboring cells with damping.

Why it is valuable:

- continuous spatial propagation
- very cheap
- naturally transient through damping
- musically useful as traveling energy

Difference from Raindrops:

- Raindrops is a specific falling-drop and splash behavior
- Wave is a general field that propagates, reflects, interferes, and is repeatedly excited

World events:

- inject a new impulse when energy falls too low
- invert boundaries
- change reflection behavior
- calm-to-storm phase shift

Search terms:

- discrete 2D wave equation grid
- ripple simulation finite difference

## 7.7 Fractal Explorer — implemented/current

**Category:** Geometry & Space / Dynamical Systems
**Priority:** High
**Cost:** Low to moderate

The behavior continuously explores a fractal rather than drawing a static one.

State includes:

- center point
- zoom level
- drift direction
- zoom direction
- iteration limit
- fractal parameters

The center drifts semi-randomly or according to a detail score.

Possible fractals:

- Mandelbrot
- Julia
- Burning Ship
- Newton fractal

Why it is valuable:

- mathematically distinct from all current behaviors
- provides an effectively infinite landscape
- naturally supports zoom, drift, morphing, and reseeding
- only 64 output samples are required for an 8×8 board

Recommended implementation:

- bounded iteration count
- 64 samples per frame
- optional higher-resolution cache
- score candidate centers by edge/detail density
- reset or zoom out when the region becomes uninteresting

World events:

- detail collapse → choose new center
- excessive uniformity → zoom out
- maximum zoom → jump to new region
- Julia parameter drift → morphing phase

Search terms:

- Mandelbrot escape-time algorithm
- Julia set explorer
- fractal zoom center selection
- adaptive fractal exploration

---

# 8. Priority 2: Strong Additions

## 8.1 Cyclic Cellular Automaton — implemented/current

**Cost:** Very low

Each cell has one of several states.

A cell advances when enough neighbors are in the next state.

Produces:

- waves
- spirals
- rotating fronts

Strengths:

- extremely cheap
- naturally active
- visibly distinct from Life and Brain

Caution:

- choose a rule that remains legible on 8×8

## 8.2 Predator–Prey Ecosystem — implemented/current

**Cost:** Very low to low

Spatial agents or cells represent:

- grass or resource
- herbivore
- predator

Simple rules create population cycles.

Strengths:

- ecological rise and collapse
- natural reset conditions
- meaningful multi-species interaction
- strong long-form musical variation

World events:

- species extinction → migration or reseeding
- overpopulation → famine
- resource recovery phase

## 8.3 Gravity / Sandpile — implemented/current

**Cost:** Very low

Particles fall, stack, slide, and avalanche.

Strengths:

- intuitive movement
- strong rhythmic cascades
- simple local rules

Renewal:

- clear bottom rows
- invert gravity
- trigger avalanche
- remove settled cells by age

## 8.4 Orbit / Moving Attractors — implemented/current

**Cost:** Low

Particles move around one or more attractors.

Attractors may:

- drift
- appear
- disappear
- repel instead of attract

Strengths:

- flowing motion
- distinct from Bounce and Boids
- simple bounded particle system

## 8.5 Crystal Growth — implemented/current

**Cost:** Very low

Seeded cells expand according to local orientation or neighborhood rules.

Strengths:

- more geometric than DLA
- easy to combine with erosion
- useful for structured growth

Renewal:

- dissolve old cells
- competing crystal colors or phases
- parity cancellation
- fracture at edge contact

## 8.6 Lightning — implemented/current

**Cost:** Very low to low

A branching leader grows toward a target edge or field gradient.

Once it connects:

- flash
- decay
- restart

Strengths:

- natural short lifecycle
- strong dramatic events
- easy reset condition

Caution:

- should not become merely a one-shot drawing
- use multiple leaders, branching, and variable decay

## 8.7 Ink / Diffusing Dye — implemented/current

**Cost:** Low

Drops spread and diffuse through a field while fading or being absorbed.

Strengths:

- soft, fluid evolution
- different from discrete cells and particles
- naturally transient

Caution:

- may resemble Wave or Reaction–Diffusion if not given a clear identity

## 8.8 Bubbles

**Category:** Motion
**Cost:** Very low to low

Bubbles spawn from the bottom of the grid and rise until they leave the top.
Each bubble has a subcell position tracked at 64 steps per LED cell on both axes,
then quantized back to the LED matrix for rendering and musical interpretation.

Bubbles may be:

- a single cell
- a small hollow circle
- intermediate radii rendered as compact hollow shapes, such as a radius-3 bubble
  drawn as a 3×3 cross with an empty center

Motion combines three influences:

- **buoyancy**: upward movement speed, adjustable 1–8, in eighth-cell units
- **current**: constant lateral movement from -8 to 8, where -8 moves one LED
  cell left per step, 0 is still, and 8 moves one LED cell right per step
- **drift**: independent random sway per bubble; each step chooses -1, 0, or 1,
  multiplies it by a 0–8 eighth-cell drift amount, and applies it to that bubble's X position

When bubbles accidentally touch, they merge into one new bubble using their
combined radius. When they rise beyond the top of the grid, they despawn. New
bubbles are always generated from the bottom.

Core parameters:

- minimum radius
- maximum radius
- spawn frequency
- spawn count
- drift amount, 0–8
- current, -8–8
- buoyancy, 1–8

Strengths:

- buoyant vertical motion distinct from Bounce and Raindrops
- sparse-to-dense texture controlled by spawn frequency and count
- musically useful merging events and disappearing lifecycle
- cheap bounded particle system with clear renewal

Caution:

- keep population bounded so high spawn settings remain predictable
- keep hollow-circle rendering legible on 8×8

---

# 9. Priority 3: Good but More Conditional

## 9.1 Reaction–Diffusion / Gray–Scott — implemented/current

**Cost:** Moderate relative to the others

Two fields diffuse and react.

Strengths:

- organic spots, stripes, and labyrinths
- rich parameter space

Limitations:

- 8×8 may be too coarse
- usually benefits from 16×16 or 32×32 internal resolution
- overlaps somewhat with Turing Patterns
- less immediately legible than simpler systems

Recommendation:

> Implement one reaction–diffusion behavior, not separate Gray–Scott and Turing entries unless they feel meaningfully different.

## 9.2 Vines — implemented/current

**Cost:** Very low to low

Branches grow toward space, light, or resources.

Strengths:

- organic branching
- simple growth logic

Limitations:

- possible overlap with DLA, Crystal, and Coral

Keep only if it has a distinct rule:

- directional growth
- branching hierarchy
- resource transport
- pruning

## 9.3 Coral — implemented/current

**Cost:** Very low to low

Growth favors exposed surfaces.

Colonies compete.

Old regions die or break away.

Strengths:

- natural competition
- attractive lifecycle

Limitations:

- can overlap with DLA and Vines

## 9.4 Maze Growth — implemented/current

**Cost:** Very low

A maze is generated incrementally.

Strengths:

- clear structure
- trivial cost
- natural completion event

Limitations:

- may be visually interesting but musically less rich
- risks becoming a construction animation rather than a living world

Improve it by adding:

- erosion
- competing maze builders
- moving path activity
- repeated collapse and regrowth

## 9.5 Ising / Magnetic Domains — implemented/current

**Cost:** Very low

Cells align with neighbors under temperature/noise.

Strengths:

- phase transitions
- moving domains
- simple local rule

Limitations:

- may be less musically obvious
- can settle into large uniform regions

Renewal:

- heat pulses
- field reversal
- local noise injection

## 9.6 Rivers — implemented/current

**Cost:** Low

Water follows a changing height field.

Flow erodes terrain and deposits material.

Strengths:

- channels form, clog, and migrate
- strong emergent structure

Limitations:

- more implementation design is required
- needs careful simplification for 8×8

## 9.7 Sand Ripples / Dunes — implemented/current

**Cost:** Low

Wind transports grains.

Deposition creates migrating ridges.

Strengths:

- slow coherent motion
- useful for long-form evolution

Limitations:

- may be subtle at 8×8

## 9.8 Lava Lamp / Metaball Approximation — implemented/current

**Cost:** Low to moderate

A few soft blobs move, merge, and split.

Strengths:

- visually distinct
- smooth and organic

Limitations:

- needs field sampling or approximation
- may be less naturally eventful musically

---

# 10. Behaviors to Avoid or Consolidate

## 10.1 Too Many Life-Like Automata

Do not add a long list of Life rule variants unless one creates a clearly different interaction pattern.

Prefer one configurable "Life Family" behavior over many near-duplicates.

## 10.2 Duplicate Reaction–Diffusion Entries

Gray–Scott and Turing Patterns belong to the same broad family.

Start with one behavior.

Add another only if the interaction and output are clearly distinct.

## 10.3 Static Fractal Renderers

A static Mandelbrot or Julia image is not enough.

Use:

- continuous zoom
- drifting center
- parameter morphing
- interesting-region selection
- lifecycle reset

The behavior should be **Fractal Explorer**, not merely **Fractal**.

## 10.4 One-Way Drawing Algorithms

Avoid systems that simply finish a picture unless they include:

- erosion
- competing growth
- conditional reset
- fragment inheritance
- ongoing transformation

## 10.5 Generic Particle Effects

Smoke, fire, snow, and sparks are not automatically strong behaviors.

They should be added only if their rules create a distinctive emergent system rather than a visual effect.

## 10.6 Near-Duplicate Growth Systems

DLA, Crystal, Vines, Coral, and Lightning can overlap.

Each should have a specific identity:

- DLA: random aggregation
- Crystal: local geometric growth
- Vines: directed branching and pruning
- Coral: surface competition and colony death
- Lightning: rapid goal-seeking fracture followed by decay

---

# 11. Final Priority Order

## Tier S — Implement First

1. *(implemented)* **Kuramoto / Pulse Network**
2. **Forest Fire**
3. *(implemented)* **Physarum / Slime Mold**
4. *(implemented)* **Cracks / Glass**
5. *(implemented)* **Boids**
6. *(implemented)* **Wave**
7. *(implemented)* **Fractal Explorer**

These offer the best combination of:

- distinctness
- musical potential
- low computational cost
- clear lifecycle
- strong interaction
- minimal overlap with the current library

## Tier A — Strong Next Additions

8. *(implemented)* **Cyclic Cellular Automaton**
9. *(implemented)* **Predator–Prey Ecosystem**
10. *(implemented)* **Gravity / Sandpile**
11. **Bubbles**
12. *(implemented)* **Orbit / Moving Attractors**
13. *(implemented)* **Lightning**
15. *(implemented)* **Ink / Diffusing Dye**

## Tier B — Add Selectively

16. *(implemented)* **Reaction–Diffusion**
17. *(implemented)* **Vines**
18. *(implemented)* **Coral**
19. *(implemented)* **Maze**
20. *(implemented)* **Ising / Magnetic Domains**
21. *(implemented)* **Rivers**
22. *(implemented)* **Sand Ripples**
23. *(implemented)* **Lava Lamp**

---

# 12. Recommended Immediate Decisions

## Keep

- None
- Keys
- Sequencer
- Looper
- Life
- Brain
- Cyclic
- Ant
- Bounce
- Bubbles
- Gravity
- Boids
- Orbit
- Shapes
- Raindrops
- DLA
- Forest Fire
- Predator–Prey
- Kuramoto
- Wave
- Ink
- Ising — implemented/current
- Coral — implemented/current
- Cracks
- Crystal Growth
- Physarum
- Vines — implemented/current
- Fractal Explorer
- Maze Growth — implemented/current
- Lightning
- Reaction–Diffusion — implemented/current

## Merged into Life

- Glider

Glider is represented by Life's `Glider Interval`, `Spawn Step`, and `Spawn Glider` controls. Do not add a separate `glider` behavior unless a future design gives it a distinct interaction loop and musical output character.

## Avoid Adding Soon

- more minor Life variants
- a second reaction–diffusion behavior
- another generic random-walker behavior
- another branching growth behavior without a clearly distinct lifecycle
- static fractal rendering
- purely cosmetic particle effects

---

# 13. Suggested Near-Term Menu

With the current set plus the first recommended additions:

```text
Behaviors
├── Human
│   ├── None
│   ├── Keys
│   ├── Sequencer
│   └── Looper
│
├── Cellular
│   ├── Life
│   ├── Brain
│   ├── Ant
│   ├── Forest Fire
│   ├── Cyclic
│   └── Predator–Prey
│
├── Motion
│   ├── Bounce
│   ├── Boids
│   ├── Gravity
│   ├── Bubbles
│   ├── Lightning
│   ├── Orbit
│   └── Flow Field
│
├── Growth
│   ├── Crystal Growth
│   ├── DLA
│   ├── Physarum
│   └── Cracks
│
├── Fields
│   ├── Raindrops
│   ├── Wave
│   ├── Ink
│   └── Reaction–Diffusion
│
├── Rhythm
│   └── Pulse Network
│
└── Geometry
    ├── Shapes
    ├── Maze
    └── Fractal Explorer
```

This tree is broad enough to scale without becoming cumbersome.

---

# 14. Guiding Principle

The best Octessera behaviors are not simply generators.

They are **small worlds with their own logic, tension, lifecycle, and renewal**.

A good behavior should make the player feel that something is happening on the board even before it is mapped to sound.

The strongest additions will not be the most mathematically sophisticated ones.

They will be the ones that are:

- easy to understand
- cheap to simulate
- hard to fully predict
- easy to influence
- capable of disappearing and renewing
- meaningfully different from everything already present
