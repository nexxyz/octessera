# Pi audio buffer experiment: 128-frame rendering and output

Decision: keep the Pi at 256-frame internal render blocks and a 256-frame output buffer for product defaults.

## What we tested

The experiment asked two separate questions:

1. Can the engine render internally in 128-frame blocks while the real output callback stays at 256 frames?
2. Can the Pi run a true 128-frame output buffer?

The first question passed. The second failed.

## Branch and commits

Experimental branch: `experiment/128-frame-synth-workers`.

Useful commits brought back to `main`:

- `Cache momentary FX runtime params`: caches momentary FX render parameters so the audio path avoids per-sample map/serde lookups and repeated conversion math.
- `Extend Pi timing probe profile modes`: adds explicit callback-sized profile measurement and soak mode to `tools/pi/run-pi-timing-probes.ps1`.

Experimental commit not brought back:

- `Experiment with 128-frame synth workers`: lowered the synth-worker parallel gate for 128-frame blocks. It was useful for testing, but it does not create a product latency win while output remains 256 frames.

## Internal 128, output 256

Configuration:

```text
OCTESSERA_AUDIO_BLOCK_FRAMES=128
OCTESSERA_PI_PROFILE_MEASURE_FRAMES=256
OCTESSERA_AUDIO_OUTPUT_BUFFER_FRAMES=256
OCTESSERA_SYNTH_SLOT_WORKERS=2
```

Result: stable enough for later consideration, but not a real output-latency win.

Evidence:

- Three callback-sized FX-limit profile runs were clean.
- Product-limit rows stayed below realtime; highest observed product max was `0.802892`.
- No `parallel_fail`, `parallel_unhealthy`, or `parallel_timing_backoff` in those FX-limit runs.
- 120s AudioDrain was clean: max `4042us`, no >5ms, >10ms, or >20ms drain latency.
- 120s live probes covered `idle`, `pulses-stress`, `stop-start`, `encoder-stress`, `mute-stress`, and `sparks-page-stress` with no >10ms or >20ms wake/loop lateness and no audio-send stalls over 1ms.
- 128/256 live probes did not print ALSA underrun recoveries.

Soak profiles had isolated raw max spikes in the bus-heavy scenario. The same kind of spike also appeared with workers disabled and with production 256/256 controls, so we treated it as Pi scheduling noise in the synchronous profile path rather than a 128-internal render failure.

## True output 128

Configuration:

```text
OCTESSERA_AUDIO_BLOCK_FRAMES=128
OCTESSERA_AUDIO_OUTPUT_BUFFER_FRAMES=128
OCTESSERA_SYNTH_SLOT_WORKERS=2
```

Result: not reasonably feasible on the current Pi ALSA/cpal output path.

Blocking evidence:

- 30s AudioDrain at internal 128/output 128/workers 2 printed 9 ALSA underrun recoveries.
- 30s AudioDrain at internal 256/output 128/workers 2 still printed 3 ALSA underrun recoveries and had recurring >5ms drain latency.
- 30s AudioDrain at internal 128/output 128/workers 0 still printed 2 ALSA underrun recoveries.
- Prior internal 128/output 256 probes were clean.

That isolates the failure to the real 128-frame output buffer path, not the synth-worker experiment or momentary FX render cost.

## What we learned

- Internal 128-frame rendering can be made stable behind a 256-frame output buffer.
- That does not halve real output latency. The hardware/backend callback budget is still 256 frames.
- True 128-frame output underruns before synth-worker load matters.
- The practical product floor remains 256/256 for now.
- If we chase lower real latency later, start with Pi ALSA/cpal output characterization, not synth optimization.

Potential future probes:

- Find the output-buffer knee at 160, 192, and 224 frames.
- Compare jack-only and USB-only output modes if both are active.
- Investigate ALSA/cpal scheduling and period settings before trying true 128 again.
