# 2: Pulses Menu Tree

This file is part of the canonical split-out menu tree spec. See [`../menu-tree-spec.md`](../menu-tree-spec.md) for the canonical index.

### 2: Pulses

```
2: Pulses
├── BPM: [40..240] step 1  default 120
├── Swing: [0..75] step 1 %  default 0
├── Aux Mappings (group)
│   ├── Aux 1 (group)
│   │   ├── Turn (group)
│   │   │   ├── (none) (action)
│   │   │   └── parameter tree...            ← same shared browser as Sparks X/Y target selection
│   │   └── Click (group)
│   │       ├── (none) (action)
│   │       └── action tree...               ← behavior actions, sample assign, selected FX map-to-grid
│   ├── Aux 2 (group)
│   └── Aux 3 (group)
├── Events when paused: [on | off]         default on; when on, grid input may still emit events while transport is stopped/paused
├── L1: ... (group)                              ← one group per layer
│   ├── Scan Mode: [none | scanning]
│   ├── Scan Axis: [rows | columns]           ← visible when scanning
│   ├── Scan Unit: [1/16, 1/8, 1/4, 1/2, 1/1] ← visible when scanning
│   ├── Scan Direction: [forward | reverse]    ← visible when scanning
│   ├── Sections: [1 | 2 | 4 | 8]              ← visible when scanning; 1=current behavior, higher values scan smaller lanes
│   ├── Event Triggers: [on | off]
│   ├── State Notes: [on | off]                    default on (all layers)
│   ├── Instrument Targets (group)
│   │   ├── Activate Action: [none | note_on | note_off]
│   │   ├── Activate Instrument: [1..8]
│   │   ├── Stable Action: [none | note_on | note_off]
│   │   ├── Stable Instrument: [1..8]
│   │   ├── Deactivate Action: [none | note_on | note_off]
│   │   ├── Deactivate Instrument: [1..8]
│   │   ├── Scanned Action: [none | note_on | note_off]
│   │   ├── Scanned Instrument: [1..8]
│   │   ├── Scanned Empty Action: [none | note_on | note_off]
│   │   └── Scanned Empty Instrument: [1..8]
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
│   │   │   └── parameter tree...                ← same shared browser as Sparks X/Y target selection
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
│   │   └── Filter Resonance (group)
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
