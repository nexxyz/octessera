# Pi DSP Voice And Momentary Profile

This note separates historical Raspberry evidence from the Orange Phase 1
qualification path. Every measurement dated 2026-07-15 and every table below
is Raspberry Pi Zero 2 W evidence only. None of those ratios are Orange results.

## Orange frame mapping and evidence boundary

Orange's production CPAL output buffer is 256 frames, which maps to 64-frame
internal `EngineSource` blocks. The earlier offline 256-frame comparison
modeled the internal block selected by a live 1024-frame output setting; it did
not open CPAL or run that live output buffer. The corrected live comparison
below opened output 256, observed ALSA period 64, and explicitly selected
engine block 256 with workers 0 and 2. It did not change the production path or
defaults.

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

The Phase 2 runner produced the Orange evidence recorded below. It uses the
fixed target and existing `with-orange-ssh.ps1` transport, stages one
hash-bound reviewed artifact, and launches one `LiveAudioBenchmark` cell in a
unique transient systemd unit. Before the native process receives fixture
events, the host checks the stable production readiness marker, startup thermal
and memory gates, exact DAC ALSA buffer/period geometry, and publishes the
identity-bound schema-2 release JSON. Requested ALSA buffer, negotiated period,
and internal engine block remain distinct from the observed CPAL callback batch.
Callback batches may vary from one positive callback to the requested buffer;
render-duration ratios use actual callback frames, while spacing lateness uses
the fixed ALSA period. Schema-1 benchmark artifacts are rejected.

Preview the single-cell payload or full matrix without contacting a board:

```powershell
./tools/orange-pi/run-orange-capability-study.ps1 -Mode LiveAudioBenchmark `
  -Scenario synth_cross_slot_96_steal -OutputFrames 256 -EngineBlockFrames 64 -Workers 2 `
  -MeasureSeconds 30 -Artifact target/orange-pi-cross/octessera-pi `
  -Metadata target/orange-pi-cross/octessera-pi.metadata.json `
  -AllowServiceInterruption -PrintOnly
./tools/orange-pi/run-orange-live-audio-matrix.ps1 -PrintOnly
```

The matrix is A 256/64 workers 2 for all 11 historical scenarios, a selected
A 120-second repeat, B 512/128 workers 2 for all 11, and C0/C2/C3 1024/256
workers 0/2/3 for synth and mixed steal scenarios. A 120-second single-cell
run requires explicit `-AllowLongRepeat`. The host retains readiness,
progress, result, release, ALSA, sensor, unit/journal, artifact, and
restoration evidence, and stops the exact transient unit on safety, identity,
geometry, heartbeat, or infrastructure failure.

The Phase 3 board run is recorded below. The active matrix stopped at the
mandatory stop before C3 and the new mixed C comparisons; no full 29-cell
completion or capability promotion is recorded.

The old cohort artifact
`2d237ce6573ece49f5b8505715fcf05c48608808a42de56214c68a77520674f0`, manifest
`target/orange-pi-study/orange-live-audio-matrix-9bf73c9397b941a28e9716d5ad203fb9.json`,
passed all 11 A 256/64 cells and all 11 B 512/128 cells with zero over-budget or
CPAL errors. Worst A was p99 0.60 (`synth96`), p99.9 0.63 and max 0.6628499490
(`synth64`); the selected A120 `synth64` repeat passed at p99 0.60, p99.9 0.64,
max 0.7666103703, with zero errors. Worst B was p99 0.58 (`synth96`), p99.9
0.63 (`synth32`), and max 0.6764759791 (`sample64`). A/B thermal max was
62.270 C and minimum available memory was 1,765,468 KiB. Both old C0 cells
passed; old C2 synth showed a worker-policy failure, but schema 2 masked its
delta.

The schema-3 instrumentation-only cohort artifact
`6505e89677b69d2c107e4bb6561cf56168a555a46e0ec1ce2ee0d9e90296411b` recorded C0
`synth96` evidence `orange-study-c88d8651d7504db4a67f1dbe0356a2dd` passing at
p99 0.56, p99.9 0.57, max 0.5639105542, with zero callback/worker errors. C2
`synth96` evidence `orange-study-818e3c79b44d4e1da44d10b8a36ab441` was
classified `over_budget`: p50 0.29, p95 0.55, p99 0.60, p99.9 1.05, max
1.8434431566, 18 over-budget callbacks, zero CPAL errors, and exact worker delta
of 4689 dispatches, 0 light skips, 1344 backoff skips, 21 timing backoffs, 0
failures, not unhealthy. Peak temperature was 59.678 C and minimum available
memory was 1,767,164 KiB. Production was restored after every run. The
mandatory stop means C3 synth and all new mixed C comparisons were not run.

Result artifacts now use schema 3 with optional exact worker delta/policy error
and independent host recomputation; readiness, progress, and release remain
schema 2. Internally recovered `EPIPE` remains unobservable, so these results
make no zero-xrun or audible-quality claim.

Retain the current Orange production 256/64 behavior and existing shared
capabilities/defaults. Do not promote 1024/256 workers, raise limits, or reduce
shared limits from this one board. The live C2 result refutes a clean
high-headroom worker operating point under `synth96`.

## Orange 256-output / period64 / engine256 comparison

Reviewed artifact SHA-256:
`b69ac4c16ccb27eb967fe80cbbc11b6abc87d7ffea54d3d65eb5de7b8a97a5c3`.
The harness independently records output buffer, expected/observed ALSA period,
and effective engine block, then adds the aggregate ratio host-side with
callback/frame consistency checks. The production path and defaults were
unchanged.

- Baseline `synth96`, evidence
  `orange-study-1e9bf108e2f9462ea691e9bdf88f58d0`, used output 256, period 64,
  engine 64, and workers 2; workers were ineffective. It passed with aggregate
  0.5225098032, p50 0.52, p95 0.55, p99 0.59, p99.9 0.64, max 0.6816834074,
  zero over-budget/CPAL/worker errors, and 58.706 C maximum temperature.
- Serial candidate `synth96`, evidence
  `orange-study-1717fdbe81754f1783be393f78441fbc`, used output 256, period 64,
  engine 256, and workers 0. It was `OVER_BUDGET` with aggregate 0.5223046208,
  p50 0.01, p95 2.13, p99 2.19, p99.9 2.27, max 2.3476927084, 5169 over-budget
  callbacks of 15506, zero CPAL/worker errors, and 59.435 C maximum
  temperature.
- Parallel candidate `synth96`, evidence
  `orange-study-9eec41cf2d6246b6a35e85928e981e7a`, used output 256, period 64,
  engine 256, and workers 2; workers were effective. It was `OVER_BUDGET` with
  a worker-policy failure, aggregate 0.3485568855, p50 0.01, p95 2.06, p99
  2.21, p99.9 2.62, max 5.4471237494, 5155 over-budget callbacks of 19376,
  zero CPAL errors, and 59.354 C maximum temperature. The exact worker delta
  was 4751 dispatches, 0 light skips, 1280 backoff skips, 20 timing backoffs,
  0 failures, and not unhealthy.

The serial candidate has essentially the same aggregate cost as the baseline.
Workers lower aggregate DSP cost by roughly one third, but both 256-frame
candidates redistribute work into bursts. Candidate callback batches ranged
64..160 frames serially and 64..192 with workers; the pattern is consistent with
256-frame refills followed by cheaper drain callbacks. Workers lower average
DSP cost but do not eliminate per-callback audio-duration budget overruns and
add tail jitter/backoff. Production
was restored active/enabled after every cell. The mandatory hard stop means all
three mixed 48+48 cells and any expansion or 120-second candidate were not run.
The direct Orange 256/256 candidate was tested and rejected for the current
256/64 ALSA path. Keep Orange production output 256/internal 64 and Raspberry
parallel 256-internal behavior unchanged; this result is Orange-specific and
does not invalidate Raspberry workers. Internally recovered `EPIPE` remains
unobservable, so no actual xrun or audible-quality result is claimed.

Oracle and QA gates passed. Final validation passed 220 Orange-feature tests,
strict Clippy, both focused PowerShell suites, a fresh AArch64 cross-build,
Rust formatting, and line checks. No defaults or capabilities changed.

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

At the Raspberry 128-frame measurement block size, setting
`OCTESSERA_SYNTH_SLOT_WORKERS=2` or `3` enabled the worker pool but dispatched
zero blocks. The engine parallel gate requires at least 256 internal frames.

The following 256-frame rows are also Raspberry Pi Zero 2 W measurements only:

| Scenario | Workers | Avg raw ratio | P95 | P99 / Max | Dispatch |
|---|---:|---:|---:|---:|---:|
| `synth_cross_slot_96_steal` | 0 | 1.049 | 1.055 | 1.264 | 0/0 |
| `synth_cross_slot_96_steal` | 2 | 0.579 | 0.590 | 0.706 | 48/48 |
| `synth_cross_slot_96_steal` | 3 | 0.934 | 1.050 | 1.080 | 48/48 |
| `mixed_cross_slot_48_48_steal` | 0 | 0.943 | 0.968 | 1.049 | 0/0 |
| `mixed_cross_slot_48_48_steal` | 2 | 0.711 | 0.714 | 0.803 | 48/48 |
| `mixed_cross_slot_48_48_steal` | 3 | 0.713 | 0.731 | 0.798 | 48/48 |

Raspberry's existing high-headroom behavior uses 256-frame internal render
blocks and 2 synth-slot workers. Its runtime default output buffer remains 256
frames. The Orange result above is Orange-specific and does not invalidate
Raspberry workers. `OCTESSERA_AUDIO_OUTPUT_BUFFER_FRAMES`,
`OCTESSERA_AUDIO_BLOCK_FRAMES`, and `OCTESSERA_SYNTH_SLOT_WORKERS` remain
profiling overrides.

`docs/internal/pi-audio-buffer-experiment.md` records the Raspberry
128-frame internal/output experiment. It retained the 256/256 Raspberry
defaults, the safe momentary FX cache, and the profiling tooling; it did not
establish an Orange output or callback result. The corrected Orange live
comparison is recorded above.
