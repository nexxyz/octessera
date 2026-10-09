# Pi development and profiling

This page covers host-only Pi builds and board-specific profiling. It does not
replace the ordered Orange bring-up procedure or the deployment runbook.

Canonical board IDs are `raspberry-pi-zero-2w` and `orange-pi-zero-2w`; their
images, pinouts, port roles, and deployment adapters are not interchangeable.
See [`../board-profiles.md`](../board-profiles.md) for feature owners and
supported board IDs.

## Builds without hardware

Host-stub Pi app build:

```bash
cargo build -p octessera-pi
```

Hardware HAL target check when the Rust target is installed:

```bash
cargo check --target aarch64-unknown-linux-gnu -p octessera-hal --features raspberry-pi-zero-2w
```

The Orange cross-builder is WSL Docker-only. It never contacts or deploys to a
board and writes checked artifacts under `target/orange-pi-cross/`:

```powershell
./tools/orange-pi/build-opi-cross.ps1 -Binary orange-oled-smoke -Profile release
./tools/orange-pi/build-opi-cross.ps1 -Binary octessera-pi -Profile release
./tools/orange-pi/test-build-opi-cross.ps1
```

Cargo and rustup caches use persistent named Docker volumes; `-DryRun` prints
the command without starting Docker. The helper accepts the two diagnostic
smoke binaries and a local Orange `octessera-pi` development binary. It does
not produce a production image or its `production-runtime` bundle, and no
artifact is run against the board by this helper.

## Pi UI and audio profiling

Pi UI/render profiling is quiet by default. Enable summaries with either form:

```bash
OCTESSERA_PI_UI_PROFILE=1 octessera-pi
octessera-pi --profile-ui
```

Summaries include loop cadence, runtime tick lateness/advance, render overruns,
snapshot/config sync, hardware polling, and LED/NeoKey/OLED phase timings.

Use Pi-side probes for rhythmic timing, trigger latency, audio-drain latency,
and DSP budget questions. PC/runtime-only probes do not measure hardware audio
timing. `tools/pi/run-pi-timing-probes.ps1` is
Raspberry-only; never point it at Orange.

```powershell
$PiTarget = "pi@<PI_HOST>"

# Shared native logical wake probe.
cargo run -p playback-runtime --bin playback_timing_probe -- --durations 6s --scenarios idle --wake-intervals-ms 2,4,6,8,10,12

# Runtime-only probe; leaves service and live audio untouched.
./tools/pi/run-pi-timing-probes.ps1 -Target $PiTarget -Mode RuntimeOnly -Durations 15s -Scenarios idle,pulses-stress -WakeIntervalsMs 2,4,6,8,10,12 -Snapshots

# Live-audio probe.
./tools/pi/run-pi-timing-probes.ps1 -Target $PiTarget -Mode Live -AllowServiceInterruption -Durations 10m -Scenarios idle -WakeIntervalsMs 2,4,6,8,10,12

# Audio-source drain latency probe.
./tools/pi/run-pi-timing-probes.ps1 -Target $PiTarget -Mode AudioDrain -AllowServiceInterruption -Durations 10m

# FX budget profile.
./tools/pi/run-pi-timing-probes.ps1 -Target $PiTarget -Mode DspFxLimits -AllowServiceInterruption

# Alternate render quantum.
./tools/pi/run-pi-timing-probes.ps1 -Target $PiTarget -Mode DspFxLimits -AllowServiceInterruption -AudioRenderQuantumFrames 256
```

The shared binary uses a deterministic logical matrix and the Pi RuntimeOnly
mode leaves the service and live audio untouched. Pi Live opens live audio.
Both Pi modes pass the actual elapsed interval to the shared runtime. These
metrics diagnose trigger cadence and event batching; use `AudioDrain` separately
for queue/control-drain measurements.

## Backlog: shared-path timing smoke

Make a short, repeatable on-device smoke profile for changes to shared audio
processing or musical-event emission: bus-FX routing, general parameter
modulation, transport, or event dispatch. Run it on both Pi variants with a
Playing default patch, OLED awake, and representative parameter turns. Compare
the same conditions before and after the change: note/event cadence, cumulative
drift, late or batched pulses, and audio underruns. Do not call a shared-path
change qualified if timing regresses or drift continues to accumulate.

An individual new instrument or Build behavior can use focused tests and its
own on-device profile unless it changes one of those shared paths. The existing
Raspberry probes and Pi UI summaries help, but PC-only or DSP-only results
would not have caught a Playing display/control stall. Reuse the existing board
profiling tools for this smoke rather than adding a new CI framework.

The wrapper stops `octessera.service` for live/audio/DSP modes and restarts it
afterward; those modes require `-AllowServiceInterruption`. Runtime-only leaves
it running. Use `-PrintOnly` to inspect the remote command first.

For the Raspberry normal-runtime, OLED-awake Aux/autosave study, first cross-build
the current clean source, then run the candidate against an isolated preset clone:

```powershell
./tools/pi/run-pi-autoaux-study.ps1 -Target $PiTarget -Artifact target/pi-cross/octessera-pi -Metadata target/pi-cross/octessera-pi.metadata.json -LiveSeconds 30 -AllowServiceInterruption
```

Raspberry and Orange share one Aux timing driver (`OCTESSERA_TIMING_AUTOAUX=1`
with `OCTESSERA_TIMING_AUTOPLAY=1` and `OCTESSERA_TIMING_KEEP_AWAKE=1`) and one
report format; only the `raspberry-autoaux`/`orange-autoaux` prefix differs.
On both boards `OCTESSERA_TIMING_AUTOPLAY=1` alone starts playback at boot,
and `OCTESSERA_TIMING_KEEP_AWAKE=1` stops startup unless the loaded settings
keep the display awake. Raspberry baselines captured before this driver used
`oled_frame_*`/`synth_cutoff_commands` keys.

This one-shot study stops and restores the installed service. It requires a
matching final automatic-save receipt and verifies that the original presets
remain unchanged. Inspect its staged candidate and kernel journals for timing
and audio errors; missing error text alone does not prove that no underrun
occurred. On failure, retain the study evidence and clone until service
restoration and the original-store checks are resolved.

### Capacity and live-latency studies

`OCTESSERA_TIMING_LOAD=start,step,max,seconds` (with `OCTESSERA_TIMING_AUTOPLAY=1`
and an isolated study store, same rules as the Aux study) ramps held 2-second
notes across Synth, Pluck, and FM through the host musical-event path on top of
the loaded patch, logging one `timing-load step=...` line per step. Every ~0.7 s
it also presses grid cell (0,0) through the real input dispatch and reports
`probe_wait` (arrival to dispatch) and `probe_dispatch_to_audio` (dispatch to
the note reaching the audio service); put a Keys layer with Link events enabled
on the active layer in the clone, or the probe reports `probe_silent`. Set
`sound.voiceStealingMode` to `none` in the clone's patch so stealing does not
hide the limit. Correlate the step lines with `underrun occurred` lines and the
`pi-ui-profile` windows in the candidate journal.

Run boards one at a time when comparing underrun counts, and keep the study
host quiet: a shell loop that spawns processes every second on the board shows
up as rare extra underruns. Tools that found real problems:

- Orange has `perf`: `perf record -k CLOCK_MONOTONIC -C 1` on the audio core, or
  `-g -t <runtime tid>` for the runtime thread, lines up with journal timestamps.
- Both boards have ftrace: `sched_switch`/`sched_wakeup` on CPU 1 with
  `trace_clock=mono` separates "callback busy" from "callback not scheduled".
- `/proc/<pid>/task/<cpal tid>/stat` fields 10 and 12 (minor/major faults)
  sampled around playback start or background load show SD-card page faults in
  the audio thread.
- A memory-bandwidth hog on the other cores (copying a 48 MB buffer in a loop)
  starves the Raspberry's audio core; CPU-only and network load barely do.

After live probes, inspect the current boot's service journal:

```powershell
$PiTarget = "pi@<PI_HOST>"
./tools/pi/with-rpi-ssh.ps1 ssh -Target $PiTarget 'sudo journalctl -u octessera.service -b --no-pager'
```

Use `-PrintOnly` before any live or service-changing probe. Runtime-only mode
leaves the service running; live, audio-drain, and DSP modes stop the service
and restart it afterward. These probes measure timing and audio-path behavior;
they do not replace board bring-up or image checks.
