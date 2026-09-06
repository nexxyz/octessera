# Pi DSP Voice And Momentary Profile

This note separates historical Raspberry evidence, the frozen Orange routing
comparison, and the current Orange product Capacity qualification. Every
measurement dated 2026-07-15 and every table in the Raspberry section is
Raspberry Pi Zero 2 W evidence only.

Historical executor fields remain readable in benchmark evidence, but active
selection exposes only Inline and routing-tree execution.

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
  -WorkerTimingMode enabled `
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
