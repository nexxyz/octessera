# L1: Life Menu Tree

This file is part of the canonical split-out menu tree spec. See [`../menu-tree-spec.md`](../menu-tree-spec.md) for the canonical index.

### L1: Life

```
L1: Life
├── P1: ... (group)                              ← one group per part, label computed via partLabel()
│   ├── Behavior: <id> (group)                   ← browser-style selector for this part's behavior
│   │   ├── [Cellular]
│   │   │   ├── ..
│   │   │   ├── ant
│   │   │   ├── brain
│   │   │   ├── glider
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
│   ├── Part Label: (text, max 32)               ← display label; editing sets Auto Label off
│   ├── Step Rate: [1/16, 1/8, 1/4, 1/2, 1/1]   ← controls how often onTick() is called; hidden when Behavior is `none`
│   ├── ... per-behavior dynamic config from behavior's configMenu()
│   └── Reset                                    ← reinitializes the active behavior state; hidden when Behavior is `none`
├── P2: ... (group)
└── P3: ... (group)                              ← up to partCount parts total
```

Rows that open submenus or selectors render with a trailing `>`. Selecting a behavior row switches the part immediately through the native runtime and returns focus to the part's Behavior row. It does not rebuild the full menu tree; only the active part's L1 rows are refreshed. Behavior IDs remain the persisted payload values under `behaviorId`.
When Auto Label is on, the part label is derived from the active behavior ID (e.g. `life`, `brain`). Editing the Part Label text field switches Auto Label off.
Part selectors (Fn+column selection, L2 Sense Part selector) display the computed part label (e.g. `P1: life`, `P2: rain`).
When a part's behavior is `none`, the L1 part group shows Behavior, Auto Label, and Part Label only; Step Rate, dynamic behavior config rows, and Reset are hidden without deleting stored values.
Parameter target pickers mirror the main menu root order (`L1: Life`, `L2: Sense`, `L3: Voice`, `L4: Dance`, `System`). Within `L1: Life`, behavior `none` parts expose no Behavior targets, while real behavior parts expose `parts.N.algorithmStep` and `parts.N.l1.behaviorConfig.*` targets under their own part label.

Behavior-specific config items (from `configMenu()`):

| Behavior | Config Items | Type/Options |
|---|---|---|
| none | *(none)* | — |
| life | Spawn Count: [0..20] | number, step 1 (default 12) |
| life | Spawn Interval: [1..20] | number, step 1 (default 1) |
| life | !Spawn Random | action, shared route `trigger.life.spawn_now` |
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
| glider | Glider Spawn Int: [0=off, 1, 2, 4, 8, 16] | enum |
| glider | !Spawn Glider | action, shared route `trigger.life.spawn_now` |

