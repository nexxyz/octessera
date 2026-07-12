# Build Menu Tree

This file is part of the canonical split-out menu tree spec. See [`../menu-tree-spec.md`](../menu-tree-spec.md) for the canonical index.

### Build

```
Build
├── L1: ... (group)                              ← one group per layer, label computed from the layer label
│   ├── Behavior: <id> (group)                   ← browser-style selector for this layer's behavior
│   │   ├── [Cellular]
│   │   │   ├── ..
│   │   │   ├── ant
│   │   │   ├── brain
│   │   │   └── life
│   │   ├── [Fields]
│   │   │   ├── ..
│   │   │   └── raindrops
│   │   ├── [Geometry]
│   │   │   ├── ..
│   │   │   └── shapes
│   │   ├── [Growth]
│   │   │   ├── ..
│   │   │   └── dla
│   │   ├── [Motion]
│   │   │   ├── ..
│   │   │   └── bounce
│   │   └── [Play]
│   │       ├── ..
│   │       ├── keys
│   │       ├── looper
│   │       ├── none
│   │       └── sequencer
│   ├── Auto Label: [on | off]                   ← on: label auto-derives from behavior ID; off: label is manual text
│   ├── Layer Label: (text, max 32)               ← display label; editing sets Auto Label off
│   ├── Step Rate: [1/16, 1/8, 1/4, 1/2, 1/1]   ← controls how often onTick() is called; hidden when Behavior is `none`
│   ├── ... per-behavior dynamic config from behavior's configMenu()
│   └── Reset                                    ← reinitializes the active behavior state; hidden when Behavior is `none`
├── L2: ... (group)
└── L3: ... (group)                              ← up to layerCount layers total
```

Rows that open submenus or selectors render with a trailing `>`. Selecting a behavior row switches the layer immediately through the native runtime and returns focus to the layer's Behavior row. It does not rebuild the full menu tree; only the active layer's Build rows are refreshed. Behavior IDs remain the persisted payload values under `behaviorId`.
`glider` is no longer selectable. Its glider injection controls are part of `life`.
When Auto Label is on, the layer label is derived from the active behavior ID (e.g. `life`, `brain`). Editing the Layer Label text field switches Auto Label off.
Layer selectors (Fn+column selection, Link Layer selector) display the computed layer label (e.g. `L1: life`, `L2: rain`).
When a layer's behavior is `none`, the Build layer group shows Behavior, Auto Label, and Layer Label only; Step Rate, dynamic behavior config rows, and Reset are hidden without deleting stored values.
Parameter target pickers mirror the main menu root order (`Build`, `Link`, `Shape`, `Play`, `System`). Within `Build`, behavior `none` layers expose no Behavior targets, while real behavior layers expose `layers.N.algorithmStep` and `layers.N.worlds.behaviorConfig.*` targets under their own layer label.

Behavior categories:

| Category | Behaviors | Description |
|---|---|---|
| Cellular | ant, brain, life | Cell-state simulations where neighboring cells or agents create evolving patterns. |
| Fields | raindrops | Field-style activity that spreads from localized events. |
| Geometry | shapes | Explicit geometric pulse patterns. |
| Growth | dla | Diffusion-limited aggregation clusters that grow from seeded particles. |
| Motion | bounce | Moving objects that rebound through the grid. |
| Play | keys, looper, none, sequencer | Direct performance, recording, silence, or step-style behaviors. |

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
| ant | Max Ants: [1..10] | number, step 1 |
| ant | !Spawn Ant | action, shared route `trigger.life.spawn_now` |
| bounce | Max Balls: [1..20] | number, step 1 |
| bounce | !Add Ball | action, shared route `trigger.life.spawn_now` |
| shapes | Shape: [ring, heart, star, plus, x] | enum |
| shapes | Expansion Speed: [1..5] | number, step 1 |
| shapes | Auto Spawn Int: [0=off, 10, 20, 50] | enum |
| shapes | !Spawn Pulse | action, shared route `trigger.life.spawn_now` |
| raindrops | !Drop Now | action, shared route `trigger.life.spawn_now` |
| dla | !Seed Cluster | action, shared route `trigger.life.spawn_now` |
