# Implementation Plan: 10 Algorithms + 4 Trigger Types

> **STATUS: COMPLETED** — All 10 algorithms, 4 trigger types, menu system, and 84 tests are implemented and passing.
>
> Key deviations from original plan:
> - `behaviors-shapes/` was replaced by `behaviors-pulse/` (renamed to better reflect wavefront model)
> - Shapes behavior uses a **wavefront model** (leading edge of active cells with lifespan decay) rather than solid filled shapes
> - `behaviors-glider/` spawns gliders at intervals rather than being a continuous glider gun
> - Menu label is "Behavior" (not "Active") for the active behavior selector
> - Default scanned channel target is 0 (not equally distributed across 0-3)
>
> See `docs/menu-and-controls-spec.md` for the authoritative current menu tree.
> See `docs/implementation-done.md` for the final architecture summary.

## Overview

Replace Conway-specific `birth/death/state_on` with 4 generic trigger types, implement 10 algorithms as pluggable `BehaviorEngine` packages, extract Sequencer from `populationMode`, add general algorithm step rate, update menu system, and update LED rendering.

---

## Key Design Decisions

1. **4 Trigger Types** (scanning ≠ algorithm triggers):
   - `activate` = algorithm: cell becomes active (birth, shape hits cell, etc.)
   - `stable` = algorithm: cell stays active (alive, inside shape, etc.)
   - `deactivate` = algorithm: cell becomes inactive (death, shape leaves cell, etc.)
   - `scanned` = scanning layer: cell found active during scan (renamed from `state_on`)

2. **Scanning ≠ Population Behavior**: Completely independent. Any algorithm works with any scan mode.

3. **Scan mode "immediate" does nothing** — no `scanned` triggers are generated. Only "scanning" mode (column/row) generates `scanned` triggers.

4. **Algorithm Step Rate**: General parameter in "L1: Life" menu, applies to ALL algorithms. Controls how often `onTick()` is called.

5. **Sequencer Extraction**: Current "grid" mode becomes a proper `BehaviorEngine` (manual toggle only, no automatic changes).

6. **5 Shape Types** for Expanding Shapes: ring, heart, star (simple 5-ray), plus, x.

7. **Separate Voice Configs**: "Activate Target", "Stable Target", "Deactivate Target" (algorithm triggers) + NEW "Scanned Target" (scanning trigger).

---

## Phase 0: 4-Trigger Rename (Minimal)

**Goal:** Replace `birth` → `activate`, `death` → `deactivate`, `state_on` → `scanned` in interpretation layer. Add `stable` type for algorithm continuous state.

### Files to Modify

| File | Change |
|---|---|
| `packages/interpretation-core/src/index.ts` | `CellTransitionKind = "activate" \| "deactivate"`; `CellTriggerKind = CellTransitionKind \| "stable" \| "scanned"`; rename `extractBirthDeathTransitions` → `extractTransitions`; update all functions |
| `packages/mapping-core/src/index.ts` | `MappingConfig`: rename `birth` → `activate`, `death` → `deactivate`; ADD `stable` + `scanned` targets; update `targetForKind()` |
| `packages/mapping-core/src/config/default-mapping.json` | Rename keys + ADD `"scanned"` key |
| `packages/behavior-api/src/index.ts` | Update `BehaviorRenderModel` with `triggerTypes?: CellTriggerType[]`; `CellTriggerType = "activate" \| "stable" \| "deactivate" \| "scanned" \| "none"` |
| `packages/behaviors-life/src/index.ts` | Emit `activate`/`deactivate` in `onTick`; use `stable` via cells[] |
| `packages/platform-core/src/index.ts` | Update `profileFromConfig()`: `scanned` comes from scan mode only (not "immediate"); update menu labels |
| `docs/menu-and-controls-spec.md` | Update references |

### Key: `scanned` Only in Scanning Mode

In `packages/platform-core/src/index.ts`, update `profileFromConfig()`:

```typescript
function profileFromConfig(cfg: RuntimeConfig): InterpretationProfile {
  // ... existing logic ...
  
  // NEW: scanned mode depends on scan mode
  // "immediate" = no scanning triggers (user said it does nothing)
  const scannedMode = cfg.scanMode === "immediate" 
    ? "none" as const
    : cfg.scanAxis === "columns" 
      ? "scan_column" as const 
      : "scan_row" as const;

  return {
    // ...
    scannedMode // NEW: controls "scanned" trigger generation
  };
}
```

---

## Phase 0.5: Add General Algorithm Step Rate + Scanned Target

**Goal:** Add `algorithmStepUnit` config + `scanned` mapping target.

### Add to RuntimeConfig

**File:** `packages/platform-core/src/index.ts`

```typescript
type RuntimeConfig = {
  algorithmStepUnit: NoteUnit; // NEW: "1/16"|"1/8"|"1/4"|"1/2"|"1/1"
  // ...rest stays (REMOVE populationMode, conwayStepUnit)
};
```

Initialize in `createInitialState()`:
```typescript
algorithmStepUnit: "1/8", // default
```

### Update PlatformState

```typescript
type PlatformState<TState> = {
  // ...existing...
  algorithmPulseAccumulator: number; // NEW
};
```

Initialize: `algorithmPulseAccumulator: 0`

### Update tick() to Use algorithmStepUnit

**File:** `packages/platform-core/src/index.ts`

Replace the old `populationMode === "conway"` block with:

```typescript
// Always accumulate for algorithm step
const algorithmStepPulses = noteUnitToPulses(next.runtimeConfig.algorithmStepUnit);
while (next.algorithmPulseAccumulator >= algorithmStepPulses) {
  next.algorithmPulseAccumulator -= algorithmStepPulses;
  next.behaviorState = behavior.onTick(next.behaviorState, { 
    bpm: next.transport.bpm, 
    emit: () => {} 
  });
}
```

### Add `scanned` to Mapping Config

**File:** `packages/mapping-core/src/config/default-mapping.json`

```json
{
  "baseMidiNote": 24,
  "maxMidiNote": 84,
  "rangeMode": "wrap",
  "scale": [0, 3, 5, 7, 10],
  "rowStepDegrees": 3,
  "columnStepDegrees": 1,
  "activate": { "channel": 0, "velocity": 96, "durationMs": 150 },
  "stable": { "channel": 1, "velocity": 88, "durationMs": 130 },
  "deactivate": { "channel": 2, "velocity": 68, "durationMs": 90 },
  "scanned": { "channel": 3, "velocity": 100, "durationMs": 120 }
}
```

### Update targetForKind()

**File:** `packages/mapping-core/src/index.ts`

```typescript
function targetForKind(kind: CellTriggerKind, config: MappingConfig): TriggerTarget {
  if (kind === "activate") return config.activate;
  if (kind === "deactivate") return config.deactivate;
  if (kind === "scanned") return config.scanned; // NEW
  return config.stable;
}
```

### Add to L1 Menu

```typescript
function behaviorMenu<TState>(state: PlatformState<TState>): MenuNode {
  return {
    kind: "group",
    label: "L1: Life",
    children: [
      {
        kind: "enum",
        label: "Active",
        key: "activeBehavior",
        options: listBehaviorIds()
      },
      {
        kind: "enum",
        label: "Step Rate",
        key: "algorithmStepUnit",
        options: ["1/16", "1/8", "1/4", "1/2", "1/1"]
      },
      ...behaviorConfigChildren(state.activeBehavior)
    ]
  };
}
```

---

## Phase 1: Separate Scanning from Population + Extract Sequencer

### 1.1 Remove `populationMode` from RuntimeConfig

**File:** `packages/platform-core/src/index.ts`

Remove:
```typescript
type RuntimeConfig = {
  // REMOVE: populationMode: "grid" | "conway";
  // REMOVE: conwayStepUnit: NoteUnit;
};
```

### 1.2 Create Sequencer Behavior (Simplest Possible)

**New package:** `packages/behaviors-sequencer/`

**Package structure:**
```
packages/behaviors-sequencer/
├── package.json
├── src/
│   └── index.ts
└── tsconfig.json
```

**State:**
```typescript
export type SequencerState = {
  width: typeof GRID_WIDTH;
  height: typeof GRID_HEIGHT;
  cells: boolean[];
};
```

**Behavior (manual toggle only, `onTick` = identity):**
```typescript
export const sequencerBehavior: BehaviorEngine<SequencerState, {}> = {
  id: "sequencer",
  init() {
    return {
      width: GRID_WIDTH,
      height: GRID_HEIGHT,
      cells: new Array(GRID_WIDTH * GRID_HEIGHT).fill(false)
    };
  },
  onInput(state, input) {
    if (input.type !== "grid_press") return state;
    const i = input.y * GRID_WIDTH + input.x;
    const nextCells = state.cells.slice();
    nextCells[i] = !nextCells[i];
    return { ...state, cells: nextCells };
  },
  onTick(state) {
    return state; // NO-OP: manual toggle only
  },
  renderModel(state) {
    return {
      name: "Sequencer",
      statusLine: "Manual",
      cells: state.cells
    };
  },
  serialize(state) { return state; },
  deserialize(data) { return data as SequencerState; }
};
```

### 1.3 Create Behavior Registry

**New file:** `packages/behavior-api/src/registry.ts`

```typescript
import type { BehaviorEngine } from "./index";

const registry = new Map<string, BehaviorEngine<any, any>>();

export function registerBehavior(engine: BehaviorEngine<any, any>): void {
  registry.set(engine.id, engine);
}

export function getBehavior(id: string): BehaviorEngine<any, any> | undefined {
  return registry.get(id);
}

export function listBehaviorIds(): string[] {
  return Array.from(registry.keys());
}
```

### 1.4 Update Platform-Core

**File:** `packages/platform-core/src/index.ts`

- Import registry functions
- Remove hardcoded `const behavior = lifeBehavior;`
- Add behavior resolution:
```typescript
function resolveBehavior(activeId: string) {
  return getBehavior(activeId) ?? sequencerBehavior; // fallback
}
```

- Update `createInitialState()` to use `getBehavior()`
- Update `applyConfigPayload()` to handle `activeBehavior`

### 1.5 Update Menu System

**File:** `packages/platform-core/src/index.ts`

Replace hardcoded `"L1: Life"` group with dynamic behavior selector (see Phase 0.5).

Remove `populationMode` and `conwayStepUnit` from menu entirely.

---

## Phase 2: LED Rendering (4 Trigger Types)

**Goal:** Show `activate` = bright white, `stable` = green, `deactivate` = dim, `scanned` = red.

### 2.1 Update BehaviorRenderModel

**File:** `packages/behavior-api/src/index.ts`

```typescript
export type CellTriggerType = "activate" | "stable" | "deactivate" | "scanned" | "none";

export type BehaviorRenderModel = {
  name: string;
  statusLine: string;
  cells: boolean[];
  triggerTypes?: CellTriggerType[]; // NEW: per-cell trigger type
};
```

### 2.2 Update LED Conversion

**File:** `packages/platform-core/src/index.ts`

```typescript
function cellsToLeds(
  cells: boolean[],
  triggerTypes: CellTriggerType[] | undefined,
  brightness: number
): LedCell[] {
  const base = Math.floor(255 * brightness / 100);
  return cells.map((on, i) => {
    if (!on) return { r: 0, g: 0, b: 0 };
    const type = triggerTypes?.[i] ?? "stable";
    switch (type) {
      case "activate": return { r: base, g: base, b: base }; // bright white
      case "deactivate": return { r: base/2, g: base/2, b: base/2 }; // dim
      case "scanned": return { r: base, g: 0, b: 0 }; // red for scanned
      default: return { r: 0, g: base, b: 0 }; // green for stable
    }
  });
}
```

### 2.3 Each Behavior Populates triggerTypes

Each behavior's `renderModel()` should return `triggerTypes` array based on current state transitions.

---

## Phase 3: 10 Algorithms

### Algorithm 0: Sequencer (Extracted)

**Package:** `packages/behaviors-sequencer/` (described in Phase 1.2)

**Behavior:** Manual toggle only, `onTick` = identity.

**Trigger types:**
- `scanned` = cell is on when scan column hits it (from interpretation layer)
- `activate`/`deactivate` = none (no automatic state changes)
- `stable` = none (cells don't have "stable" in this behavior)

**Menu config:**
```
L1: Life > Sequencer
  ├── Step Rate: [1/16, 1/8, 1/4, 1/2, 1/1]  ← general param
  └── (no algorithm-specific config - simplest)
```

---

### Algorithm -1: Conway's Game of Life +

**Package:** `packages/behaviors-life/` (extend existing)

**State:**
```typescript
type LifeState = {
  width: number; height: number;
  cells: boolean[];
  generation: number;
  randomCellsPerTick: number;
  randomTickInterval: number;
  tickCounter: number;
  internalPulseAccumulator: number;
};
```

**onTick logic:**
1. Standard Conway rules (B3/S23) - happens when platform calls `onTick()` based on `algorithmStepUnit`
2. If `randomCellsPerTick > 0` and counter met: add random alive cells

**Trigger types:**
- `activate` = cell just born
- `deactivate` = cell just died
- `stable` = cell alive and stable
- `scanned` = comes from interpretation layer (if scan mode active)

**Menu config:**
```
L1: Life > Conway+
  ├── Step Rate: [1/16, 1/8, 1/4, 1/2, 1/1]  ← general param
  ├── Random Cells/Tick: [0, 1, 2, ...]
  └── Random Interval: [1, 2, 4, ...]
```

---

### Algorithm 1: Expanding Shapes (5 shapes)

**Package:** `packages/behaviors-shapes/`

**State:**
```typescript
type ShapeKind = "ring" | "heart" | "star" | "plus" | "x";
type Shape = {
  originX: number; originY: number;
  kind: ShapeKind;
  radius: number;    // expansion progress
  alive: boolean;   // still expanding?
};
type ShapesState = {
  width: number; height: number;
  cells: boolean[];     // union of all shapes
  shapes: Shape[];
  nextId: number;
  autoSpawnInterval: number; // 0 = disabled
  tickCounter: number;
};
```

**Shape Geometry Functions:** (in `packages/behaviors-shapes/src/shapes.ts`)

| Shape | Geometry |
|---|---|
| `ring` | Euclidean distance ≈ r from origin (concentric circles) |
| `heart` | Parametric: (16sin³t, 13cost - 5cos2t - 2cos3t - cos4t) scaled by r |
| `star` | **Simple 5 rays at 0°, 72°, 144°, 216°, 288°** (pentagram) |
| `plus` | Horizontal + vertical cross (Manhattan distance ≤ r) |
| `x` | Both diagonals (Manhattan distance on diagonals ≤ r) |

**Star shape function (simplified 5-ray):**
```typescript
function starCells(x: number, y: number, r: number, w: number, h: number): [number, number][] {
  const cells: [number, number][] = [];
  // 5 rays at angles 0°, 72°, 144°, 216°, 288°
  const angles = [0, 72, 144, 216, 288];
  for (const angleDeg of angles) {
    const rad = angleDeg * Math.PI / 180;
    for (let d = 0; d <= r; d++) {
      const cx = Math.round(x + d * Math.cos(rad));
      const cy = Math.round(y + d * Math.sin(rad));
      if (cx >= 0 && cx < w && cy >= 0 && cy < h) {
        cells.push([cx, cy]);
      }
    }
  }
  return cells; // remove duplicates if needed
}
```

**Trigger types:**
- `activate` = cell just entered by expanding shape this tick
- `stable` = cell in shape interior
- `deactivate` = shape leaves cell (if shape shrinks) or shape died
- `scanned` = from interpretation layer

**Menu config:**
```
L1: Life > Shapes
  ├── Step Rate: [1/16, 1/8, 1/4, 1/2, 1/1]  ← general param
  ├── Shape Type: [ring | heart | star | plus | x]
  ├── Expansion Speed: [1, 2, ...] cells/tick
  └── Auto-Spawn Interval: [0=off, 10, 20, ...] ticks
```

---

### Algorithms 2-8 (Summary)

| # | Name | Package | Trigger Types |
|---|---|---|---|
| 2 | Brian's Brain | `behaviors-brain/` | activate=born, deactivate=dying→dead, stable=alive |
| 3 | Langton's Ant | `behaviors-ant/` | activate=enter, deactivate=leave, stable=ant on cell |
| 4 | Bounce | `behaviors-bounce/` | activate=enter (or edge), stable=ball in cell, deactivate=leave |
| 5 | Pulse Wave | `behaviors-pulse/` | activate=pulse hits, stable=in pulse, deactivate=pulse passes |
| 6 | Raindrops | `behaviors-raindrops/` | activate=ring hits, stable=in ring, deactivate=ring passes |
| 7 | DLA Growth | `behaviors-dla/` | activate=attachment, stable=in structure |
| 8 | Glider Gun | `behaviors-glider/` | activate=glider enters, stable=in glider, deactivate=leaves |

Each algorithm follows `BehaviorEngine` interface and manages internal timing.

---

## Phase 4: Voice Menu Updates (4 Targets)

**File:** `packages/platform-core/src/index.ts`

Add "Scanned Target" to L3: Voice menu:

```
L3: Voice
  ├── Activate Target: [0, 1, 2, 3]  ← was "Birth Target"
  ├── Stable Target: [0, 1, 2, 3]    ← was "State Target" (algorithm stable)
  ├── Deactivate Target: [0, 1, 2, 3] ← was "Death Target"
  ├── Scanned Target: [0, 1, 2, 3] ← NEW (scanning trigger)
  └── ... (rest unchanged)
```

**Note:** Mapping is consistent across ALL algorithms. Same (x,y) = same note. Only trigger TYPE changes which channel/velocity/duration is used.

---

## Phase 5: Menu System Full Structure

```
Root Menu
├── L1: Life
│   ├── Active: [sequencer | life | shapes | brain | ant | bounce | pulse | raindrops | dla | glider]
│   ├── Step Rate: [1/16, 1/8, 1/4, 1/2, 1/1] ← NEW general param
│   ├── Sequencer Config (if active=sequencer)
│   │   └── (no config - simplest)
│   ├── Conway+ Config (if active=life)
│   │   ├── Random Cells/Tick: [0, 1, 2, ...]
│   │   └── Random Interval: [1, 2, 4, ...]
│   ├── Shapes Config (if active=shapes)
│   │   ├── Shape Type: [ring | heart | star | plus | x]
│   │   ├── Expansion Speed: [1, 2, ...]
│   │   └── Auto-Spawn Interval: [0=off, 10, 20, ...]
│   └── ... (per-behavior config groups)
├── L2: Sense ← Scanning config (UNCHANGED in behavior!)
│   ├── Scan Mode: [immediate | scanning]
│   ├── Scan Axis: [rows | columns] (if scanning)
│   ├── Scan Unit: [1/16, 1/8, ...] (if scanning)
│   ├── Scan Direction: [forward | reverse] (if scanning)
│   ├── Event Triggers: [on | off]
│   ├── Event Pattern: [All | Odd/Even]
│   ├── State Notes: [on | off]  ← controls "scanned" generation
│   ├── X Axis Config
│   └── Y Axis Config
├── L3: Voice
│   ├── Activate Target: [0, 1, 2, 3]  ← algorithm: activate
│   ├── Stable Target: [0, 1, 2, 3]    ← algorithm: stable
│   ├── Deactivate Target: [0, 1, 2, 3] ← algorithm: deactivate
│   ├── Scanned Target: [0, 1, 2, 3] ← NEW: scanning trigger
│   └── ... (rest unchanged)
└── ... (rest unchanged)
```

---

## Implementation Order

| Phase | Items | Rationale |
|---|---|---|
| 0 | Rename 4 trigger types (birth→activate, death→deactivate, state_on→scanned) + add `stable` | Foundation: 4 types |
| 0.5 | Add `algorithmStepUnit` + `scanned` target | General step rate + scanning config |
| 1 | Extract Sequencer + remove populationMode + behavior registry | Core architectural change |
| 2 | LED rendering updates (4 trigger types) | Visual feedback |
| 3.0 | Sequencer (extracted) | Simplest, manual toggle only |
| 3.-1 | Conway+ (extend existing) | Add random cells feature |
| 3.1 | Shapes (5 types: ring, heart, star, plus, x) | User's original request |
| 3.2-8 | Remaining 7 algorithms | Can be parallelized |
| 4 | Voice menu: add "Scanned Target" | 4 mapping targets |
| 5 | Full menu integration | Depends on Phase 1 + each algorithm |

---

## Summary of File Changes

| File | Action |
|---|---|
| `packages/interpretation-core/src/index.ts` | 4 types: `birth`→`activate`, `death`→`deactivate`, `state_on`→`scanned`; add `stable` |
| `packages/mapping-core/src/index.ts` | 4 targets: rename + add `stable` + `scanned`; update `targetForKind()` |
| `packages/mapping-core/src/config/default-mapping.json` | Rename keys + ADD `"scanned"` key |
| `packages/behavior-api/src/index.ts` | Add `CellTriggerType` with 4 types + `triggerTypes` to `BehaviorRenderModel` |
| `packages/behavior-api/src/registry.ts` | NEW: behavior registry |
| `packages/behaviors-life/src/index.ts` | Extend for Conway+ (random cells) |
| `packages/behaviors-sequencer/` | NEW: extracted from "grid" mode |
| `packages/behaviors-shapes/` | NEW: 5 shape types |
| `packages/behaviors-shapes/src/shapes.ts` | NEW: shape geometry functions |
| `packages/behaviors-brain/` | NEW |
| `packages/behaviors-ant/` | NEW |
| `packages/behaviors-bounce/` | NEW |
| `packages/behaviors-pulse/` | NEW |
| `packages/behaviors-raindrops/` | NEW |
| `packages/behaviors-dla/` | NEW |
| `packages/behaviors-glider/` | NEW |
| `packages/platform-core/src/index.ts` | Remove populationMode, add algorithmStepUnit, registry integration, menu update, add "Scanned Target" |
| `docs/menu-and-controls-spec.md` | Update to match |

---

## Future: Multi-Layer Architecture (Design Only)

The current single-`behaviorState: TState` in `PlatformState` will evolve to:

```typescript
type LayerState = {
  id: number;           // 0-7
  behaviorId: string;    // which algorithm
  behaviorState: unknown; // opaque per-behavior state
  mappingConfig: MappingConfig; // per-layer mapping (including scanned target)
  muted: boolean;
  solo: boolean;
};

type PlatformState = {
  // ...existing...
  layers: LayerState[8];
  activeLayer: number; // 0-7
};
```

Each layer renders independently, triggers mix to a single output stream. **Not** part of current plan but informs design (keep `BehaviorEngine` interface generic enough to be layer-agnostic).

---

**Plan saved with correct 4-trigger-type design.**
