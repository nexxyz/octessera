# Pi DSP Voice And Momentary Profile

This note records Pi-side voice, momentary FX, and synth-slot parallel profiling after the 3-slot FX bus work.

Target: Raspberry Pi running `octessera-pi` with `--profile-dsp`.

## 2026-07-15: 128-frame default profile

Setup:

- 44.1 kHz, 128-frame render blocks.
- `OCTESSERA_PI_PROFILE_MODE=full` and `overload`.
- No throttling observed.

Representative full-profile rows:

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

Overload rows at 128 frames:

| Scenario | Avg raw ratio | P95 | P99 / Max | Notes |
|---|---:|---:|---:|---|
| `synth_cross_slot_96_steal` | 1.065 | 1.168 | 1.293 | Voice stealing still leaves 64 active synth voices, which is too heavy. |
| `sample_cross_slot_96_steal` | 0.837 | 0.841 | 0.904 | 64 sample voices stayed under budget. |
| `mixed_cross_slot_48_48_steal` | 0.948 | 0.952 | 1.097 | Mixed 32 synth + 32 sample can miss deadlines. |

Recommendation: keep current shipped voice and momentary budgets. Do not raise synth voices based on the 32-voice isolated result; mixed overload shows the margin disappears.

## Synth-slot parallelism

At the current 128-frame block size, `OCTESSERA_SYNTH_SLOT_WORKERS=2` and `3` enabled the worker pool but dispatched zero blocks. The engine's parallel gate requires at least 256 frames, so the current low-latency Pi path does not use synth-slot parallel rendering.

At 256-frame blocks, overload profiling showed the parallel path can help if it is configured carefully:

| Scenario | Workers | Avg raw ratio | P95 | P99 / Max | Dispatch |
|---|---:|---:|---:|---:|---:|
| `synth_cross_slot_96_steal` | 0 | 1.049 | 1.055 | 1.264 | 0/0 |
| `synth_cross_slot_96_steal` | 2 | 0.579 | 0.590 | 0.706 | 48/48 |
| `synth_cross_slot_96_steal` | 3 | 0.934 | 1.050 | 1.080 | 48/48 |
| `mixed_cross_slot_48_48_steal` | 0 | 0.943 | 0.968 | 1.049 | 0/0 |
| `mixed_cross_slot_48_48_steal` | 2 | 0.711 | 0.714 | 0.803 | 48/48 |
| `mixed_cross_slot_48_48_steal` | 3 | 0.713 | 0.731 | 0.798 | 48/48 |

Recommendation: do not enable synth-slot workers by default for the current 128-frame low-latency path; they cannot dispatch there. If Octessera gets a high-headroom Pi audio mode at 256-frame blocks, prefer 2 workers first. Three workers helped mixed overload about the same as two, but was worse for synth-only overload on this Pi.
