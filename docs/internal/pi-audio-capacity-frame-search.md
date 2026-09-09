# Pi audio capacity frame search

> **Approved for diagnostic execution.** The table is the human-owned input to
> the frame-search harness. It does not change product defaults by itself.

The campaign chooses one canonical frame constellation for Raspberry Inline,
Raspberry Multicore, Orange Inline, and Orange Multicore. It locates the
Stable→Stretched and Stretched→Compromised U borders and quantifies Multicore
gain per board. Each preliminary selected cell is 180 seconds, repeated exactly
twice. Final winners receive one 600-second soak each.

## Grading

For each 180-second repetition:

```text
worst = max(repeat incidents, silent incidents,
            whole-run ALSA recovery journal incidents,
            CPAL stream errors, CPAL device errors)
```

| Grade | 180-second repetition `worst` | 600-second soak `worst` |
|---|---:|---:|
| Stable | 0–2 | 0–9 |
| Stretched | 3–7 | 10–24 |
| Compromised | 8+ | 25+ |

The paired-cell grade is the worse grade of its two repetitions; incident
counts are never summed. Callback over-budget and native status are recorded
separately from this grade. Evidence is pre-mute callback consumption, not
literal analogue capture. Stretched is accepted capacity.

## Candidate profiles

`U` is any integer from 1–42 under benchmark pools of 128. Rows marked
**diagnostic alternative** are candidate tuples requiring narrow validation
support later; they are not implementation commitments.

| Profile | Board | Mode | Output | Period | Internal | Lookahead | Effective | Seed U | Role |
|---|---|---|---:|---:|---:|---:|---:|---|---|
| RI64 | Raspberry | Inline | 256 | 64 | 64 | 0 | 256 | 12, 24, 32 | diagnostic alternative |
| RI128 | Raspberry | Inline | 256 | 64 | 128 | 0 | 256 | 8, 12, 16, 24, 32 | current anchor |
| RI512 | Raspberry | Inline | 512 | 128 | 128 | 0 | 512 | 12, 24, 32 | diagnostic alternative |
| RM64 | Raspberry | Multicore | 256 | 64 | 64 | 64 | 320 | 12, 24, 32 | low-latency candidate; prior short evidence poor; diagnostic alternative |
| RM128 | Raspberry | Multicore | 256 | 64 | 128 | 128 | 384 | 12, 16, 20, 24, 32 | current diagnostic anchor; non-monotonic check |
| RM256 | Raspberry | Multicore | 256 | 64 | 256 | 256 | 512 | 12, 24, 32 | diagnostic alternative |
| OI32 | Orange | Inline | 128 | 32 | 32 | 0 | 128 | 12, 16, 24, 32 | current anchor |
| OI64 | Orange | Inline | 128 | 32 | 64 | 0 | 128 | 12, 24, 32 | diagnostic alternative |
| OI256 | Orange | Inline | 256 | 64 | 64 | 0 | 256 | 12, 24, 32 | diagnostic alternative |
| OM32 | Orange | Multicore | 128 | 32 | 32 | 32 | 160 | 12, 24, 32 | diagnostic alternative |
| OM64 | Orange | Multicore | 256 | 64 | 64 | 64 | 320 | 12, 16, 24, 32 | current anchor |
| OM128 | Orange | Multicore | 256 | 64 | 128 | 128 | 384 | 12, 24, 32 | diagnostic alternative |

## Seed queue

This is the human-editable queue. Each row is one paired cell and expands to
two physical repetitions later. `state` starts as `pending`. W1 is U12 across
all profiles; W2 is historical anchors other than U12/U24/U32 (including
RI128 U8); W3 is U24; W4 is U32.

| Cell | Wave | Profile | Board | Mode | Output | Period | Internal | Lookahead | Effective | U | Sec | Reps | State | Condition |
|---|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| RI64-U12 | W1 | RI64 | Raspberry | Inline | 256 | 64 | 64 | 0 | 256 | 12 | 180 | 2 | pending | always |
| RI128-U12 | W1 | RI128 | Raspberry | Inline | 256 | 64 | 128 | 0 | 256 | 12 | 180 | 2 | pending | always |
| RI512-U12 | W1 | RI512 | Raspberry | Inline | 512 | 128 | 128 | 0 | 512 | 12 | 180 | 2 | pending | always |
| RM64-U12 | W1 | RM64 | Raspberry | Multicore | 256 | 64 | 64 | 64 | 320 | 12 | 180 | 2 | pending | always |
| RM128-U12 | W1 | RM128 | Raspberry | Multicore | 256 | 64 | 128 | 128 | 384 | 12 | 180 | 2 | pending | always |
| RM256-U12 | W1 | RM256 | Raspberry | Multicore | 256 | 64 | 256 | 256 | 512 | 12 | 180 | 2 | pending | always |
| OI32-U12 | W1 | OI32 | Orange | Inline | 128 | 32 | 32 | 0 | 128 | 12 | 180 | 2 | pending | always |
| OI64-U12 | W1 | OI64 | Orange | Inline | 128 | 32 | 64 | 0 | 128 | 12 | 180 | 2 | pending | always |
| OI256-U12 | W1 | OI256 | Orange | Inline | 256 | 64 | 64 | 0 | 256 | 12 | 180 | 2 | pending | always |
| OM32-U12 | W1 | OM32 | Orange | Multicore | 128 | 32 | 32 | 32 | 160 | 12 | 180 | 2 | pending | always |
| OM64-U12 | W1 | OM64 | Orange | Multicore | 256 | 64 | 64 | 64 | 320 | 12 | 180 | 2 | pending | always |
| OM128-U12 | W1 | OM128 | Orange | Multicore | 256 | 64 | 128 | 128 | 384 | 12 | 180 | 2 | pending | always |
| RI128-U8 | W2 | RI128 | Raspberry | Inline | 256 | 64 | 128 | 0 | 256 | 8 | 180 | 2 | pending | always |
| RI128-U16 | W2 | RI128 | Raspberry | Inline | 256 | 64 | 128 | 0 | 256 | 16 | 180 | 2 | pending | always |
| RM128-U16 | W2 | RM128 | Raspberry | Multicore | 256 | 64 | 128 | 128 | 384 | 16 | 180 | 2 | pending | always |
| RM128-U20 | W2 | RM128 | Raspberry | Multicore | 256 | 64 | 128 | 128 | 384 | 20 | 180 | 2 | pending | always |
| OI32-U16 | W2 | OI32 | Orange | Inline | 128 | 32 | 32 | 0 | 128 | 16 | 180 | 2 | pending | always |
| OM64-U16 | W2 | OM64 | Orange | Multicore | 256 | 64 | 64 | 64 | 320 | 16 | 180 | 2 | pending | always |
| RI64-U24 | W3 | RI64 | Raspberry | Inline | 256 | 64 | 64 | 0 | 256 | 24 | 180 | 2 | pending | skip if profile dominated/unsafe |
| RI128-U24 | W3 | RI128 | Raspberry | Inline | 256 | 64 | 128 | 0 | 256 | 24 | 180 | 2 | pending | skip if profile dominated/unsafe |
| RI512-U24 | W3 | RI512 | Raspberry | Inline | 512 | 128 | 128 | 0 | 512 | 24 | 180 | 2 | pending | skip if profile dominated/unsafe |
| RM64-U24 | W3 | RM64 | Raspberry | Multicore | 256 | 64 | 64 | 64 | 320 | 24 | 180 | 2 | pending | skip if profile dominated/unsafe |
| RM128-U24 | W3 | RM128 | Raspberry | Multicore | 256 | 64 | 128 | 128 | 384 | 24 | 180 | 2 | pending | skip if profile dominated/unsafe |
| RM256-U24 | W3 | RM256 | Raspberry | Multicore | 256 | 64 | 256 | 256 | 512 | 24 | 180 | 2 | pending | skip if profile dominated/unsafe |
| OI32-U24 | W3 | OI32 | Orange | Inline | 128 | 32 | 32 | 0 | 128 | 24 | 180 | 2 | pending | skip if profile dominated/unsafe |
| OI64-U24 | W3 | OI64 | Orange | Inline | 128 | 32 | 64 | 0 | 128 | 24 | 180 | 2 | pending | skip if profile dominated/unsafe |
| OI256-U24 | W3 | OI256 | Orange | Inline | 256 | 64 | 64 | 0 | 256 | 24 | 180 | 2 | pending | skip if profile dominated/unsafe |
| OM32-U24 | W3 | OM32 | Orange | Multicore | 128 | 32 | 32 | 32 | 160 | 24 | 180 | 2 | pending | skip if profile dominated/unsafe |
| OM64-U24 | W3 | OM64 | Orange | Multicore | 256 | 64 | 64 | 64 | 320 | 24 | 180 | 2 | pending | skip if profile dominated/unsafe |
| OM128-U24 | W3 | OM128 | Orange | Multicore | 256 | 64 | 128 | 128 | 384 | 24 | 180 | 2 | pending | skip if profile dominated/unsafe |
| RI64-U32 | W4 | RI64 | Raspberry | Inline | 256 | 64 | 64 | 0 | 256 | 32 | 180 | 2 | pending | skip if profile dominated/unsafe |
| RI128-U32 | W4 | RI128 | Raspberry | Inline | 256 | 64 | 128 | 0 | 256 | 32 | 180 | 2 | pending | skip if profile dominated/unsafe |
| RI512-U32 | W4 | RI512 | Raspberry | Inline | 512 | 128 | 128 | 0 | 512 | 32 | 180 | 2 | pending | skip if profile dominated/unsafe |
| RM64-U32 | W4 | RM64 | Raspberry | Multicore | 256 | 64 | 64 | 64 | 320 | 32 | 180 | 2 | pending | skip if profile dominated/unsafe |
| RM128-U32 | W4 | RM128 | Raspberry | Multicore | 256 | 64 | 128 | 128 | 384 | 32 | 180 | 2 | pending | skip if profile dominated/unsafe |
| RM256-U32 | W4 | RM256 | Raspberry | Multicore | 256 | 64 | 256 | 256 | 512 | 32 | 180 | 2 | pending | skip if profile dominated/unsafe |
| OI32-U32 | W4 | OI32 | Orange | Inline | 128 | 32 | 32 | 0 | 128 | 32 | 180 | 2 | pending | skip if profile dominated/unsafe |
| OI64-U32 | W4 | OI64 | Orange | Inline | 128 | 32 | 64 | 0 | 128 | 32 | 180 | 2 | pending | skip if profile dominated/unsafe |
| OI256-U32 | W4 | OI256 | Orange | Inline | 256 | 64 | 64 | 0 | 256 | 32 | 180 | 2 | pending | skip if profile dominated/unsafe |
| OM32-U32 | W4 | OM32 | Orange | Multicore | 128 | 32 | 32 | 32 | 160 | 32 | 180 | 2 | pending | skip if profile dominated/unsafe |
| OM64-U32 | W4 | OM64 | Orange | Multicore | 256 | 64 | 64 | 64 | 320 | 32 | 180 | 2 | pending | skip if profile dominated/unsafe |
| OM128-U32 | W4 | OM128 | Orange | Multicore | 256 | 64 | 128 | 128 | 384 | 32 | 180 | 2 | pending | skip if profile dominated/unsafe |

## Adaptive rules

- Retain at most two nondominated profiles per board/mode.
- Bisect each observed grade crossing toward adjacent U values. Allocate four
  adaptive paired cells per retained profile first, then up to eight for an
  unresolved finalist while room remains inside the global maximum of 32 paired
  adaptive cells (64 physical runs).
- If U32 is non-compromised, probe U36 and then U42. If the low end is not
  Stable, probe downward at adjacent U values.
- A reversal triggers local probes. A later Stable island above a prior
  Compromised result cannot be selected while that reversal is unresolved.
- Append adaptive work as paired rows in the next wave, or mark speculative
  seed rows skipped. Adaptive rows remain subject to the 20-hour wall.

## Interlacing and repetition

The harness expands each paired queue row into two physical repetitions. Use
separate repetition epochs, alternate boards and modes where possible, and aim
for at least four other runs between repetitions of the same cell. A smaller
terminal adaptive batch uses the maximum available spacing; a singleton remains
inconclusive. Rep 2 uses a different deranged profile/U order from rep 1, not a
reverse or rotation. Adaptive rows accumulate into explicit waves.

## Winner and gain rules

For each board/mode, accepted capacity is the highest tested U that is Stable or
Stretched, has two structurally complete repetitions, and has no unresolved
reversal or Compromised result below it. Rank candidates by accepted U, then
lower effective latency, lower incident count, and cleaner native/callback
timing. The winner is the canonical profile for that board/mode.

| Quantity | Formula |
|---|---|
| Raspberry Multicore gain | `ΔU_R = U_RM − U_RI` |
| Orange Multicore gain | `ΔU_O = U_OM − U_OI` |
| Gain percent | `100 × ΔU / U_Inline` |
| Synth-voice delta | `3 × ΔU` |
| Sample-voice delta | `ΔU` |
| Effective latency | `latency_ms = effective / 44.1` |

## Final soaks

There is one 600-second soak per winner: four winners and 40 measured minutes.

| Soak | Board | Mode | Profile | Geometry | U | Sec | Reps | State |
|---|---|---|---|---|---:|---:|---:|---|
| SOAK-RI | Raspberry | Inline | TBD from winner | TBD from winner | TBD | 600 | 1 | pending |
| SOAK-RM | Raspberry | Multicore | TBD from winner | TBD from winner | TBD | 600 | 1 | pending |
| SOAK-OI | Orange | Inline | TBD from winner | TBD from winner | TBD | 600 | 1 | pending |
| SOAK-OM | Orange | Multicore | TBD from winner | TBD from winner | TBD | 600 | 1 | pending |

## Execution result log

| Run | Cell | Rep | State | Native | Grade | Worst | Repeat | Silence | ALSA | CPAL | CB over | P999 | Max | Temp | Artifact | Evidence | Skip reason |
|---|---|---:|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---|---|---|
|  |  |  |  |  |  |  |  |  |  |  |  |  |  |  |  |  |  |

## Budget and tooling constraints

The hard campaign wall-clock maximum is 20 hours; adaptive rows may be skipped
on the fly. The planned measured-time budget is:

| Work | Physical runs | Measured minutes |
|---|---:|---:|
| 42 seed paired cells | 84 | 252 |
| Adaptive maximum | 64 | 192 |
| Four 600-second soaks | 4 | 40 |
| **Total** | **152** | **484 (8h04)** |

Expected wall-clock time is 12–16 hours after transfers, warmup, and
restoration. Stop adding preliminary work when elapsed time threatens 18h45 so
the final soaks and restoration finish under 20 hours.

The harness and one-cell board runners implement narrow table-bound support for
the approved tuples, U1–U42, 180-second compromise retention, and 600-second
soaks. Existing non-study runner behavior and product defaults remain unchanged.
