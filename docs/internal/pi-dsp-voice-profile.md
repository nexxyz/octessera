# Pi DSP Voice And Momentary Profile

This note separates historical Raspberry evidence, the frozen Orange routing
comparison, and the current Orange product Capacity qualification. Every
measurement dated 2026-07-15 and every table in the Raspberry section is
Raspberry Pi Zero 2 W evidence only.

Historical executor fields remain readable in benchmark evidence, but active
selection exposes only Inline and routing-tree execution.

## Schema-13 practical continuity campaign

The completed campaign used schema-13 result evidence. All runs used source
commit `218b3e7eb7854447d436d7e1df0b4ddabea50ebe`, a 120-second measurement,
44.1 kHz, I16 stereo, `SCHED_FIFO` priority 70 on CPU 1, and no safety abort.

| Board / executor | Diagnostic artifact SHA-256 | Geometry: output / ALSA period / internal / lookahead / effective |
|---|---|---|
| Raspberry Inline | `d8ca8bb516a7314c7566e2e4222133f47838b0c7e6ddad5b003b0f19f833627a` | 256 / 64 / 128 / 0 / 256 |
| Raspberry Multicore | `d8ca8bb516a7314c7566e2e4222133f47838b0c7e6ddad5b003b0f19f833627a` | 256 / 64 / 128 / 128 / 384 |
| Orange Inline | `dd8997cd202db5a89cba53c9d7c6f358a2b8f4726b7914c9b87c9fedb2754546` | 128 / 32 / 32 / 0 / 128 |
| Orange Multicore | `5fdc06bd3a76a4c3b712487a227ea4428a7765b85a203b6889b08349cbaf9bab` | 256 / 64 / 64 / 64 / 320 |

### Grading and evidence boundaries

Practical continuity grade is the worst of these five incident counts for a
completed run:

1. repeated-quantum incidents;
2. silent-quantum incidents;
3. conservative whole-run ALSA recovery journal incidents;
4. CPAL stream errors; and
5. CPAL device errors.

The exact grade is **Stable** for 0–1, **Stretched** for 2–4, and
**Compromised** for 5+. Callback-duration over-budget counts are reported
separately and are explanatory only; they are not one of the five practical
grading inputs. Native strict `status` is also reported separately and remains
the native pass/fail contract.

Persistent-output provenance describes pre-mute callback consumption. It does
not prove literal analogue output at the DAC. The campaign therefore makes no
DAC or analogue-output proof claim. The ALSA column is a conservative count
from the whole-run unit journal, not phase-exact EPIPE observability. CPAL
stream/device errors were 0 / 0 in every row.

### Raspberry Pi Zero 2 W results

`Repeat` and `silence` cells are incidents / PCM frames. `Strict` is the native
schema-13 result `status`; `CB over` is callback-duration over-budget count.

| Executor | Units | Strict | Practical | Repeat | Silence | ALSA journal | CPAL stream / device | CB over | P99.9 | Max | Temperature | Evidence |
|---|---:|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| Inline | U8 | fail | Stable | 0 / 0 | 0 / 0 | 0 | 0 / 0 | 1473 | 1.05 | 1.135 | 54.768 °C | `5c36364ddee844c9a9d33bf3f6d1809e` |
| Inline | U12 | fail | Stable | 0 / 0 | 0 / 0 | 1 | 0 / 0 | 18037 | 1.21 | 1.289 | 56.920 °C | `ebb17d2845a441a38b90da6b1596b22d` |
| Inline | U16 | fail | Stretched | 0 / 0 | 0 / 0 | 3 | 0 / 0 | 16162 | 1.63 | 1.798 | 58.534 °C | `1f5c228345024932a1ad820ad629492f` |
| Multicore | U12 | pass | Stable | 0 / 0 | 0 / 0 | 0 | 0 / 0 | 0 | .65 | .913 | 61.762 °C | `b70306a8d6af4e9ab63bed2c667f419d` |
| Multicore | U16 | fail | Stretched | 2 / 256 | 2 / 256 | 0 | 0 / 0 | 1 | .77 | 1.242 | 63.376 °C | `3ef5a7867e5d4fbf8ae55c61923b5acc` |
| Multicore | U24 | fail | Stable | 0 / 0 | 0 / 0 | 0 | 0 / 0 | 133 | 1.05 | 1.525 | 66.066 °C | `5e8da09829534556b64d097305f0ef3d` |

### Orange Pi Zero 2 W results

| Executor | Units | Strict | Practical | Repeat | Silence | ALSA journal | CPAL stream / device | CB over | P99.9 | Max | Temperature | Evidence |
|---|---:|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| Inline | U12 | pass | Stable | 0 / 0 | 0 / 0 | 0 | 0 / 0 | 0 | .76 | .915 | 79.199 °C | `8ce5af76f71b4f878c68e55759248fcf` |
| Inline | U16 | fail | Stable | 0 / 0 | 0 / 0 | 0 | 0 / 0 | 1 | .87 | 1.028 | 80.657 °C | `4fa6db3a55c64502b459fa196f10536b` |
| Inline | U24 | fail | Stable | 0 / 0 | 0 / 0 | 1 | 0 / 0 | 1551 | 1.09 | 1.197 | 81.062 °C | `16e8621b3c1845c9b6397c2f434217ee` |
| Multicore | U16 | pass | Stable | 0 / 0 | 0 / 0 | 0 | 0 / 0 | 0 | .55 | .664 | 83.816 °C | `921b989aff174103a2634e6df3a42d96` |
| Multicore | U32 | pass | Compromised | 7 / 448 | 7 / 448 | 0 | 0 / 0 | 0 | .52 | .642 | 85.517 °C | `a52fd79ce5054a3692a377cb66377c93` |
| Multicore | U36 | pass | Compromised | 23 / 1472 | 23 / 1472 | 0 | 0 / 0 | 0 | .49 | .761 | 82.844 °C | `9885a6b409de4b9ca0beb2e93199b9b9` |

Orange remained at 1.416 GHz with cooling state 0 throughout this campaign.

### Interpretation and product decision

The Raspberry Inline rows are practically Stable through U12 and Stretched at
U16 because the whole-run ALSA journal count reaches 3. The Raspberry
Multicore rows are Stable at U12, Stretched at U16 because both provenance
incident counts are 2, and Stable again at U24. That non-monotonic single-run
result does not support a hard capacity inference.

The Orange Inline rows remain practically Stable through U24, while the
Orange Multicore result is Stable at U16 and Compromised at U32 and U36 from
repeated and silent callback consumption. Native strict status stayed `pass`
for those two Orange rows because their callback and native lifecycle counters
were clean; the practical continuity grade still rules them out for product
capacity. The U32/U36 results do not justify raising the product limit.

- Keep Orange Capacity at U16 and keep the existing Inline limits. Do not raise
  Capacity to U32 or U36.
- Keep Raspberry on the product Inline executor and current limits. Do not
  expose Multicore based on this inconsistent diagnostic result.
- No product defaults changed.

## Orange frame mapping and evidence boundary

Orange Inline uses a 128-frame production CPAL output buffer, ALSA period 32,
and internal block 32. Capacity uses output 256, ALSA period 64, internal 64,
and routing lookahead 64. Its current product qualification uses the diagnostic
`capacity_analogue_16` routing-tree run and does not change production defaults.

The Orange runner's offline rows consume a configured measurement chunk and
report that chunk separately from `internal_block_frames`. The chunk controls
how many samples are pulled per timing observation; it is not a live CPAL
output or callback frame count. Offline raw ratios locate computational knees,
not live xruns, deadlines, or recovered `EPIPE` events. The current CPAL/ALSA
path cannot count internally recovered `EPIPE` events, so these reports do not
establish zero xruns or change capabilities.

Orange runs use the existing scenarios through
`tools/orange-pi/run-orange-capability-study.ps1 -Mode Dsp64` or `-Mode Dsp256`;
non-print runs require `-AllowServiceInterruption`.

## Orange live benchmark procedure

Preview the current product Capacity command without contacting a board:

```powershell
./tools/orange-pi/run-orange-capability-study.ps1 -Mode LiveAudioBenchmark `
  -Scenario capacity_analogue_16 -OutputFrames 256 -EngineBlockFrames 64 `
  -MeasureSeconds 120 -ExecutorMode routing_tree_persistent `
  -WorkerTimingMode disabled `
  -Artifact target/orange-pi-cross-diagnostics/routing-tree-benchmark/benchmark-voice-pools-128/octessera-pi `
  -Metadata target/orange-pi-cross-diagnostics/routing-tree-benchmark/benchmark-voice-pools-128/octessera-pi.metadata.json `
  -AllowServiceInterruption -PrintOnly
./tools/orange-pi/run-orange-live-audio-matrix.ps1 -PrintOnly
```

The frozen routing comparison matrix is A: output 256, period 64, internal 128,
lookahead 128, and 11 scenarios, followed by the selected A120 repeat. It is
comparison evidence only; it is not the current product Capacity qualification
or current default. The current Capacity run above is the product qualification
command.

Readiness, progress, and result evidence use the current schema contract, while
the tooling retains historical executor and schema parsing for old evidence.
Internally recovered `EPIPE` remains unobservable, so these results make no
zero-xrun or audible-quality claim.

## Raspberry Pi Zero 2 W: 2026-07-15 128-frame default profile

Setup for all measurements in this section:

- Raspberry Pi Zero 2 W hardware only.
- 44.1 kHz, 128-frame offline measurement/render blocks.
- `OCTESSERA_PI_PROFILE_MODE=full` and `overload`.
- No throttling observed during the Raspberry runs.

Representative Raspberry-only full-profile rows:

| Scenario | Avg raw ratio | P95 | P99 / Max | Notes |
|---|---:|---:|---:|---|
| `synth_ramp_16` | 0.392 | 0.399 | 0.454 | Current shipped synth voice budget is safe. |
| `synth_ramp_32` | 0.610 | 0.630 | 0.721 | Headroom exists, but not enough to raise shipped limits without more mixed-load testing. |
| `synth_ramp_64` | 1.056 | 1.105 | 1.267 | Unsafe at 128-frame blocks. |
| `sample_ramp_64` | 0.836 | 0.848 | 0.890 | Current sample voice ceiling is near the high-load range but stayed under budget in this isolated profile. |
| `mixed_ramp_16_16` | 0.544 | 0.552 | 0.614 | Safe. |
| `mixed_ramp_32_32` | 0.944 | 0.959 | 1.083 | Occasional deadline miss risk. |
| `bus_heavy_6_bus_fx_2_global` | 0.573 | 0.587 | 0.717 | Safe. |
| `momentary_combined` | 0.491 | 0.493 | 0.790 | Current 2 momentary FX budget is safe in this profile. |

Raspberry-only overload rows at 128 frames:

| Scenario | Avg raw ratio | P95 | P99 / Max | Notes |
|---|---:|---:|---:|---|
| `synth_cross_slot_96_steal` | 1.065 | 1.168 | 1.293 | Voice stealing still leaves 64 active synth voices, which is too heavy. |
| `sample_cross_slot_96_steal` | 0.837 | 0.841 | 0.904 | 64 sample voices stayed under budget. |
| `mixed_cross_slot_48_48_steal` | 0.948 | 0.952 | 1.097 | Mixed 32 synth + 32 sample can miss deadlines. |

The Raspberry recommendation was to keep current shipped voice and momentary
budgets. The isolated 32-voice result was not sufficient to raise synth limits;
mixed overload removed the apparent margin.

## Raspberry-only synth-slot parallelism measurements

At the Raspberry 128-frame measurement block size, the legacy synth worker-pool
setting at `2` or `3` enabled the worker pool but dispatched zero blocks. The
engine parallel gate requires at least 256 internal frames.

The following 256-frame rows are also Raspberry Pi Zero 2 W measurements only:

| Scenario | Workers | Avg raw ratio | P95 | P99 / Max | Dispatch |
|---|---:|---:|---:|---:|---:|
| `synth_cross_slot_96_steal` | 0 | 1.049 | 1.055 | 1.264 | 0/0 |
| `synth_cross_slot_96_steal` | 2 | 0.579 | 0.590 | 0.706 | 48/48 |
| `synth_cross_slot_96_steal` | 3 | 0.934 | 1.050 | 1.080 | 48/48 |
| `mixed_cross_slot_48_48_steal` | 0 | 0.943 | 0.968 | 1.049 | 0/0 |
| `mixed_cross_slot_48_48_steal` | 2 | 0.711 | 0.714 | 0.803 | 48/48 |
| `mixed_cross_slot_48_48_steal` | 3 | 0.713 | 0.731 | 0.798 | 48/48 |

Raspberry's Phase 1 behavior uses a 256-frame runtime output buffer and a
128-frame internal render quantum. The Orange result above is Orange-specific.
`OCTESSERA_AUDIO_OUTPUT_BUFFER_FRAMES` and
`OCTESSERA_AUDIO_RENDER_QUANTUM_FRAMES` remain profiling overrides.

`docs/internal/pi-audio-buffer-experiment.md` records the Raspberry
128-frame internal/output experiment. It retained the 256-frame output, the
128-frame internal render quantum, the disabled default worker pool, the safe
momentary FX cache, and the profiling tooling; it did not establish an Orange
output or callback result. The corrected Orange live comparison is recorded
above.
