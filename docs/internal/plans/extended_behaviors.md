# Octessera – Behaviors

## Philosophy

Octessera is an instrument for **steering emergent musical systems**, not programming deterministic sequences.

Each behavior represents a small autonomous "world" governed by simple local rules. Music emerges from the interaction between the behavior, the player's input, and the musical mapping.

The goal is not randomness.

The goal is **emergence**.

---

# Design Principles

## Simple Rules

Every behavior should be understandable after a short explanation.

Complexity should arise from interaction, not from dozens of parameters.

If the core idea cannot be explained on a napkin, it probably does not belong in Octessera.

---

## Emergence over Randomness

Interesting patterns should arise from local interactions.

Randomness is useful only as:

* initial seeding
* occasional perturbation
* controlled renewal

Random note generation is not a behavior.

---

## Transient Worlds

Nothing should exist forever.

Structures should:

* appear
* evolve
* interact
* decay
* disappear

Permanent equilibrium is generally undesirable.

---

## Continuous Renewal

Every behavior should remain interesting indefinitely.

If a simulation naturally converges, Octessera should gently reintroduce novelty without fundamentally changing the algorithm.

---

## Player Influence

The musician does not program the world.

The musician **steers** it.

The world remains partially autonomous while responding naturally to intervention.

---

# Native vs. Imposed Transience

Some algorithms naturally contain both constructive and destructive forces.

Others only accumulate.

Both are acceptable.

If a source algorithm does not naturally remove structures, Octessera may introduce a complementary lifecycle system while preserving the recognizable character of the original behavior.

The objective is compelling musical behavior, not strict academic fidelity.

## Native Transience

Examples:

* Brian's Brain
* Forest Fire
* Slime Mold (trail evaporation)
* Wave Equation (damping)
* Raindrops (expanding and fading splashes)

These generally require little additional lifecycle management.

## Imposed Transience

Possible strategies include:

### Cell Lifetime (TTL)

Each created cell receives a finite lifetime.

When the timer expires, it disappears.

---

### Age-Based Erosion

Older cells become progressively more likely to disappear.

---

### XOR / Toggle Combination

A secondary process toggles cells instead of only creating them.

Overlapping activity naturally cancels.

---

### Even/Odd (Parity) Combination

Each activation flips a cell.

Odd activations produce live cells.

Even activations remove them.

---

### Competing Instances

Run two copies of the same behavior.

One primarily creates.

One primarily destroys.

The destructive instance may be mirrored, phase-shifted, delayed or independently seeded.

---

### Destructive Walkers

Introduce agents whose purpose is to erase cells.

Examples:

* wandering particles
* ants
* predator agents

---

### Density-Based Death

Dense regions gradually erode.

---

### Resource Consumption

Cells consume local resources.

Without resources they disappear.

Resources regenerate over time.

---

### Alternating Phases

Alternate between periods of growth and erosion.

---

### Pattern-Based Erasure

Inject destructive patterns such as:

* waves
* gliders
* masks
* expanding circles

These periodically clear space for new structures.

---

## Guideline

Behaviors that do not naturally remove their own output should define an explicit removal mechanism.

The removal process should complement the original algorithm rather than replacing it.

---

# Performance Philosophy

Octessera is a musical instrument.

Behavior simulation exists to drive musical interaction, not to maximize simulation accuracy.

A convincing miniature implementation is preferred over a computationally expensive reference implementation.

The musical character is more important than algorithmic completeness.

---

## Fixed Computational Budget

Every behavior should have a strictly bounded execution cost.

Recommended constraints:

* Fixed grid size
* Fixed maximum agent count
* Fixed work per update
* No unbounded searches
* No recursive growth without limits
* No heap allocation during steady-state updates

The CPU cost of a behavior should remain predictable regardless of its current state.

---

## Simulation Rate

Behaviors should operate at musical control rates rather than audio rates.

Typical update frequencies:

* 10–30 Hz for slower evolving systems
* Up to 60 Hz for highly dynamic behaviors

There is generally no benefit in simulating behaviors at audio sample rates.

---

## Audio Separation

Behavior simulation must remain independent from the realtime audio callback.

The audio engine should consume lightweight snapshots or generated musical events.

Simulation should never perform:

* expensive computation
* memory allocation
* locking
* unbounded loops

inside the realtime audio thread.

---

# Existing Behaviors

## None

**Category:** Null

Empty behavior.

Performs no simulation, processing or autonomous activity.

Represents the intentional absence of a behavior.

---

## Keys

**Category:** Performance

Direct manual performance.

---

## Sequencer

**Category:** Musical

Traditional deterministic step sequencer.

---

## Looper

**Category:** Musical

Loop recording and playback.

---

## Life

**Category:** Cellular Automaton

Conway's Game of Life.

---

## Glider

**Category:** Cellular Automaton

Conway's Game of Life with periodic glider injection.

---

## Brain

**Category:** Cellular Automaton

Brian's Brain.

Cells cycle through:

* resting
* firing
* refractory

Optional random seeding maintains continuous activity.

---

## Ant

**Category:** Cellular Automaton

Langton's Ant.

---

## Bounce

**Category:** Physics

Particles bounce through the grid and interact with boundaries.

---

## Shapes

**Category:** Geometry

Procedural geometric generators.

---

## Raindrops

**Category:** Physics

Droplets fall onto the grid.

Each impact produces expanding splashes that gradually disappear.

---

## DLA

**Category:** Emergent Growth

Diffusion-Limited Aggregation.

Random walkers attach to existing structures, gradually producing branching forms.

Future implementations should incorporate erosion or competing growth to avoid permanent accumulation.

---

# Candidate Behaviors

## Tier 1 — Strongest Candidates

### Slime Mold (Physarum)

**Category:** Agent Simulation

A bounded population of simple agents:

* senses nearby trails
* moves
* deposits trails

Trails gradually evaporate.

Produces continuously evolving transport networks.

**Implementation Notes**

* Small fixed agent count (e.g. 16–64)
* Fixed trail buffer
* No dynamic allocation

**References**

* Physarum simulation
* Jones Physarum algorithm

---

### Forest Fire

**Category:** Cellular Automaton

Trees grow.

Lightning ignites fires.

Burned areas recover.

Excellent natural birth–death cycle.

**References**

* Drossel–Schwabl Forest Fire

---

### Boids

**Category:** Swarm Simulation

Agents follow:

* Separation
* Alignment
* Cohesion

Produces flocking behavior.

**Implementation Notes**

Use a small fixed population.

A few dozen boids are sufficient for rich behavior.

**References**

* Craig Reynolds Boids

---

## Tier 2 — Excellent Candidates

### Cyclic Cellular Automaton

Produces spirals and travelling waves.

Very inexpensive.

---

### Wave Equation

Travelling waves with:

* damping
* reflection
* interference

Natural extension of Raindrops.

---

### Falling Sand

Simple local material rules.

Supports sand, water, smoke and similar materials.

---

### Crystal Growth

Seeds nucleate.

Crystals grow.

Old structures dissolve.

---

### Gravity

Particles fall, stack and avalanche.

Periodic disturbances maintain renewal.

---

## Tier 3 — Good Candidates

### Diffusion

Random walkers continuously enter and leave the world.

---

### Vines

Branches grow, compete and die.

---

### Ising Model

Neighbor interactions produce evolving domains.

Introduce thermal noise to avoid freezing.

---

### Orbital System

Particles orbit temporary moving attractors.

---

## Tier 4 — More Computationally Demanding

These remain viable but should use simplified implementations or lower internal resolutions.

### Reaction–Diffusion (Gray–Scott)

Virtual chemicals diffuse and react.

Produces:

* spots
* stripes
* labyrinths

**References**

* Gray–Scott reaction diffusion

---

### Turing Patterns

Reaction–diffusion variant inspired by biological morphogenesis.

Produces evolving organic textures.

**References**

* Turing morphogenesis

---

# General Implementation Guidelines

Every behavior should satisfy the following:

* Simple local rules
* Emergent global behavior
* Predictable computational cost
* Bounded execution time
* No unbounded loops
* Fixed memory usage
* Continuous renewal
* Suitable for real-time musical interaction

When choosing between algorithmic purity and musical usefulness, prioritize musical usefulness.

A miniature implementation that captures the essence of a system is preferable to a computationally expensive reference implementation.

---

# Final Guiding Principle

Every world should balance **construction** and **destruction**.

Some algorithms provide both naturally.

Others require Octessera to introduce complementary removal mechanisms.

The objective is not to faithfully reproduce scientific simulations.

The objective is to create **living, evolving musical worlds** that remain expressive, responsive and computationally lightweight enough to coexist with real-time synthesis.
