# Link Menu Tree

This file is part of the canonical split-out menu tree spec. See [`../menu-tree-spec.md`](../menu-tree-spec.md) for the canonical index.

### Link

```

Link event `Delay` waits that many layer link ticks before emitting. `Retrig` adds extra repeats after the original, spaced one tick apart after the delay. Probability is rolled once before scheduling; delayed repeats do not re-roll.
Link
├── BPM: [40..240] step 1  default 120
├── Swing: [0..75] step 1 %  default 0
├── Aux Mappings (group)
│   ├── Aux 1 (group)
│   │   ├── Turn (group)
│   │   │   ├── (none) (action)
│   │   │   └── parameter tree...            ← same shared browser as Play X/Y target selection; Aux turn bindings do not show Range Min/Range Max rows
│   │   └── Click (group)
│   │       ├── (none) (action)
│   │       └── action tree...               ← behavior actions, sample assign, selected FX map-to-grid
│   ├── Aux 2 (group)
│   └── Aux 3 (group)
├── Paused Events: [on | off]              default on; when on, grid input may still emit events while transport is stopped/paused
├── L1: ... (group)                              ← one group per layer
│   ├── Scanning (group)
│   │   ├── Scan Mode: [none | scanning]
│   │   ├── Scan Axis: [rows | columns]           ← visible when scanning
│   │   ├── Scan Unit: [1/32T, 1/32, 1/16T, 1/16, 1/8T, 1/8, 1/4T, 1/4, 1/2T, 1/2, 1/1T, 1/1] ← visible when scanning
│   │   ├── Scan Direction: [forward | reverse]   ← visible when scanning
│   │   ├── Sections: [1 | 2 | 4 | 8]             ← visible when scanning; 1=current behavior, higher values scan smaller lanes
│   │   ├── Instrument: [1..8]                    ← visible when scanning
│   │   ├── Action: [none | note_on | note_off]   ← visible when scanning
│   │   ├── Scan Delay: [0..16]                   ← visible when scanning
│   │   ├── Scan Retrig: [0..8]                   ← visible when scanning
│   │   ├── Empty Inst: [1..8]                    ← visible when scanning
│   │   ├── Empty Trig: [none | note_on | note_off] ← visible when scanning
│   │   ├── Empty Delay: [0..16]                  ← visible when scanning
│   │   └── Empty Retrig: [0..8]                  ← visible when scanning
│   ├── Events (group)
│   │   ├── Event Triggers: [on | off]
│   │   ├── State Notes: [on | off]               default on (all layers)
│   │   ├── On Inst: [1..8]
│   │   ├── On Trig: [none | note_on | note_off]
│   │   ├── On Delay: [0..16]
│   │   ├── On Retrig: [0..8]
│   │   ├── Hold Inst: [1..8]
│   │   ├── Hold Trig: [none | note_on | note_off]
│   │   ├── Hold Delay: [0..16]
│   │   ├── Hold Retrig: [0..8]
│   │   ├── Off Inst: [1..8]
│   │   ├── Off Trig: [none | note_on | note_off]
│   │   ├── Off Delay: [0..16]
│   │   └── Off Retrig: [0..8]
│   ├── Trigger Prob. (group)
│   │   ├── Mode: [zero | custom | full]
│   │   ├── Low Prob: [0..100] step 1
│   │   ├── High Prob: [0..100] step 1
│   │   └── Map Prob Grid (action)
│   ├── Note Mapping (group)
│   │   ├── Low Note: [0..127] step 1          ← lower bound, displayed as note name + MIDI number, e.g. C2 (36)
│   │   ├── High Note: [0..127] step 1         ← upper bound, displayed as note name + MIDI number, e.g. D5 (74)
│   │   ├── Start Note: [0..127] step 1        ← nearest scale start index, displayed as note name + MIDI number, e.g. C4 (60)
│   │   ├── Scale: [chromatic | major | natural_minor | dorian | mixolydian | major_pentatonic | minor_pentatonic | harmonic_minor]
│   │   ├── Root: [C | C# | D | D# | E | F | F# | G | G# | A | A# | B]
│   │   └── Out of Range: [clamp | wrap]
│   ├── X Axis (group)
│   │   ├── Slot 1 (group)
│   │   │   ├── (none) (action)
│   │   │   └── parameter tree...                ← same shared browser as Play X/Y target selection; current numeric bindings expose Range Min/Range Max
│   │   ├── Slot 1 Invert: [on | off]
│   │   ├── Slot 2 (group)
│   │   │   ├── (none) (action)
│   │   │   └── parameter tree...
│   │   ├── Slot 2 Invert: [on | off]
│   │   ├── Pitch Steps (group)
│   │   │   ├── Enabled: [on | off]
│   │   │   ├── Steps: [-16..16] step 1       ← visible when enabled
│   │   │   └── Restart Section: [on | off]   ← visible when enabled; restarts pitch within column sections
│   │   ├── Velocity (group)
│   │   │   ├── Enabled: [on | off]
│   │   │   ├── From: [0..127] step 1         ← visible when enabled
│   │   │   ├── To: [0..127] step 1
│   │   │   ├── Grid Offs: [-7..7] step 1
│   │   │   └── Curve: [linear | curve]
│   │   ├── Filter Cutoff (group)
│   │   │   ├── Enabled: [on | off]
│   │   │   ├── From: [0..127] step 1
│   │   │   ├── To: [0..127] step 1
│   │   │   ├── Grid Offs: [-7..7] step 1
│   │   │   └── Curve: [linear | curve]
│   │   └── Filter Res (group)
│   │       ├── Enabled: [on | off]
│   │       ├── From: [0..127] step 1
│   │       ├── To: [0..127] step 1
│   │       ├── Grid Offs: [-7..7] step 1
│   │       └── Curve: [linear | curve]
│   └── Y Axis (group)
│       └── (same sub-structure as X Axis, modulation target keys use param:N:y:slot, config keys use y.* prefix, defaults: Pitch Steps steps=3; Restart Section affects row sections)
├── L2: ... (group)
├── L3: ... (group)
```
