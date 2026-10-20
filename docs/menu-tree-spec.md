# Menu Tree Spec

This file and the section trees it links are part of the authoritative menu/control spec rooted at
[`menu-and-controls-spec.md`](menu-and-controls-spec.md). Each section file holds its tree plus the
semantics that only that section owns; cross-cutting rules (value editing, overlays, aux bindings,
save/load) stay in the main spec. Keep both in sync with native menu changes in the same commit.

## Root Menu

```
Root (group)
├── Build (group)
├── Link (group)
├── Shape (group)
├── Play (group)
├── [spacer] (blank visual separator)
└── System (group)
```

Short breadcrumb forms are `B`, `L`, `S`, `P`, and `SYS`.

## Section trees

- [Build](menu-tree/build.md): layers, behavior selector, behavior config rows, Seed
- [Link](menu-tree/link.md): BPM, Swing, Aux Mappings, LFOs, per-layer scanning/events/mapping/axes/Arp
- [Shape](menu-tree/shape.md): instruments, FX buses, global FX, [routing semantics](menu-tree/shape.md#routing-semantics)
- [Play](menu-tree/play.md): Play pages and their grid layouts
- [System](menu-tree/system.md): System tree resolved per platform, and every System action

## Tree notation

- `(group)` opens a submenu; `(action)` or a leading `!` is an action row; `(text, max N)` is a text
  row.
- `[a | b | c]` lists enum options in menu order. Bool rows are written `[on | off]` and display
  `On`/`Off` on the OLED.
- `[min..max] step N` gives the stored range and linear step. Rows on the time, frequency, or
  tail curves ignore that step; see
  [Numeric steps](menu-and-controls-spec.md#numeric-steps).
- `default N` is the value used when nothing is saved. Per-layer and per-slot rows may start with
  other values in the shipped patch (`config/defaults/base.json`).
- Text after `←` is a note: visibility conditions, platform availability, or behavior. Platforms are
  desktop, Raspberry (Gadget or Host USB role), and Orange.
